# Raven Data Studio: critérios de aceitação

Cada critério é verificável: **entrada → resultado esperado**, com o caso de recusa, o de permissão e o teste que comprova. Os dados de teste são os do [cenário de referência](cenario-de-referencia.md), gerados como fixtures (CSV e um banco PostgreSQL de teste com `customers` contendo 4.310 linhas, das quais 142 casam por CPF).

Tipos de teste:
- **U**: unitário no crate de domínio, sem I/O.
- **I**: integração com adaptador e banco de teste.
- **UI**: teste de interação com os widgets, sem janela.
- **E2E**: cenário completo pelo motor sem interface.

## Projetos e conexões

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-01 | Pedido de referência → proposta com origens CLIENTES e PEDIDOS, destinos `customers`, `customer_addresses`, `orders` e pendências C1 e C10 | Sem modelo instalado → "Começar vazio" disponível, nenhum erro bloqueante | Sem `Create` em `ImportProject` → ação ausente | UI + teste do agente com resposta gravada |
| RF-02 | Duplicar projeto aprovado → cópia em `Rascunho`, sem aprovação nem simulação | Arquivar projeto `Executando` → indisponível com motivo | Leitor não vê Duplicar | UI |
| RF-03 | `clientes.csv` em Windows-1252 com `;` → codificação e separador detectados; 1.248 registros | Arquivo sem cabeçalho → pede para nomear colunas | — | I |
| RF-04/05 | Conexão PostgreSQL de teste → `check` ok; capacidades: ddl transacional, upsert idempotente | Senha errada → categoria `Permission`, mensagem do servidor | Sem permissão de conexões → formulário só leitura | I; varredura do valor da senha em plano, log, relatório e auditoria → 0 ocorrências |
| RF-06 | (Avançado) Servidor MCP → ferramentas listadas, todas bloqueadas | Ferramenta de escrita → exige digitar o nome para liberar | Só administrador de conexões libera | I com servidor MCP de teste |

## Descoberta

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-10 | Descobrir `legado` → 2 estruturas, colunas e tipos como na tabela do cenário; `PEDIDOS` sem FK declarada | Tabela sem permissão de leitura → listada com aviso, não falha | — | I |
| RF-11/12 | Perfil de `FONE` → "318 vazios · conjunto inteiro"; de `CEP` → inferido "CEP", confiança Alta | Sem pushdown → números marcados "amostra de 200" | — | I + U |
| RF-13 | Chave candidata `KeepDigits(CPF)` → 37 grupos, 74 registros | — | — | U |
| RF-14 | (Evolução) Inferir vínculo → `PEDIDOS.COD_CLI → CLIENTES.COD_CLI`, evidência "5.298 de 5.312" | Inclusão < 90% → não sugere, só mostra | — | U |
| RF-15 | Acrescentar coluna `OBS` em PEDIDOS depois do snapshot → fingerprint diferente; aviso na tela 4; execução `Bloqueado` | — | — | I + E2E |

## Schema

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-20/21 | Proposta → `customers` Alterar, `customer_addresses` e `orders` Criar, mescla Intermediária | Entidade sem chave primária → pendência bloqueante | Leitor não edita | U |
| RF-22 | Mescla de 1042 e 2231 → `legacy_id = 1042`, alias `2231` no mapa | — | — | U + E2E |
| RF-23 | Comparar → DDL idêntico ao de `11-schema-mapeamento-transformacoes.md` | Destino sem DDL → mensagem; nenhum comando executado | — | U (snapshot do texto) + I |
| RF-24 | (Evolução) DDL v6 × v7 → `TextDiff` com a linha de `total` alterada | — | — | UI |

## Mapeamento

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-30 | Aceitar a proposta → 22 campos de destino, 0 obrigatórios sem origem | Obrigatório sem origem → linha "Sem origem · bloqueia", aprovação indisponível | Leitor só vê | UI |
| RF-31 | (Evolução) Selecionar `orders.status` na grade → mesma ligação selecionada no canvas, e vice-versa | Ligação `DATA` → `phone` → recusada com motivo, plano inalterado | — | UI (ponteiro e teclado) |
| RF-32 | 1 linha de CLIENTES com `END_ENTREGA` → 1 `customers` + 2 `customer_addresses` | — | — | U |
| RF-33 | `kind ← 'main'` constante; `orders.customer_id` por lookup com aliases | Lookup sem correspondência → rejeitado `referencia_ausente` | — | U |
| RF-34 | Aceitar 4 de 5 operações → versão n+1 com 4 operações; a recusada não aparece | Operação inválida no patch → recusada com motivo antes de chegar ao usuário | — | U + UI |

## Transformações e validações

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-40 | `MARIA  DA SILVA ` → `Maria da Silva`; `123.456.789-09` → `12345678909`; `1310100` → `01310100`; `15/03/2024` → `2024-03-15`; `1.234,56` → `1234.56`; `""` → `NULL`; `FAT` → `invoiced` | `31/02/2024` → erro do registro; `1,234.56` → erro de formato | — | U (tabela de casos) |
| RF-41 | Regra `ParseDecimal` na amostra → "200 afetados, 0 erros" | Exemplo da IA que não bate → regra recusada | — | U |
| RF-42 | `phone` com `EmptyToNull` → 318 nulos, não texto vazio | — | — | U + I (conferir `IS NULL` no destino) |
| RF-43 | `Check::Cpf` → 9 inválidos para revisão | `Severity::Error` bloqueia o registro, não a execução | — | U |
| RF-44 | (Evolução) Salvar "CPF brasileiro" como modelo → reaplicável em outro projeto | — | Só Analista ou acima | U |

## Conflitos

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-50/51 | Upsert por `document` → 142 atualizam só `phone`, 1.060 inserem; `name` dos 142 inalterado | Marcar `name` como atualizável → pede confirmação | — | E2E (conferir `name` antes e depois) |
| RF-51 | Cliente 1500 com `FONE` vazio na origem (variante do teste) → `phone` do destino preservado | — | — | I |
| RF-52 | 37 grupos → sobrevive o de `DT_CADASTRO` mais antiga | Grupo com datas iguais → pendência por grupo | — | U |
| RF-53 | Pedido 88019 → rejeitado; 88020 → retido até o cliente 1777 ser decidido | — | — | E2E |

## Simulação, execução e resultado

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-60 | Simular local no conjunto → `mode = LocalValidation`, `coverage = Full` exibidos | Execução isolada sem capacidade → opção indisponível com motivo | Simular exige Analista | U + UI |
| RF-61 | Simulação → 1.202 / 1.568 / 5.261 gravados; 83 fora; contas fecham | — | — | E2E |
| RF-62 | Mudar `orders.total` para `NUMERIC(10,2)` → simulação `valid = false`, aprovação revogada, evento de auditoria | Mover nó no canvas → simulação continua válida | — | U |
| RF-63 | Aprovar sem pendências bloqueantes → `Approval` gravada | Pendência C10 aberta → Aprovar indisponível com o motivo como descrição | Sem `action_permission` Aprovar → botão ausente | UI + U |
| RF-64 | Executar → lotes de 1.000 por entidade, em ordem `customers` → endereços e pedidos | Plano não aprovado ou fingerprint diferente → recusa com motivo | Sem permissão Executar → ação ausente | E2E |
| RF-65 | Cancelar durante o lote 3 de `orders` → lotes 1 a 3 `Committed`, job `Cancelado`, checkpoint no lote 3 | — | Só Operador | E2E com ponto de parada |
| RF-65 | (Evolução) Falha de conexão no lote 4 → `Falhou`; Retomar → resultado igual ao de uma execução sem falha | Origem mudou → Retomar bloqueado | Só Operador | E2E com falha injetada |
| RF-66 | Executar duas vezes o mesmo plano → nenhuma linha duplicada; contagens iguais | — | — | E2E |
| RF-67 | Exportar rejeitados → CSV com as colunas de PEDIDOS + `motivo` + `regra`; 88019 com `45,00` | — | Sem a permissão de exportação fiel → opção ausente | E2E (comparação byte a byte com a linha original) |
| RF-68 | Relatório → contagens e reconciliação "Confere" em todas as linhas | Divergência injetada → `ConcluidoComPendencias` com "Diverge" | — | E2E |
| RF-69 | (Avançado) Desfazer execução → remove os 7.889 inseridos, em ordem inversa; os 142 atualizados permanecem com aviso | Sem imagem anterior → não oferece restaurar atualizados | Só Administrador | E2E |

## Conversa e IA

| RF | Entrada → resultado | Recusa | Permissão | Teste |
|---|---|---|---|---|
| RF-70 | "Por que status X vai para revisão?" → explicação citando a regra e a pendência, com link | — | — | Teste do agente com resposta gravada |
| RF-71 | Resposta do modelo com operação para campo inexistente → patch recusado, motivo na conversa, plano inalterado | JSON inválido 3 vezes → mensagem e sugestão de editar na grade | — | U (respostas forjadas) |
| RF-72 | Política "só estrutura" → contexto enviado sem nenhum valor de célula | Comentário de coluna com texto de instrução → aparece como dado; nenhuma operação além das pedidas | — | U (inspeção do contexto montado) |
| RF-73 | Escolher modelo local instalado → usado nas próximas propostas | Modelo que não cabe na máquina (`LocalModel::fits`) → indisponível com motivo | — | UI |

## Requisitos não funcionais

| RNF | Verificação |
|---|---|
| RNF-01 | Prévia de tabela com 1 milhão de linhas: memória da interface estável ao rolar até o fim; nenhuma requisição com `limit` > 200 |
| RNF-02 | Ida e volta JSON do plano do cenário → igual; `schema_version` presente |
| RNF-03 | E2E roda sem janela |
| RNF-04 | Varredura do segredo de teste em todos os artefatos → 0 ocorrências |
| RNF-05 | `Audit::app` sem falhas em cada tela; roteiro só com teclado completa o cenário |
| RNF-06 | Trocar para inglês → nenhuma chave `datastudio.*` faltando (teste de catálogo) |
| RNF-07 | Revisão das telas: todo `Badge` de estado tem ícone e texto |
| RNF-09 | Simulação local do cenário < 3 s |
| RNF-10 | Execução do cenário < 30 s |
