# Raven Data Studio: execução e recuperação

Este documento descreve o motor determinístico: como um plano aprovado vira escrita no destino, o que acontece quando algo falha e o que é (e o que não é) garantido.

## Pré-condições de uma execução

O motor recusa começar se qualquer uma falhar, e a recusa diz qual:

1. A `PlanVersion` está **aprovada** e é a `approved_version` do projeto.
2. Existe `SimulationResult` **válida** para essa versão.
3. O fingerprint de cada origem é igual ao da simulação. Se mudou: estado `Bloqueado`, com a diferença de estrutura.
4. O fingerprint do destino é igual ao da simulação, exceto pelas mudanças que o próprio plano faz.
5. Não há pendência bloqueante aberta.
6. A conexão de destino declara as capacidades que o plano usa (ex.: `idempotent_upsert` para upsert).

## Fases

```
 Preparando ──► Schema ──► Carga por entidade (em ordem de dependência) ──► Reconciliação ──► Relatório
```

### 1. Preparando

- Abre as conexões com o segredo lido do cofre.
- Confere fingerprints e capacidades.
- Cria o `ExecutionJob` com `run_key` = hash(`plan_version`, fingerprints). Duas execuções do mesmo plano sobre a mesma origem têm a mesma `run_key`.
- Cria as tabelas de controle do módulo no destino, se o plano permitir (ver "Onde ficam os controles").

### 2. Schema

- Executa o DDL aprovado, um comando por vez, na ordem calculada (tabelas, colunas, depois índices e chaves estrangeiras).
- Cada comando é registrado com resultado. DDL **não** é desfeito automaticamente: na maior parte dos bancos o DDL tem efeito próprio; o PostgreSQL permite DDL transacional, e o adaptador declara isso em `ConnectorCapabilities.ddl.transactional`. Quando é transacional, todo o DDL do plano roda numa transação só; quando não é, roda comando a comando e a falha para a execução com o estado de cada comando.
- No cenário: `ALTER TABLE customers ADD COLUMN legacy_id`, `CREATE TABLE customer_addresses`, `CREATE TABLE orders`, índices únicos e FKs.

### 3. Carga

A ordem vem do `DependencyGraph` do plano: `customers` → (`customer_addresses`, `orders`). Entidades sem dependência entre si podem rodar em paralelo (Evolução); no MVP rodam em sequência.

Para cada entidade:

```
 para cada lote (padrão 1.000 registros da origem, ordenados pela chave legada):
     ler o lote da origem (página por chave, nunca offset em tabela grande)
     aplicar transformações e validações (puras, no motor)
     separar: gravar | rejeitar | revisar | reter
     resolver referências pelo mapa legacy_id → id (ex.: pedido → cliente)
     abrir transação no destino
         gravar os registros (insert/upsert conforme a política)
         gravar o mapa legacy_id → id dos registros deste lote
         gravar o checkpoint (lote, cursor da origem)
     commit
     gravar rejeitados e pendências do lote no banco do módulo
     publicar progresso
```

- **Mescla de duplicados (C1):** acontece antes da carga, numa passagem de agrupamento por chave normalizada. O resultado (sobrevivente e aliases) entra no mapa de ids, para os pedidos de 2231 caírem no cliente de 1042 (C12).
- **Referência ausente (C5):** o pedido vai para `RejectedRecord` com motivo `referencia_ausente`, sem abrir exceção no lote.
- **Retidos (C3):** pedidos cujo cliente está em revisão ficam como `Held { waiting_for: cliente }`. Quando a pendência do cliente é resolvida, uma **execução complementar** do mesmo plano grava o cliente e os pedidos retidos.

### 4. Reconciliação

Conferências feitas no fim (MVP: contagens; Avançado: somas de controle):

| Conferência | Cenário |
|---|---|
| Lidos = gravados + rejeitados + em revisão + retidos + mesclados | CLIENTES: 1.248 = 1.202 + 0 + 9 + 0 + 37 |
| | PEDIDOS: 5.312 = 5.261 + 14 + 6 + 31 + 0 |
| Linhas no destino com `legacy_id` desta execução = gravados | customers 1.202, customer_addresses 1.568, orders 5.261 |
| Soma de `orders.total` = soma de `VALOR` dos pedidos gravados | Avançado |

Uma diferença na reconciliação não apaga nada: o job termina como `ConcluidoComPendencias` e o relatório mostra a divergência.

### 5. Relatório

`ExecutionReport` com contagens por entidade e ação, tempos por fase, lotes, retomadas, rejeitados por motivo e reconciliação. Exportado pelo motor de relatórios (PDF, XLSX); rejeitados em CSV fiel (ver "Rejeitados").

## Idempotência

O objetivo é: **rodar de novo o mesmo plano não duplica nada e chega ao mesmo destino.**

- Cada registro gravado carrega `legacy_id` (no cenário, coluna nova em `customers` e `orders`; em `customer_addresses`, a chave composta `customer_id + kind`).
- A gravação usa upsert pela chave de casamento (`document` em customers, `legacy_id` em orders). Repetir um lote já gravado vira atualização sem mudança.
- Quando o destino **não** tem upsert idempotente declarado, o motor consulta o mapa de ids antes de inserir e pula o que já está mapeado. É mais lento e é declarado na simulação.
- O mapa de ids é gravado na **mesma transação** do lote. Não existe estado em que o registro está no destino e o mapa não sabe.

## Cancelar, pausar, retomar

| Ação | Efeito | Fase |
|---|---|---|
| Cancelar | O lote em curso termina (commit ou falha inteiro); nenhum lote novo começa; job `Cancelado`. Lotes já gravados ficam gravados. | MVP |
| Pausar | Igual a cancelar, mas o job fica `Pausado` e pode continuar. | Evolução |
| Retomar | Relê o checkpoint de cada entidade, confere fingerprints de novo e continua do próximo lote. Se a origem mudou, `Bloqueado`. | Evolução |
| Falha de conexão | O lote em curso falha (rollback do lote); até 3 novas tentativas com espera crescente; depois `Falhou`, retomável. | MVP (falha), Evolução (retomar) |
| Erro de restrição no destino (ex.: único violado por dado concorrente) | O lote é refeito registro a registro para isolar o culpado; o culpado vira rejeitado `restricao_destino`; o resto do lote é gravado. | MVP |

**Garantia oferecida:** cada lote é atômico; a execução como um todo é **retomável e idempotente**, não atômica.

**Garantia não oferecida:** rollback global. Desfazer uma execução concluída é uma operação separada da fase Avançado: apaga, na ordem inversa das dependências, os registros com `legacy_id` desta execução que foram **inseridos** (não os atualizados, cujo valor anterior só é restaurável se a execução tiver guardado a imagem anterior, o que é opcional e custa espaço).

## Rejeitados

- Guardados no banco do módulo como `RejectedRecord` com o registro **original** (bytes da linha do CSV ou valores crus da linha da tabela), cifrado.
- "Exportar rejeitados" gera um CSV com as **mesmas colunas e o mesmo formato da origem**, mais `motivo` e `regra`. Ele pode ser corrigido e reimportado pelo mesmo projeto.
- Isso é diferente de exportar a grade de rejeitados: a grade mostra valores formatados e colunas escolhidas; o arquivo fiel preserva o original. A grade usa `ravenrs-grid-export`; o arquivo fiel é escrito pelo motor.

## Onde ficam os controles

| Dado | Onde | Por quê |
|---|---|---|
| Projeto, versões, simulações, jobs, rejeitados, relatório | Banco do módulo (persistência do ravenrs-app) | É dado administrativo do aplicativo, não do destino. |
| Mapa `legacy_id → id` e checkpoint de cada lote | **No destino**, numa tabela de controle (`_raven_import_map`, `_raven_import_checkpoint`), quando o plano permite | Precisa estar na mesma transação do lote. |
| Alternativa quando o destino não pode receber tabelas de controle | Banco do módulo, com gravação em duas fases e conferência na retomada | Mais frágil; declarado na simulação como "idempotência por conferência". |

## Geração de ids no destino

A chave `id` das tabelas novas é gerada pelo banco de destino (`BIGINT GENERATED … AS IDENTITY` no PostgreSQL), não pelo motor. Isso evita depender da geração de chaves do ravenrs-app, que tem limitações para criação concorrente por vários processos (ver [02-analise-do-codigo.md](02-analise-do-codigo.md#persistência)). Consequências para o planejamento:

- O motor precisa do id gerado de volta (`RETURNING id`) para alimentar o mapa; adaptadores sem essa capacidade usam a chave de casamento para reler.
- Execuções paralelas da mesma entidade (Evolução) não podem depender de faixa de ids calculada no cliente.

## Progresso e observabilidade

- Eventos de progresso por entidade e lote: `registros_lidos`, `gravados`, `rejeitados`, `em_revisao`, `lote_atual / total_estimado`, taxa por segundo e estimativa de término.
- O total de lotes é **estimado** a partir da contagem do snapshot e marcado como estimativa até a leitura terminar.
- A interface recebe eventos agregados (no máximo 4 por segundo); nunca recebe os registros.
- Anúncios para leitor de tela a cada 10% e em toda mudança de estado (ver [13-seguranca-privacidade-a11y-i18n.md](13-seguranca-privacidade-a11y-i18n.md)).
