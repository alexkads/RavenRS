# Raven Data Studio: visão do produto e requisitos

## O que é

O Raven Data Studio é um módulo do RavenRS para **trazer dados de um sistema para outro com segurança**: descobrir o que existe na origem, propor a estrutura do destino, mapear e transformar campos, decidir conflitos, simular e só então executar, com relatório e recuperação.

A IA entra para **transformar um pedido em linguagem natural num plano estruturado**. Ela não executa nada. O plano é um documento versionado, que o domínio valida, o usuário revisa e um motor determinístico executa.

## Para quem

| Perfil | O que quer | Como o produto atende |
|---|---|---|
| Implantador de sistema (consultor, revenda) | Migrar a base do cliente do sistema antigo num dia, sem escrever script. | Pedido por intenção, plano proposto, simulação com contagens, relatório para o cliente assinar. |
| Desenvolvedor de app RavenRS | Popular o banco do app novo com dados reais e repetir quando o legado mudar. | Plano salvo e reexecutável, idempotente, com mapa de ids legados. |
| Analista de dados da empresa | Importar planilhas recorrentes (fornecedores, preços) sem pedir para a TI. | Modelos reutilizáveis, validações, fila de revisão. |
| Auditor / responsável pelos dados | Saber o que entrou, o que ficou de fora e por quê. | Versões do plano com autor, aprovações, rejeitados fiéis, reconciliação. |

## O pedido de referência

> "Importe clientes e pedidos do sistema antigo. Separe os endereços, identifique duplicidades por CPF, preserve os vínculos dos pedidos e crie as tabelas necessárias."

O produto transforma esse pedido em um plano com:

1. **Conexões** de origem (`legado`: `CLIENTES`, `PEDIDOS`) e destino (`loja`).
2. **Estruturas descobertas**, com tipos declarados e inferidos e perfil dos dados.
3. **Schema proposto**: usar `customers` (com a coluna nova `legacy_id`), criar `customer_addresses` e `orders`.
4. **Relacionamentos**: `PEDIDOS.COD_CLI → CLIENTES.COD_CLI` inferido (não declarado), virando `orders.customer_id → customers.id`.
5. **Mapeamento de colunas**: um para um, vários para um (mescla de duplicados) e um para vários (divisão do endereço).
6. **Transformações e validações**: CPF só com dígitos e validado, CEP com zeros, datas e valores brasileiros, telefone vazio como nulo, situações mapeadas.
7. **Política de conflitos**: upsert de clientes pelo CPF, campos atualizáveis, vazio não apaga.
8. **Dependências de execução**: clientes antes de endereços e pedidos.
9. **Prévia dos resultados**: contagens por entidade e ação, com a conta fechando.
10. **Pendências para decisão**: CPF inválido, endereço ambíguo, situação `X`, qual duplicado sobrevive.

O cenário completo, com todas as contagens, está em [cenario-de-referencia.md](cenario-de-referencia.md).

## Princípios

1. **Um plano, várias vistas.** Assistente guiado, canvas, grade e conversa leem e escrevem o mesmo `MappingPlan` versionado. Nenhuma vista guarda regra de domínio própria.
2. **A IA propõe; o domínio valida; o usuário revisa; o motor executa.** Cada sugestão chega como alteração tipada, com motivo, que pode ser aceita ou recusada item a item.
3. **Nada é escrito no destino sem simulação válida da versão aprovada.**
4. **Números dizem de onde vieram.** Declarado, inferido, calculado sobre amostra ou sobre o conjunto.
5. **Problemas persistentes ficam numa lista persistente.** Notificação transitória só confirma ações.
6. **IA local por padrão.** Um provedor remoto só com escolha explícita, e o domínio decide o que sai da máquina.
7. **Conteúdo externo é dado, nunca instrução.**
8. **Sem promessas que o motor não cumpre.** Não há rollback global; há lotes, checkpoints, idempotência e reconciliação.
9. **Composto com a biblioteca.** Toda tela é feita com componentes do RavenRS; o que faltar entra antes na biblioteca, genérico, e só depois é usado aqui.

## Requisitos funcionais

Os identificadores são usados em [15-criterios-de-aceitacao.md](15-criterios-de-aceitacao.md) e no roadmap.

### Projetos e conexões

| Id | Requisito | Fase |
|---|---|---|
| RF-01 | Criar projeto a partir de um pedido em linguagem natural ou vazio. | MVP |
| RF-02 | Listar, buscar, duplicar, arquivar projetos; ver estado e última execução. | MVP |
| RF-03 | Cadastrar conexão de arquivo CSV (caminho ou arrastar), com separador, codificação, cabeçalho e formato de data detectados e editáveis. | MVP |
| RF-04 | Cadastrar conexão com um banco relacional (MVP: PostgreSQL ou SQLite, ver decisão D-01), com segredo no cofre do sistema. | MVP |
| RF-05 | Testar conexão e mostrar as capacidades declaradas pelo adaptador. | MVP |
| RF-06 | Cadastrar servidor MCP externo como fonte de conectores, com capacidades listadas e permissões por ferramenta. | Avançado |

### Descoberta

| Id | Requisito | Fase |
|---|---|---|
| RF-10 | Listar estruturas (tabelas, arquivos), colunas, tipos declarados e chaves declaradas. | MVP |
| RF-11 | Perfil por coluna: nulos, vazios, distintos, mínimo/máximo, padrões, tipo inferido com confiança. | MVP |
| RF-12 | Indicar a base de cada número: declarado, inferido, amostra (com tamanho) ou conjunto inteiro. | MVP |
| RF-13 | Detectar duplicidades por chave candidata escolhida (ex.: CPF normalizado). | MVP |
| RF-14 | Inferir relacionamentos não declarados por nome, tipo e inclusão de valores, com evidência. | Evolução |
| RF-15 | Gravar snapshot datado com fingerprint e detectar mudança de estrutura da origem. | MVP |

### Schema

| Id | Requisito | Fase |
|---|---|---|
| RF-20 | Para cada entidade de destino: usar existente, criar, alterar ou estrutura intermediária. | MVP |
| RF-21 | Definir tipo lógico, precisão, nulidade, padrão, chave primária, únicas, índices, chaves estrangeiras. | MVP |
| RF-22 | Guardar identificador legado (`legacy_id`) e aliases de registros mesclados. | MVP |
| RF-23 | Comparar destino atual × proposto e mostrar o DDL gerado como texto revisável. | MVP |
| RF-24 | Destaque de sintaxe SQL e comparação lado a lado de versões do DDL. | Evolução |

### Mapeamento

| Id | Requisito | Fase |
|---|---|---|
| RF-30 | Mapear em grade: destino, origem, transformação, estado, problemas. | MVP |
| RF-31 | Mapear em canvas sincronizado com a grade (seleção, mudanças, pendências). | Evolução (canvas simples no MVP é opcional, ver D-04) |
| RF-32 | Relações 1:1, N:1 (mescla) e 1:N (divisão). | MVP (1:1, 1:N por divisão), Evolução (N:1 geral) |
| RF-33 | Constante, cálculo (expressão), lookup em outra entidade, campo ignorado, obrigatório sem origem como pendência. | MVP |
| RF-34 | Aceitar ou recusar sugestões da IA item a item, com motivo visível. | MVP |

### Transformações e validações

| Id | Requisito | Fase |
|---|---|---|
| RF-40 | Regras estruturadas: aparar, espaços, caixa, só dígitos, completar à esquerda, data por formato, decimal por separadores, dividir, juntar, tabela de valores, vazio para nulo, expressão. | MVP |
| RF-41 | Antes e depois sobre a amostra para cada regra, com contagem de afetados e de erros. | MVP |
| RF-42 | Política de nulo e de erro por regra. | MVP |
| RF-43 | Validações: obrigatório, CPF/CNPJ, padrão, faixa, conjunto, único, expressão; severidade erro ou aviso. | MVP |
| RF-44 | Modelos de regras reutilizáveis entre projetos. | Evolução |

### Conflitos

| Id | Requisito | Fase |
|---|---|---|
| RF-50 | Por entidade: inserir, atualizar, ignorar, upsert, revisão. | MVP (inserir, upsert, ignorar), Evolução (revisão por registro) |
| RF-51 | Chaves de casamento, campos atualizáveis, vazio na origem não apaga. | MVP |
| RF-52 | Duplicados na origem: mesclar com regra de sobrevivente, primeiro vence, ou revisão. | MVP |
| RF-53 | Referência ausente: rejeitar, revisar ou reter até outra decisão. | MVP |

### Simulação, execução e resultado

| Id | Requisito | Fase |
|---|---|---|
| RF-60 | Simular em modo validação local, geração de comandos ou execução isolada (quando o adaptador permitir), dizendo qual. | MVP (local e comandos), Evolução (isolada) |
| RF-61 | Mostrar mudanças de schema, contagens por entidade e ação, erros, pendências; a conta fecha. | MVP |
| RF-62 | Invalidar a simulação e a aprovação após mudança relevante. | MVP |
| RF-63 | Aprovar com resumo do que será feito; bloqueio enquanto houver pendência bloqueante. | MVP |
| RF-64 | Executar em lotes, com progresso por entidade e lote. | MVP |
| RF-65 | Pausar, cancelar (ao fim do lote) e retomar do checkpoint. | MVP (cancelar), Evolução (pausar e retomar) |
| RF-66 | Idempotência: reexecutar não duplica. | MVP |
| RF-67 | Rejeitados com motivo e registro original fiel; exportar CSV fiel. | MVP |
| RF-68 | Relatório de execução em PDF/XLSX, com reconciliação de contagens. | MVP (contagens), Avançado (somas de controle e amostragem cruzada) |
| RF-69 | Desfazer uma execução concluída onde o mapa de ids permitir. | Avançado |

### Conversa e IA

| Id | Requisito | Fase |
|---|---|---|
| RF-70 | Conversa ligada ao plano: pedir mudança, perguntar "por quê", receber explicação com referência aos itens. | MVP |
| RF-71 | Toda sugestão da IA vira `PlanPatch` com motivo; nada muda sem passar pelo domínio. | MVP |
| RF-72 | Configurar o que pode ir ao modelo: só estrutura, perfis agregados, amostra mascarada. | MVP |
| RF-73 | Escolher modelo local; provedor remoto por escolha explícita da pessoa. | MVP (local), Evolução (remoto) |

## Requisitos não funcionais

| Id | Requisito |
|---|---|
| RNF-01 | A interface nunca carrega o conjunto inteiro: grades e listas pedem páginas; perfis grandes são calculados no backend do módulo. |
| RNF-02 | O plano é serializável em JSON com `schema_version`, legível e comparável entre versões. |
| RNF-03 | O motor roda sem interface (para execução agendada e para testes). |
| RNF-04 | Segredos nunca aparecem em plano, log, relatório ou conversa. |
| RNF-05 | Toda tela tem alternativa tabular e navegação completa por teclado; o canvas tem a grade como equivalente. |
| RNF-06 | Textos traduzidos pelos mecanismos existentes (`ravenrs-app-i18n` e `Labels` dos widgets), pt-BR e en no MVP. |
| RNF-07 | Estado sempre com texto e ícone, nunca só cor. |
| RNF-08 | Desktop (macOS, Windows, Linux) no MVP; web depende de a conexão ao banco passar por um servidor (ver D-07). |
| RNF-09 | Tempo da simulação local do cenário de referência (1.248 + 5.312 registros) abaixo de 3 s numa máquina de desenvolvimento comum. |
| RNF-10 | Execução do cenário de referência abaixo de 30 s, dominada pela escrita no destino. |

## Fora do escopo

- Integração contínua de dados (replicação em tempo real, CDC). O produto faz cargas, repetíveis, não sincronização permanente.
- Edição de dados do destino fora do fluxo de importação.
- Geração automática de formulários e grades do ravenrs-app a partir das entidades criadas: integração futura separada.
- Criação dinâmica de entidades Rust do ravenrs-app em tempo de execução (ver [08-arquitetura.md](08-arquitetura.md)).
- Qualquer promessa de rollback global entre lotes já gravados.

## Hipóteses registradas

| Id | Hipótese | Se for falsa |
|---|---|---|
| H-01 | O primeiro destino relacional é PostgreSQL, por já ter crate de persistência e upsert nativo. | Trocar por SQLite muda só o adaptador; o domínio não muda. |
| H-02 | O primeiro modelo de IA é local (via `ravenrs-ai`), sem servidor externo. | Se o modelo local não der conta do planejamento, o MVP funciona sem IA (regras heurísticas + edição manual) e a IA remota vira escolha explícita. |
| H-03 | Amostra padrão de 200 linhas por estrutura para inferência e prévia. | Configurável por projeto. |
| H-04 | Volume do MVP: até ~1 milhão de linhas por estrutura, lotes de 1.000. | Volumes maiores ficam para a fase Avançado (escrita em massa nativa). |
| H-05 | O módulo é uma aplicação do ravenrs-app (`Module`) e também roda como exemplo independente. | Se o ravenrs-app não estiver pronto, o exemplo independente usa os mesmos crates de domínio. |
