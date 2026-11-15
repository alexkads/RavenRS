# Raven Data Studio: matriz de componentes reutilizáveis

Revisão: `master` @ `0a990af3`. Todos os componentes abaixo são usados pela fachada `ravenrs::widgets` (ou `ravenrs_app*` para o framework de aplicação). Os caminhos apontam a definição, só como evidência; o módulo não importa os crates de família.

## Legenda de adequação

| Código | Significado |
|---|---|
| **R** | Existente e diretamente reutilizável |
| **C** | Existente, mas exige composição ou adaptação |
| **E** | Existente, mas precisa evoluir na biblioteca |
| **N** | Novo componente reutilizável (entra na biblioteca, genérico) |
| **M** | Nova funcionalidade específica do módulo (fica no Data Studio) |

## Estrutura da aplicação

| Requisito | Componente/API | Caminho | Evidência | Adequação | Limitação | Evolução necessária |
|---|---|---|---|---|---|---|
| Navegação entre Projetos, Conexões, Execuções | `Sidebar`, `SidebarGroup`, `SidebarItem` | `crates/ravenrs-widgets-kit/src/nav.rs:489`, `:1128`, `:1303` | `.collapsible`, `.narrow_below(px)` vira gaveta, `.rail`, Cmd/Ctrl+B, `SidebarItem::count/.badge` | **R** | Seleção manual (`selected(bool)`), sem rota | Nenhuma |
| Localização atual | `Breadcrumb` | `nav.rs:1741` | `new(iter)`, `.on_navigate(Fn(usize))` | **R** | Só texto, sem menu por nível | Nenhuma |
| Área de trabalho com painéis | `DockArea`, `DockLayout`, `DockPanel` | `crates/ravenrs-widgets-dock/src/dock/area.rs:128`, `layout.rs:341`, `area.rs:45` | Arrastar abas, teclado completo, `DockLayout::to_text/parse` (JSON versionado) | **R** | Quatro regiões (sem topo), sem janela flutuante, só o painel da frente é construído | Nenhuma para o MVP; o layout é salvo em `ProjectViewState` |
| Divisão conteúdo / inspetor / prévia | `SplitPanes`, `SplitPane` | `crates/ravenrs-widgets-base/src/extras.rs:519`, `:433` | `.sizes(Binding<Vec<Option<f32>>>)` persiste | **R** | | |
| Divisão simples de duas áreas | `Splitter` | `extras.rs:682` | `.ratio(f32)`, setas movem | **C** | Razão sem binding | Usar `SplitPanes` quando precisar persistir |
| Planos e entidades abertos | `DocumentTabs`, `DocumentTab` | `crates/ravenrs-widgets-lists/src/lists/document_tabs.rs:98`, `:30` | `.on_close`, `.on_reorder`, `.unsaved(bool)` | **C** | Id `u64` | O módulo mapeia `PlanItemRef` → `u64` |
| Alternar Canvas / Grade / Schema / DDL | `Tabs`, `SegmentedControl` | `nav.rs:52`, `crates/ravenrs-widgets-base/src/inputs.rs:861` | `Tabs::new(Binding<usize>)`, setas/Home/End | **R** | `Tabs` só constrói a aba ativa (bom: canvas não pinta escondido) | |
| Fluxo guiado | `Wizard`, `WizardStep` | `crates/ravenrs-widgets-kit/src/wizard.rs:73`, `:18` | `.check(Fn()->Result<(),String>)` por passo | **C** | Validação síncrona; sem passo opcional/desabilitado; sem `on_step_change` | O passo lê o estado já validado pelo domínio (síncrono); passo "pulável" vira passo com aviso. Evolução opcional: `WizardStep::optional()` |
| Indicador de etapas | `StepIndicator`, `IndicatorStep`, `StepState` | `wizard.rs:337`, `:117` | `Upcoming/Current/Done/Error`, `.vertical()`, `.on_select` | **R** | `StepState` sem "aviso" | Avaliar `StepState::Warning` (pendência não bloqueante) |
| Comandos | `Toolbar`, `ToolbarAction`, `SplitButton`, `IconButton` | `commands.rs:250`, `:31`, `:700`; `crates/ravenrs-widgets-base/src/button.rs:507` | Excedente vai para menu, `.hotkey(Hotkey)` | **R** | Sem ação de alternância (checked) | Alternar "mostrar só pendências" usa `SegmentedControl` ao lado |
| Ações contextuais | `ContextMenu`, `DropdownMenu`, `MenuItem`, `Popover` | `crates/ravenrs-widgets-base/src/floating.rs:24`, `overlay.rs:467`, `:370`, `:251` | `DropdownMenu` com teclado completo | **E** | `ContextMenu` só abre com o mouse e tem itens fixos | Abrir por Shift+F10/tecla Menu e itens pelo alvo. Até lá, todo item do menu de contexto também está num `DropdownMenu` do inspetor |
| Busca de comandos | `CommandPalette`, `Command` | `floating.rs:998` | Fuzzy, Cmd/Ctrl+K, `.group` | **C** | Lista estática | O módulo reconstrói a lista quando o contexto muda; "ir para campo" (dinâmico) é evolução |
| Composição de layout | `HStack`, `VStack`, `Grid`, `ScrollView`, `PropertyRow` | `crates/ravenrs-widgets-base/src/stack.rs:106`, `:782`, `:463`, `:225` | | **R** | `Grid` só colunas iguais; `ScrollView` não virtualiza; `PropertyRow` só leitura | Listas longas sempre em `VirtualList`/`DataGrid` |
| Barra de estado | `StatusBar` | `stack.rs:1205` | Centro anunciado como status | **R** | | |
| Telas administrativas (listas e formulários de projetos, conexões, execuções) | `ravenrs-app-ui`: `ListView`, `DetailView`, `ActionContainer` | `crates/ravenrs-app-ui/src/list_view.rs`, `detail_view.rs:270`, `container.rs:205` | "Lists are `DataGrid`, forms are `FormLayout`, commands are `Toolbar`" | **R** | Sem tela de importação | Nenhuma: essas telas saem do modelo |

## Dados e configuração

| Requisito | Componente/API | Caminho | Evidência | Adequação | Limitação | Evolução necessária |
|---|---|---|---|---|---|---|
| Explorar conexões, schemas, tabelas, campos | `TreeView`, `TreeNode` | `crates/ravenrs-widgets-lists/src/tree.rs:168`, `:24` | `.lazy()` + `on_expand`; virtualiza com `.height`; `.filter(text)` | **C** | Seleção única; sem colunas (tipo, contagem); sem menu de contexto; sem indicador de carregando no nó preguiçoso | Tipo e contagem no rótulo/ícone; indicador de carregando no nó (**E**, pequeno) |
| Selecionar arquivo | `FilePicker` (serviço), `ravenrs::native::OpenDialog` | `crates/ravenrs-widgets-base/src/file_picker.rs:76`, `crates/ravenrs-platform-native/src/files.rs:72` | `PickedFile { bytes }` sem caminho; `OpenDialog` devolve `PathBuf` | **C** | Arquivo inteiro em memória | Desktop: `OpenDialog` + leitura em blocos pelo adaptador. Web: limite de tamanho declarado (D-07) |
| Arrastar arquivos | `DropZone`, `FileUpload` | `crates/ravenrs-widgets-kit/src/files.rs:148`, `:422` | `.accept`, `.most_bytes`, `.choosable()` (teclado) | **C** | Bytes em memória; todas as zonas acendem no arraste | Usar só para arquivos até o limite (padrão 50 MB); acima, pedir `OpenDialog` |
| Parâmetros da conexão | `FormLayout`, `FormGroup`, `FormField`, `TextField`, `Select`, `NumberField` | `form_layout.rs:115`, `:27`; `extras.rs:1143`; `text_field.rs:28`; `overlay.rs:968`; `editors.rs:825` | `TextField::secure()`; `FormField::validate` | **R** | `FormField::validate` só para `Binding<String>` | |
| Escolher entidade e campo de destino | `Combobox`, `LookupField` | `floating.rs:1190`, `crates/ravenrs-widgets-kit/src/lookup.rs:120` | `LookupField::remote(binding, Fn(LookupRequest, LookupResponder))` descarta respostas fora de ordem | **C** | `LookupItem` só `key` + `label`; `Combobox` sem texto livre | Mostrar tipo no rótulo ("document · texto(11)"). Evolução: `LookupItem::detail` |
| Catálogos e problemas numerosos | `VirtualList` | `crates/ravenrs-widgets-lists/src/lists/virtual_list.rs:230` | Alturas variáveis medidas; `.on_visible(Range)` | **R** | Seleção única | |
| Prévia e mapeamento em grade | `DataGrid`, `GridColumn`, `GridCellStyle`, `CellEdit` | `crates/ravenrs-widgets-grid/src/grid/view.rs:56`, `model.rs:203`, `view.rs:152`, `:173` | `selection_mode(Multiple)`, `.cell_style`, `.row_detail`, `on_cell_edit -> Result` | **E** | `Value` sem decimal exato e sem data-hora; edição de célula só texto; `GridCellStyle` só cor (sem ícone) | Editor de célula por coluna (Select/Lookup na célula) e ícone no estilo da célula. Até lá: coluna `template` com ícone + texto; edição pelo inspetor |
| Dados paginados | `RemoteRows`, `GridRequest`, `GridResponder`, `GridPage` | `crates/ravenrs-widgets-grid/src/grid/remote.rs:214`, `:28`, `:113` | `offset/limit`, `sort`, `filters` (texto cru), `search`, `after` (por chave), `wants_total`; LRU; descarte por geração | **C** | Backend interpreta o mini-dialeto do filtro; sem agrupamento remoto; exportar tudo indisponível | Serviço do módulo traduz `GridRequest` → leitura do adaptador. A tradução `GridRequest → Criteria` existe privada em `ravenrs-app-ui/src/list_view.rs:194`: **E**, tornar pública num lugar compartilhado |
| Condições de origem | `FilterBuilder`, `FilterField` | `crates/ravenrs-widgets-kit/src/filter_builder.rs:325`, `:35` | Lê e escreve texto `ravenrs-criteria`; grupos E/OU | **R** | Sem "entre" nem "termina com"; sem tipo data-hora | |
| Transformações e validações por expressão | `ExpressionEditor` | `crates/ravenrs-widgets-kit/src/expression_editor.rs:59` | Valida com `Criteria::parse` e `ravenrs_criteria::validate`; catálogo de funções | **C** | Sem autocomplete, sem destaque, sem checagem de tipo | Funções do módulo registradas com `Environment::register` aparecem no catálogo |
| Valores específicos | `MaskedField`, `NumberField`, `TextArea` | `editors.rs:1180`, `:825`, `:152` | Máscara `0/a/*`; separadores pelos `Labels` | **R** | | |

## IA, revisão e execução

| Requisito | Componente/API | Caminho | Evidência | Adequação | Limitação | Evolução necessária |
|---|---|---|---|---|---|---|
| Conversa | `Chat`, `ChatMessage`, `ChatAuthor` | `crates/ravenrs-widgets-chat/src/chat.rs:356`, `:265`, `:255` | `assistant_writing` (streaming), `.card(Fn()->AnyView)`, `.busy` | **C** | É interface, não agente. `ChatAuthor` só `Person`/`Assistant`; lista não virtualizada; sem parar geração | O agente é do módulo (**M**). Botão parar fora do `Chat`. Evolução: `Chat::on_stop` e lista virtualizada |
| Sugestões aceitáveis na conversa | `ChatMessage::card` + `Badge` + `Button` | `chat.rs` | Card com botões sob a mensagem | **C** | | Card "Proposta" com operações e Aceitar/Recusar (**M**) |
| Referências anexadas | `AttachmentStrip`, `Attachment` | `crates/ravenrs-widgets-chat/src/conversation.rs:117` | `.on_remove` | **R** | | |
| Explicações formatadas | `RichText` | `crates/ravenrs-widgets-chat/src/rich_text.rs:234` | `*negrito*`, listas, citações, código | **C** | Não é Markdown (sem títulos, sem tabelas) | Tabelas na explicação viram `DataGrid` pequena no card |
| Seleção de modelo | `ModelPicker` | `crates/ravenrs-widgets-kit/src/model_picker.rs:129` | Baixar/testar/usar/remover modelos locais | **C** | Não escolhe provedor; não comprova integração com provedores | Usado nas Configurações de IA para modelos locais; provedor remoto (Evolução) num `Select` à parte |
| Exibição de DDL | `CodeEditor`, `CodeDiagnostic`, `CodeLanguage` (feature `code`) | `crates/ravenrs-widgets-chat/src/code_editor.rs:95`, `:31`, `:19` | `.read_only`, `.diagnostics`, `.line_numbers` | **E** | `CodeLanguage` só `Plain` e `Rust`; sem destaque plugável; sem rolagem horizontal | MVP: `Plain`. Fase 0/Evolução: `CodeLanguage::Sql` |
| Comparação de versões (DDL, plano) | nenhum | | O único `CompareView` (`crates/ravenrs-widgets-imaging/src/imaging/viewing.rs:79`) compara imagens | **N** | | `TextDiff` (ver lacunas) |
| Confirmações | `ConfirmDialog`, `Dialog` | `commands.rs:814`, `overlay.rs:1287` | `ask_typing(…, palavra, …)` exige digitar | **R** | Uma ação; sem callback de cancelar | |
| Situações | `Badge`, `Tone`, `Alert` | `crates/ravenrs-widgets-base/src/display.rs:62`, `:22`, `:516` | `Badge::icon`; `Tone::{Neutral, Accent, Success, Warning, Danger}` | **R** | `Alert` com uma ação, não dispensável | Sempre `Badge` com ícone + texto |
| Progresso | `ProgressBar`, `MultiProgress`, `Spinner` | `display.rs:317`, `crates/ravenrs-widgets-kit/src/status.rs:728`, `extras.rs:19` | `MultiProgress::meter(label, done, total)` | **C** | Sem modo indeterminado (`Spinner` cobre); `MultiProgress` sem estado por item | Linhas por entidade compostas com `ProgressBar` + `Badge` (**M**) |
| Resumos | `Stat`, `StatusBar` | `display.rs:720`, `stack.rs:1205` | `.delta_toned` | **R** | | |
| Estados vazios e carregamento | `EmptyState`, `Skeleton` | `status.rs:505`, `display.rs:465` | | **R** | | |
| Notificações | `Toast`, `Toaster`, `Toasts` | `floating.rs:522`, `:747` | `show_with(ToastExtras::action)` | **R** | Transitório | Só confirmação de ação; problema persistente nunca só em toast |
| Lista de problemas persistente | `Diagnostics`, `DiagnosticsHost` | `crates/ravenrs-widgets-kit/src/diagnostics.rs:69`, `:190` | "A panel any desktop app can open to see its GPU and what its frames cost" | **Não serve** | Ferramenta de desenvolvedor | `IssueList` (**N**) |

## Canvas de mapeamento

| Requisito | Componente/API | Caminho | Evidência | Adequação | Limitação | Evolução necessária |
|---|---|---|---|---|---|---|
| Editor de nós e conexões | `FlowCanvas`, `FlowGraph`, `FlowNode`, `FlowPort`, `FlowEdge`, `FlowViewport`, `FlowSelection`, `FlowKind`, `FlowValidator` | `crates/ravenrs-widgets-charts/src/flow.rs:210`; `crates/ravenrs-graphics/src/flow.rs` | Ver seção abaixo | **E** | Ver #238 | Ver [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md) |
| Desenho livre | `Canvas` | `crates/ravenrs-widgets-base/src/canvas.rs:28` | Closure que acrescenta à cena | **Não serve sozinho** | Sem teste de acerto, foco ou semântica | Não usado: `FlowCanvas` já resolve |
| Zoom e minimapa | `ZoomView`, `ZoomControl`, `ZoomPanView`, `Minimap` | `crates/ravenrs-widgets-base/src/zoom.rs:36`, `:184`; `crates/ravenrs-widgets-imaging/src/imaging.rs:1574`, `:1840` | "The zoom of a picture" | **Não serve** | Feitos para imagem | `FlowCanvas` tem `FlowViewport`, controles e minimapa próprios |

### O que o `FlowCanvas` já atende

| Necessidade | Situação | Evidência |
|---|---|---|
| Nós com portas | ✅ portas com lado, direção, rótulo e tipo | `FlowPort` (`ravenrs-graphics/src/flow.rs:60`) |
| Nós com **lista de campos** | 🟡 portas podem ser uma por campo, mas o nó desenha ícone, legenda, título e subtítulo, não uma tabela de campos | doc do módulo (`flow.rs:1-9`) |
| Conexões selecionáveis | ✅ alcance de 6 px | `EDGE_REACH` |
| Arraste e criação de ligações com validação | ✅ portas que aceitam acendem; recusa em vermelho com motivo | `FlowValidator` |
| Teste de acerto | ✅ nós são caixas com foco; portas com área de 24 px | `PORT_HIT` |
| Coordenadas tela ↔ documento | ✅ | `FlowViewport::to_screen` / `to_content` |
| Seleção múltipla | ✅ Shift e retângulo (Shift-arrastar) | `marquee` |
| Pan e zoom | ✅ 25% a 200%, âncora no ponteiro, `+ - 0` | `ZOOM_RANGE` |
| Minimapa | ✅ | `.minimap(true)` |
| Agrupamento e recolhimento | ⬜ | #238 "Nós-grupo" |
| Organização automática | ⬜ | #238 "Organizar sozinho" |
| Busca e foco | 🟡 foco por nó existe; busca não | |
| Redução de conexões visíveis | ⬜ | #238 desempenho; filtro é do módulo |
| Navegação por teclado | ✅ nós, pan e portas (Enter inicia e termina a ligação, Esc desiste) | PR #389 |
| Histórico de ações | ✅ `FlowHistory` (PR #389); no Data Studio o desfazer é a pilha de `PlanPatch` | PR #389 |
| Acessibilidade | 🟡 nós com semântica; equivalência tabular é do módulo (grade) | |
