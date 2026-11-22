# Raven Data Studio: lacunas e novos componentes propostos

Regra da casa: componente de interface que falta entra **antes** na biblioteca, genérico, com teste e exemplo na galeria, e só depois é usado pelo Data Studio. O que é regra de importação fica no módulo. Cada item abaixo diz em qual dos dois lados ele fica.

Os nomes `NodeEditor`, `NodePort`, `NodeConnection`, `SchemaDiagram`, `MappingInspector` e `SchemaDiffView` vieram do pedido como **propostas**. Nenhum existe. A análise mostra que três deles já têm equivalente pronto.

## Avaliação dos nomes propostos

| Nome proposto | Decisão | Motivo |
|---|---|---|
| `NodeEditor` | **Não criar.** Evoluir `FlowCanvas`. | `FlowCanvas` já é o editor de nós genérico da biblioteca (pan, zoom, seleção, portas, ligações validadas, minimapa, teclado). Um segundo editor seria um sistema paralelo. |
| `NodePort` | **Não criar.** Usar `FlowPort`. | Já tem lado, direção, rótulo e tipo. |
| `NodeConnection` | **Não criar.** Usar `FlowEdge`. | Já tem forma (curva, degrau, reta), rótulo e tracejado animado. |
| `SchemaDiagram` | **Criar (N)**, genérico, sobre `FlowCanvas`. | Diagrama de entidades com campos é útil fora do Data Studio (visualizar o modelo do ravenrs-app, documentação de banco). |
| `MappingInspector` | **Módulo (M)**, composto. | É a edição de um `FieldMapping` com regras de importação: específico. Feito com `FormLayout`, `LookupField`, `ExpressionEditor`, `DataGrid`. |
| `SchemaDiffView` | **Módulo (M)**, composto sobre `TextDiff` (N) e `DataGrid`. | A comparação "existente × proposto" por entidade e campo é do domínio; a comparação de texto por baixo é genérica. |

## Novos componentes reutilizáveis (biblioteca)

### N-1. `SchemaDiagram` (família `ravenrs-widgets-charts`, ao lado do `FlowCanvas`)

**Para que serve.** Mostrar entidades como cartões com a lista de campos (nome, tipo, chave, nulidade) e relacionamentos ligando campo a campo.

**Como é feito.** Usa o `FlowCanvas` por baixo: cada entidade é um `FlowNode`, cada campo é uma linha do nó com uma porta de cada lado, cada relacionamento é um `FlowEdge` entre portas de campos.

**Depende de evoluções do `FlowCanvas`** (ver E-1):

- nó com **linhas** (seções de conteúdo com uma porta por linha), em vez de só título e subtítulo;
- **recolher** um nó (mostra só o cabeçalho; as ligações dos campos escondidos se juntam numa porta do cabeçalho);
- organizar sozinho (camadas da esquerda para a direita).

**API indicativa:**

```rust
SchemaDiagram::new(model: Binding<SchemaModel>)
    .view(Binding<FlowViewport>)
    .selection(Binding<SchemaSelection>)       // entidades, campos, relações
    .collapsed(Binding<BTreeSet<String>>)
    .field_badge(Fn(&FieldRef) -> Option<Badge>) // o módulo põe "pendente", "erro"
    .on_connect(Fn(FieldRef, FieldRef) -> Result<(), String>)
    .filter(Fn(&FieldRef) -> bool)             // esconder campos e ligações
```

`SchemaModel` é genérico (entidades, campos, tipos como texto, relações). Não conhece `MappingPlan`.

**Pronto quando:** exemplo na galeria com 3 entidades e relações; teste de interação de conectar, recolher, filtrar; navegação por teclado entre campos; equivalência tabular documentada.

### N-2. `TextDiff` (família `ravenrs-widgets-chat`, ao lado do `CodeEditor`)

**Para que serve.** Comparar dois textos lado a lado ou em modo unificado, com linhas acrescentadas, removidas e alteradas marcadas por cor **e** sinal (`+`, `−`, `~`).

**Usado em:** Comparação e DDL (DDL atual × proposto), histórico do plano (JSON de duas versões), relatório de execução (plano executado × plano atual).

**API indicativa:**

```rust
TextDiff::new(before: String, after: String)
    .mode(DiffMode::SideBySide | DiffMode::Unified)
    .language(CodeLanguage)   // destaque quando houver
    .context_lines(3)
    .on_hunk(Fn(usize))       // navegar entre trechos
```

Algoritmo de diferença por linhas (Myers ou paciência) em `ravenrs-text` ou crate próprio, sem dependência de interface, para servir também ao relatório.

**Acessibilidade:** cada trecho é um item com rótulo "linhas 12 a 14 acrescentadas"; F7 / Shift+F7 vão ao próximo e ao anterior.

### N-3. `IssueList` (família `ravenrs-widgets-kit`)

**Para que serve.** Lista persistente de problemas, agrupável por severidade, origem ou regra, com filtro, contagem e navegação ao item que causou o problema. Substitui o uso indevido de toasts para erros e não confunde com o painel `Diagnostics` do desenvolvedor.

**Usado em:** Perfil, Mapeamento (pendências), Transformações (erros por regra), Simulação, Resultado. Útil também ao editor de relatórios e aos formulários do ravenrs-app (lista de violações).

**API indicativa:**

```rust
IssueList::new(issues: RemoteRows | Vec<Issue>)    // virtualizada
    .group_by(IssueGroup::Severity | Source | Rule)
    .on_open(Fn(&IssueRef))                         // leva ao campo, regra ou registro
    .actions(Fn(&Issue) -> Vec<MenuItem>)           // "Aceitar sugestão", "Ignorar"
struct Issue { id, severity: Severity /* Error | Warning | Info */, title, detail, source, count }
```

Cada item mostra ícone **e** palavra de severidade. Anúncio por região viva quando a contagem de erros muda.

### N-4. Leitor de CSV compartilhado (`ravenrs-csv`, crate novo, sem interface)

Extraído de `ravenrs-sheet/src/craft/csv` (detecção de codificação, separador, aspas) para leitura **em blocos** a partir de um `Read`, com posição de byte por registro (para paginação e retomada). A planilha passa a usá-lo. Não é cópia: o código sai de um lugar e vai para o outro.

### N-5. Cliente MCP (`ravenrs-mcp-client`, crate novo, Avançado)

Ver [10-mcp-e-conectores.md](10-mcp-e-conectores.md#2-cliente-mcp-da-aplicação-avançado). Genérico: qualquer app RavenRS pode usar.

## Evoluções de componentes existentes (biblioteca)

| Id | Componente | Evolução | Por que | Fase |
|---|---|---|---|---|
| E-1 | `FlowCanvas` | Itens que restam da #238 (desfazer, JSON e portas pelo teclado já vieram no PR #389): organizar sozinho, grupos/recolher, desempenho (linhas achatadas em cache, desenhar só o visível) **e** nó com linhas de conteúdo e porta por linha | Base do `SchemaDiagram` e do canvas de mapeamento | Evolução |
| E-2 | `CodeEditor` | `CodeLanguage::Sql` (palavras-chave, tipos, textos, números, comentários) | DDL legível | Fase 0 ou fim do MVP |
| E-3 | `DataGrid` | Editor de célula por coluna (`Select`/`LookupField` na célula) e ícone + dica em `GridCellStyle` | Grade de mapeamento editável sem inspetor; estado com ícone | MVP (ícone via `template` serve até lá) / Evolução |
| E-4 | `DataGrid` / `Value` | Decimal exato e data-hora em `Value` | Prévia de `NUMERIC(12,2)` e carimbos de tempo sem arredondar | Evolução (MVP mostra decimal como texto formatado pelo domínio) |
| E-5 | `ContextMenu` | Abrir pelo teclado (Shift+F10, tecla Menu) e itens calculados pelo alvo | Ações por campo no canvas e na árvore | MVP |
| E-6 | `TreeView` | Indicador de carregando no nó preguiçoso; tipo e contagem como acessório à direita | Explorador de conexões | MVP |
| E-7 | `Chat` | `on_stop` (parar geração) e lista virtualizada | Conversas longas com o agente | Evolução |
| E-8 | `LookupItem` | Campo `detail` (texto secundário) | Mostrar tipo do campo de destino ao escolher | MVP |
| E-9 | Tradução `GridRequest → Criteria` | Tornar pública num lugar compartilhado (hoje privada em `ravenrs-app-ui/src/list_view.rs:194`) | O serviço de prévia do módulo precisa da mesma tradução | MVP |
| E-10 | `StepIndicator` / `StepState` | Estado `Warning` | Etapa com pendência não bloqueante | Evolução, opcional |
| E-11 | `CommandPalette` | Fonte de comandos dinâmica | "Ir para campo `orders.status`" | Evolução |

## Funcionalidades específicas do módulo

| Id | Peça | Composta com |
|---|---|---|
| M-1 | `MappingGrid`: grade de mapeamento (destino, origem, regra, estado, problemas, confiança) | `DataGrid` remoto, `GridColumn::template`, `Badge` |
| M-2 | `MappingCanvas`: canvas de mapeamento origem → destino | `SchemaDiagram` (N-1) com duas colunas de entidades e nós de regra no meio |
| M-3 | `MappingInspector` | `FormLayout`, `LookupField`, `ExpressionEditor`, `DataGrid` (antes/depois) |
| M-4 | `SchemaDiffView` | `TextDiff` (N-2), `DataGrid` por entidade/campo, `Badge` |
| M-5 | Card de proposta da IA na conversa | `ChatMessage::card`, `DataGrid` pequena, `Button` |
| M-6 | Painel de execução por entidade | `ProgressBar`, `Badge`, `Stat`, `StatusBar` |
| M-7 | Perfilador e prévia (serviço) | Adaptadores + `RemoteRows` |
| M-8 | Agente de planejamento | `ravenrs-ai` |

## Infraestrutura de grafos × regras de importação

A fronteira é rígida, para que o editor de grafos sirva a outros produtos (fluxos de automação, diagramas do modelo do app) e o Data Studio não dependa de detalhes de desenho.

| Camada | Sabe | Não sabe |
|---|---|---|
| `ravenrs-graphics::flow` | Geometria: nós, portas, rotas das linhas, limites, organização automática (módulo novo) | Interface, tema, importação |
| `FlowCanvas` | Interação: ponteiro, foco, teclado, seleção, viewport, minimapa, validação genérica por `FlowValidator` | O que um nó significa |
| `SchemaDiagram` | Entidades com campos, relações campo a campo, recolher, filtrar | Mapeamento, regras, conflitos |
| `MappingCanvas` (módulo) | Que uma ligação é um `FieldMapping`, que um nó do meio é uma `TransformationRule`, quais ligações são válidas pelo tipo lógico | Como desenhar |

O `FlowValidator` do `MappingCanvas` chama o domínio ("este tipo aceita este?") e devolve o motivo em palavras; o `FlowCanvas` só mostra.

### Como o canvas atende cada necessidade

| Necessidade | Quem atende |
|---|---|
| Nós com campos e portas | `SchemaDiagram` (linhas com porta), sobre E-1 |
| Conexões selecionáveis | `FlowCanvas` (pronto) |
| Arraste e criação de ligações | `FlowCanvas` (pronto) + `FlowValidator` do módulo |
| Teste de acerto | `FlowCanvas` (pronto): caixas de nó e área de porta |
| Coordenadas tela ↔ documento | `FlowViewport` (pronto) |
| Seleção múltipla | `FlowCanvas` (pronto) |
| Pan e zoom | `FlowCanvas` (pronto) |
| Agrupamento e recolhimento | E-1 (grupos) + `SchemaDiagram` (recolher entidade) |
| Organização automática | E-1, em `ravenrs-graphics` |
| Busca e foco | Módulo: busca no plano → `FlowViewport` centraliza e foca o nó |
| Redução de conexões visíveis | `SchemaDiagram::filter` + módulo ("só pendências", "só a entidade selecionada"); E-1 desenha só o visível |
| Navegação por teclado | `FlowCanvas` (portas pelo teclado, PR #389) + `SchemaDiagram` (setas entre campos) |
| Histórico de ações | Módulo: pilha de `PlanPatch` (não o desfazer interno do canvas, que não conhece o plano) |
| Acessibilidade | Nós com semântica (pronto) + a grade como equivalente completo (obrigatória) |
