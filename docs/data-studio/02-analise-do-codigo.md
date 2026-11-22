# Raven Data Studio: análise do código

## Revisão examinada

| | |
|---|---|
| Repositório | `alexkads/RavenRS` |
| Branch | `master` |
| Commit | `0a990af3ffa5f3ab9c8296f924ffc2375ccbe473` (Merge pull request #380) |
| Data da análise | 15/11/2026 |
| Modo | Somente leitura: código, READMEs, `docs/`, exemplos. Nenhum build, nenhum teste, nenhuma dependência instalada. |
| Levantamento inicial | O mesmo commit `0a990af3`, que era o `origin/master` no momento da análise. Todos os pontos foram revalidados nele. |

### Depois da revisão analisada

Antes de juntar esta documentação, o `master` avançou para `513577b5d` com o PR #389 (parte da #238), que acrescentou ao `FlowCanvas`:

- desfazer e refazer (`FlowHistory` em `ravenrs-graphics`, compartilhável por `FlowCanvas::history`), até 100 mudanças, guardando o diagrama inteiro;
- copiar, recortar e colar nós com as conexões entre eles, com ids novos;
- `FlowGraph::to_json` / `from_json`, com conferência de ids e portas (`FlowJsonError`);
- mover a ponta de uma conexão para outra porta;
- portas com foco e nome acessível, conexão pelo teclado (Enter, Enter, Escape).

O resto desta análise continua valendo; os pontos acima já estão corrigidos na matriz e nas lacunas. No Data Studio o desfazer continua sendo a pilha de `PlanPatch`: o `FlowHistory` desfaz o diagrama inteiro, inclusive mudanças feitas pelo app entre duas edições, então o canvas de mapeamento não o usa para mudanças do plano.

O servidor `ravenrs-mcp` não estava instalado no ambiente e não foi instalado. As consultas que ele responderia (`component_api`, `expression_functions`, `app_contracts`…) foram feitas lendo as fontes que ele embute.

## Como as APIs chegam à aplicação

- Todos os widgets usados aqui são reexportados por `crates/ravenrs-widgets/src/lib.rs` (linhas ~113 a 264). A fachada `ravenrs` expõe tudo por `ravenrs::widgets::*` (`crates/ravenrs/src/lib.rs:23`) e `ravenrs::prelude::*` (`:51`).
- Os crates de família (`ravenrs-widgets-base`, `-kit`, `-dock`, `-lists`, `-grid`, `-chat`, `-charts`, `-imaging`) declaram seus módulos `#[doc(hidden)]` e dizem não ser API estável. **O Data Studio depende só da fachada.**
- **Feature `code`:** `CodeEditor`, `CodeDiagnostic` e `CodeLanguage` só existem com a feature `code`, que **não é padrão** (`crates/ravenrs/Cargo.toml:17`: `default = ["decode", "charts", "flow", "lottie"]`). O módulo precisa habilitá-la.
- `FlowCanvas` está na feature `flow`, que é padrão.
- `ravenrs-app` e `ravenrs-app-ui` **não** estão na fachada `ravenrs` (`docs/APP-FRAMEWORK.md`: "Nothing here is in the `ravenrs` facade yet"). O módulo depende deles diretamente.

## Achados principais

### Pontos fortes que mudam o desenho

1. **Existe um editor de diagramas: `FlowCanvas`** (`crates/ravenrs-widgets-charts/src/flow.rs:210`, modelo em `crates/ravenrs-graphics/src/flow.rs`). Ele não estava na matriz inicial. Tem nós com portas, conexões selecionáveis, criação de ligação por arraste com validação e motivo da recusa (`FlowValidator`), seleção múltipla (Shift e retângulo), pan, zoom com âncora no ponteiro, conversão tela↔documento (`FlowViewport::to_screen` / `to_content`), minimapa, controles, grade com encaixe, trava, teclado (setas, Delete, Escape, `+ - 0`) e leitor de tela por nó. O modelo é serializável. A continuação (issue #238) listava desfazer/refazer, copiar/colar, religar pela ponta, portas pelo teclado, JSON, organizar sozinho, grupos, redimensionar, `View` dentro do nó e desempenho com 1.000 nós. Depois da revisão analisada, o PR #389 entregou a metade de edição e dados (ver abaixo); continuam abertos organizar sozinho, grupos, redimensionar, `View` dentro do nó e a medição com 1.000 nós. **O canvas de mapeamento deve ser construído sobre ele, não do zero.**
2. **`DataGrid` remoto** (`crates/ravenrs-widgets-grid/src/grid/remote.rs`) já pede páginas com ordenação, filtros, busca, paginação por chave (`after`) e total, com cache LRU e descarte de respostas antigas. É a base da prévia sem carregar o conjunto.
3. **`ExpressionEditor` valida de verdade** com `ravenrs-criteria` (sintaxe com linha e coluna, nomes desconhecidos) e mostra o catálogo de funções com assinaturas traduzidas.
4. **`ravenrs-criteria`** tem 129 funções escalares e 26 agregados, validação de nomes, avaliação sobre `Record` e tradução parcial para SQL em quatro dialetos.
5. **`EntityInfo` pode ser montada em tempo de execução** e o SQLite já tem `entities_from_schema`.
6. **Segurança e auditoria do ravenrs-app existem** (`Role`, `Permissions`, `SecuredObjectSpace`, `AuditTrail` com `AuditMode::CreationOnly` pensado para cargas grandes).
7. **Leitor de CSV robusto existe** em `ravenrs-sheet` (codificação, separador, inferência de datas), mas preso ao crate da planilha.

### Limitações que mudam o desenho

1. **Não há destaque de SQL nem comparação de texto.** `CodeLanguage` tem só `Plain` e `Rust` (`crates/ravenrs-widgets-chat/src/code_editor.rs:19`), sem API pública para outro destaque. O único `CompareView` compara imagens.
2. **`Diagnostics` / `DiagnosticsHost` não servem para erros de importação.** São o painel do desenvolvedor para GPU e custo de frames (`crates/ravenrs-widgets-kit/src/diagnostics.rs:1`), já instalado pela plataforma.
3. **`ModelPicker` é um gerenciador de modelos locais** (baixar, testar, usar, remover), não um seletor de provedor.
4. **`Chat` não é agente**: `ChatAuthor` tem só `Person` e `Assistant`; a lista não é virtualizada; não há parar geração.
5. **`Canvas`** é só desenho (closure que acrescenta à cena): sem teste de acerto, sem foco, sem semântica. **`ZoomView`, `ZoomControl`, `ZoomPanView` e `Minimap`** são feitos para imagem ("The zoom of a picture", `crates/ravenrs-widgets-base/src/zoom.rs:1`). Para nós e conexões, o `FlowCanvas` já tem seu próprio viewport e minimapa; os de imagem não são usados.
6. **Arquivos chegam inteiros em memória.** `FilePicker`, `DropZone` e `FileUpload` entregam `PickedFile { bytes: Rc<[u8]> }`, sem caminho. No desktop há `OpenDialog` nativo com `PathBuf`; na web, não.
7. **Valores da grade são pobres para dados importados.** `Value` da grade tem `Empty`, `Text`, `Number(f64)`, `Bool`, `Date`: sem decimal exato, sem data e hora, sem inteiro de 64 bits. Edição de célula é só texto.
8. **Persistência do ravenrs-app não é motor de carga.** Chave numérica por `MAX + 1` por processo; sem inserção em lote; DDL só cria tabela e adiciona coluna (sem índice, FK, `UNIQUE`, `NOT NULL`); `commit()` não roda as regras de validação.
9. **IA local limitada.** `ravenrs-ai` roda modelos de 0,6 a 8 bilhões de parâmetros em CPU, um turno por pergunta, sem embeddings, sem provedor remoto, com chamada de ferramenta por JSON solto.
10. **Não há cliente MCP.** `ravenrs-mcp` é servidor de desenvolvimento.

## Persistência

### `ravenrs-app-sql`

- `trait SqlBackend` (`crates/ravenrs-app-sql/src/lib.rs:185`): `dialect`, `column_type`, `columns_of`, `execute`, `batch`, `query`.
- `SqlBuilder::managed` (`:320`) faz `CREATE TABLE IF NOT EXISTS` e `ALTER TABLE ADD COLUMN`; `SqlBuilder::migration(id, sql)` (`:371`) roda SQL escrito à mão com controle de migrações.
- Filtro que não traduz para SQL faz ler a tabela inteira para memória (`:634-650`).
- `save` consulta existência por registro (`:508`); cada mudança é um `INSERT` ou `UPDATE` separado (`:542-565`), dentro de `BEGIN … COMMIT` (`:756-776`).

### Geração de chaves no PostgreSQL

`crates/ravenrs-app-postgres/README.md:15`:

> "Numbered keys come from the highest key in the table, so only one process should create records on a table."

**Continua valendo** em `0a990af3`: `SqlObjectSpace::next_key` (`crates/ravenrs-app-sql/src/lib.rs:524-540`) faz `SELECT COALESCE(MAX(col), 0) + 1` e guarda o próximo número num mapa por processo. Não há sequence, identity nem `RETURNING`. `docs/APP-PARITY.md:121-127` registra que o segundo commit concorrente falha (não grava registro errado) e aponta a issue #241.

**Consequências para o planejamento:**

- As entidades administrativas do módulo usam `FieldKind::Guid` como chave, para o módulo poder rodar com o servidor e com a interface ao mesmo tempo.
- O motor de importação **não** grava no destino pelo `ObjectSpace`. As tabelas criadas pelo Data Studio usam identidade gerada pelo banco de destino, e o adaptador lê o id de volta com `RETURNING`.
- Se um app RavenRS for o destino e continuar usando o `ObjectSpace` para criar registros nessas mesmas tabelas, o `MAX + 1` dele continua funcionando (lê o maior existente), mas **não pode** rodar ao mesmo tempo que a importação. A Revisão final avisa isso quando o destino é uma tabela gerida por um app RavenRS.

### `ravenrs-app-sqlite` e `ravenrs-app-postgres`

- SQLite: `entities_from_schema` (`crates/ravenrs-app-sqlite/src/lib.rs:107`) lê `sqlite_master`, `PRAGMA table_info` e `foreign_key_list`. Heurística de tipos simples.
- PostgreSQL: `PostgresBackend::connect_with` com TLS; `columns_of` por `information_schema.columns` (só nomes); sem `entities_from_schema` (`docs/APP-FRAMEWORK.md:504`).

## Expressões

- `Criteria::parse` (`crates/ravenrs-criteria/src/ast.rs:150`), `Scope::evaluate` / `matches` (`eval.rs:232-292`), `validate(&Criteria, &Schema) -> Vec<Problem>` (`validate.rs:100`), `to_sql(&Criteria, Dialect)` (`sql.rs:71`), `functions::catalog()` (`functions.rs:426`), `Environment::register` (`eval.rs:153`).
- `Problem` só tem `UnknownField`, `UnknownCollection`, `UnknownParameter`, `UnknownFunction` (`validate.rs:77`): **não há checagem de tipos**.
- Sem JOIN nem subconsulta; `[A.B]` vira identificador qualificado. Agregados sobre coleções e a maior parte das funções não traduzem para SQL.
- As assinaturas em pt-BR (commit `01faca921`) estão em `crates/ravenrs-app-i18n/texts/pt-BR/widgets.json` (`widgets.expression_signature.*`) e são lidas por `Labels::expression_signature`; os nomes das funções continuam em inglês.

## Exportação

`ravenrs-grid-export` (`crates/ravenrs-grid-export/src/lib.rs`, 246 linhas): `export(&GridTable, GridExportFormat::{Xlsx, Pdf, Csv}, suffix)`. Monta um documento de relatório com **todas as linhas em memória** (`GridTable.rows: Vec<Vec<Value>>`) e passa pelo motor de relatórios. Serve para exportar o que a grade mostra (ex.: lista de problemas filtrada). Não serve para o relatório de execução completo (que é um relatório próprio no `ravenrs-report`) nem para o arquivo fiel dos rejeitados (que o motor escreve a partir do registro original).

No modo remoto do `DataGrid`, `GridCurrent::table` (exportar tudo) devolve `None`: exportar a grade de rejeitados inteira exige que o serviço do módulo gere o arquivo.

## IA e MCP

- `ravenrs-ai`: `LocalEngine::load` / `generate` (`engine.rs:192,236`), `LocalAssistant::write` / `answer` (`assistant.rs:102-157`), `ModelManager`, `Machine`, `LocalModel::fits`, `tool_rules` / `read_tool_call` (`tools.rs:35-42`). Catálogo com Qwen3, Qwen2.5, Llama 3.2, Gemma 3, LFM3, Phi-3, Mistral 7B, quantizados, em candle sobre CPU.
- `ravenrs-mcp`: servidor stdio, ferramentas em `src/server.rs:144-354`, sem efeitos colaterais. Detalhes e separação em [10-mcp-e-conectores.md](10-mcp-e-conectores.md).

## Acessibilidade, idiomas e estilo

- `ravenrs-app-a11y` (`src/lib.rs:28-34`): `AccessibleText`, `Announcer`, `LiveRegion`, `Urgency`, `AccessWords`, `Audit` (nomes, foco, contraste AA).
- `ravenrs-app-i18n`: catálogo JSON plano em `texts/{en,pt-BR}/*.json`; `Catalog::builtin`, `merge_json`, `texts(Language)`; só `English` e `BrazilianPortuguese`.
- Widgets: `Labels` (~400 campos, `Labels::from_texts(&Texts)`) e `Localized::new(labels, view)`; `ModelPicker` usa `ModelPickerLabels` à parte.
- Estilo: `ravenrs-style` (`Theme`, `Palette`, `Semantic`, `Typography`, `Spacing`, `Radii`, `Elevation`, `MotionTokens`), tokens em `docs/design-system/tokens.json`.
- `responsive(|size| view)` recebe o tamanho do viewport (`crates/ravenrs-widgets-base/src/view.rs:100`); `Sidebar::narrow_below` dá a troca para gaveta.

## O que não foi verificado

- Desempenho real de `FlowCanvas` com dezenas de nós de muitos campos (a #238 pede medir com 1.000 nós).
- Qualidade de planejamento dos modelos locais do `ravenrs-ai` para gerar `PlanPatch` em JSON. Precisa de prova antes de depender dela (decisão D-03).
- Comportamento do `DataGrid` remoto com páginas lentas de um banco externo (só há exemplos com dados locais).
