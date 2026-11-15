# Raven Flows

Exemplo desktop de automação de fluxos por nós, local primeiro, para validar e fechar lacunas dos componentes e da infraestrutura do RavenRS.

O exemplo existe para exercitar, **com uso real**, o máximo de componentes da biblioteca: 160 dos 181 do catálogo e 21 widgets dos motores (filmes, relatórios, planilha e camada de aplicação), como mostra a [cobertura de componentes](#cobertura-de-componentes). Ele também traz os próprios treinamentos em filme vetorial narrado pelo Kokoro e tem nós que geram vídeo e nós que chamam o Codex CLI e o Claude CLI.

Ainda não há código: esta pasta guarda o levantamento e fica fora dos membros do workspace até o código começar.

## Estado em resumo

**Legenda:** ✅ componente ou contrato reutilizável disponível para o recorte descrito; 🟡 base existente, mas falta especialização ou integração; ⬜ capacidade não encontrada nas buscas indicadas. Nenhum estado significa que a aplicação já esteja implementada ou validada em execução.

As contagens correspondem às linhas das tabelas abaixo, sem ponderação por esforço.

| Área | ✅ | 🟡 | ⬜ |
|---|---:|---:|---:|
| 1. Editor de fluxo | 11 | 2 | 0 |
| 2. Painel do nó | 4 | 4 | 0 |
| 3. Expressões | 0 | 4 | 1 |
| 4. Motor de execução | 8 | 1 | 0 |
| 5. Gatilhos | 0 | 5 | 2 |
| 6. Nós de integração | 1 | 6 | 0 |
| 7. Execuções | 3 | 2 | 0 |
| 8. Credenciais | 0 | 2 | 3 |
| 9. Gestão | 1 | 5 | 1 |
| 10. Usuários e permissões | 1 | 3 | 0 |
| 11. Servidor | 2 | 1 | 1 |
| 12. Experiência geral | 6 | 1 | 0 |
| 13. Vídeo e treinamento | 9 | 9 | 2 |
| 14. Nós de agente por CLI | 0 | 9 | 6 |
| **Total** | **46** | **54** | **16** |

**Regra de construção:** toda peça visual ou de infraestrutura ausente deve entrar primeiro na biblioteca, no componente responsável. O exemplo apenas compõe APIs públicas e fornece os dados e as políticas do produto.

Os nomes de crates novas e os identificadores `L01`–`L27` são propostas deste levantamento. As áreas 13 e 14 listam suas lacunas nas próprias seções.

## Levantamento por área

Os caminhos partem da raiz do repositório. As referências apontam para tipos ou funções públicas abertos e lidos. Nas linhas com ausência, `B1`–`B9` identificam as buscas documentadas ao final.

### 1. Editor de fluxo

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Canvas, nós, portas, conexões e várias saídas nomeadas | ✅ | `FlowCanvas::new`, `crates/ravenrs-widgets-charts/src/flow.rs:642`; `FlowPort`, `crates/ravenrs-graphics/src/flow.rs:74` | App declarar portas de verdadeiro, falso, erro e demais tipos; aplicar validação semântica | APP: catálogo e regras de conexão |
| Nó desativado | ✅ | `FlowNodeStatus::Disabled`, `crates/ravenrs-graphics/src/flow.rs:251`; `NodeDef::disabled`, `crates/ravenrs-flows/src/definition.rs:200`: o nó desativado passa a primeira entrada para a primeira saída | App escolher se mostra o atalho de desativar | APP |
| Fixar dados de um nó | 🟡 | `NodeDef::pin`, `crates/ravenrs-flows/src/definition.rs:212`, e `Mode::Test`, `crates/ravenrs-flows/src/catalog.rs:106`: no teste, o nó fixado entrega os itens guardados sem rodar | Falta a marca visual no canvas (#528) e editar os itens fixados no inspetor (#531) | BIBLIOTECA: #528, #531 |
| Notas adesivas | ✅ | `FlowShape::Note`, `crates/ravenrs-graphics/src/flow.rs:124`; `FlowCanvas::content`, `crates/ravenrs-widgets-charts/src/flow.rs:859` | App oferecer criação, texto e cor; definir se usa título simples ou conteúdo composto | APP, usando o canvas |
| Zoom, pan, encaixe e minimapa | ✅ | `FlowViewport`, `crates/ravenrs-widgets-charts/src/flow.rs:289`; `FlowCanvas::controls` e `minimap`, mesmo arquivo:884 e :890 | Configurar limites e persistir viewport do usuário | APP |
| Organização automática | ✅ | `FlowLayout::positions` e `apply`, `crates/ravenrs-graphics/src/flow/layout.rs:94` e :150 | Escolher direção e espaçamentos; organização geométrica não define ordem de execução | APP |
| Desfazer e refazer do diagrama | ✅ | `FlowHistory::record`, `undo` e `redo`, `crates/ravenrs-graphics/src/flow.rs:1319`, :1332 e :1339 | Para alterações de parâmetros fora do grafo, histórico de documento completo | BIBLIOTECA: `L03`; APP: comandos do documento |
| Copiar, colar e importar JSON | ✅ | `FlowNode::data` e `FlowGraph::data`, `crates/ravenrs-graphics/src/flow.rs:322` e :760; `NODE_REFS` e `FlowGraph::insert_with`, `flow.rs:770` e :1123; `FlowGraph::from_json` com migração, `flow.rs:1178`; `FlowDefinition::to_graph` e `from_graph`, `crates/ravenrs-flows/src/graph.rs:42` e :89: a configuração de cada nó viaja no cartão, e colar renomeia o que um nó cita de outro | App listar em `NODE_REFS` onde os dados citam outros nós | APP |
| Busca rápida ao soltar conexão no vazio | 🟡 | `FlowCanvas::quick_create`, `crates/ravenrs-widgets-charts/src/flow.rs:838`: as setas e Cmd/Ctrl+Enter criam o próximo nó já ligado. Soltar uma conexão no vazio não abre busca; busca `B1`: `on_connect_end`, `connection_drop`, `connect_to_empty` | Evento público com origem e posição ao soltar no vazio, busca no catálogo compatível com a porta e criação e conexão numa só transação | BIBLIOTECA: canvas, `L01` |
| Catálogo lateral por categoria | ✅ | `FlowToolbox`, `crates/ravenrs-widgets-charts/src/flow_toolbox.rs:60`: grupos, busca, arrastar para o canvas e Enter pelo teclado, ligado por `FlowCanvas::arrivals`, `crates/ravenrs-widgets-charts/src/flow.rs:697` | App fornecer categorias, descrições e os modelos de nó do catálogo | APP, usando a caixa de ferramentas |
| Grupos, conteúdo próprio, validação e religação | ✅ | `FlowGraph::group`, `crates/ravenrs-graphics/src/flow.rs:1033`; `FlowCanvas::content`, `validate` e `on_change`, `crates/ravenrs-widgets-charts/src/flow.rs:859`, :772 e :794; `FlowChange::Reconnect`, mesmo arquivo:439 | Conectar os guards às regras e permissões do documento | APP |
| Verificar o fluxo e apontar o erro no nó | ✅ | `FlowCanvas::checks` e `FlowIssue`, `crates/ravenrs-widgets-charts/src/flow.rs:732` e :353 | App escrever as regras (nó sem credencial, parâmetro obrigatório vazio, gatilho ausente) | APP |
| Pontos de dobra manuais e exportação visual | ✅ | `FlowEdge::waypoint`, `crates/ravenrs-graphics/src/flow.rs:699`; `FlowExportFormat` (PNG, SVG e PDF), `crates/ravenrs-graphics/src/flow/picture.rs:61`; `FlowCanvas::on_export`, `crates/ravenrs-widgets-charts/src/flow.rs:689` | App decidir onde gravar o arquivo exportado | APP |

O histórico atual guarda até 100 estados completos do `FlowGraph`. Ele não constitui histórico de versões publicadas nem cobre automaticamente os parâmetros mantidos pelo app.

### 2. Painel do nó

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Formulário por metadados: texto, número, booleano e opções | ✅ | `FieldKind`, `crates/ravenrs-app/src/entity.rs:25`; `EntityForm::new`, `crates/ravenrs-app-ui/src/form.rs:71` | App declarar metadados, validação, valores iniciais e campos do tipo de nó | APP |
| Coleções, coleções fixas e grupos repetíveis | 🟡 | `FieldKind::Collection`, `crates/ravenrs-app/src/entity.rs:25`; `EntityForm::new`, `crates/ravenrs-app-ui/src/form.rs:71` | Coleção de entidades não oferece diretamente uma coleção embutida de parâmetros, com grupos fixos, adicionar/remover e cancelamento atômico | BIBLIOTECA: esquema e editor, `L05` |
| Opções dependentes de outro campo | 🟡 | `FieldInfo::lookup_filter`, `crates/ravenrs-app/src/entity.rs:358` | Filtragem de referência existe; faltam opções carregadas por conector, dependências declaradas, cache, carregamento, erro e invalidação de seleção | BIBLIOTECA: `L05`, `L16` |
| Entrada, parâmetros e saída em painéis redimensionáveis | ✅ | `DockArea::new`, `crates/ravenrs-widgets-dock/src/dock/area.rs:304` | Compor os três painéis, seleção e abas de apresentação | APP |
| Propriedades visuais do nó (título, tipo, forma, estado) | ✅ | `FlowInspector`, `crates/ravenrs-widgets-charts/src/flow_inspector.rs:61` | Não cobre os parâmetros do tipo de nó, que dependem de `L05` | APP, ao lado do formulário de parâmetros |
| Dados em tabela, JSON e esquema | 🟡 | `DataGrid::new`, `crates/ravenrs-widgets-grid/src/grid/view.rs:331`; `TreeView::new`, `crates/ravenrs-widgets-lists/src/tree.rs:199` | Inspector reutilizável de JSON aninhado, tipos, ausente versus nulo, arrays, busca e alternância coerente entre modos | BIBLIOTECA: `L06`, `L08` |
| Arrastar campo da entrada para parâmetro e gerar expressão | 🟡 | `DragPayload::new`, `crates/ravenrs-widgets-base/src/context.rs:165`; `BuildCx::drag_source` e `drop_target`, mesmo arquivo:1701 e :1710 | Payload com caminho tipado, origem do item, escape correto e editor que aceite a inserção; manter alternativa por teclado | BIBLIOTECA: inspector e editor, `L07` |
| Executar somente o nó | ✅ | `Executor::run_node`, `crates/ravenrs-flows/src/run.rs:355`: roda o nó com o que chegou a ele numa execução anterior | App ligar o botão no painel do nó | APP |

`ravenrs-app-spec` descreve estruturas e atributos para entidades; `KindSpec`, em `crates/ravenrs-app-spec/src/lib.rs:102`, não é um esquema completo de parâmetros dinâmicos de conectores. A extensão deve preservar a geração existente.

### 3. Expressões

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Expressões delimitadas por `{{ }}` | 🟡 | `ExpressionEditor::new`, `crates/ravenrs-widgets-kit/src/expression_editor.rs:78` | A linguagem atual usa campos `[campo]` e parâmetros `?nome`; falta interpolação em texto, escape e diagnóstico por trecho | BIBLIOTECA: `ravenrs-criteria`, `L09` |
| `$json`, nós anteriores, variáveis e datas | 🟡 | `Record`, `crates/ravenrs-criteria/src/eval.rs:18`; `Environment::at` e `parameter`, mesmo arquivo:124 e :130; `Scope::row`, mesmo arquivo:237 | Contexto de fluxo com namespaces, referência estável a nós e seleção explícita de item; datas já têm base, mas o contexto precisa fixar relógio e fuso | BIBLIOTECA: `L09`, `L10`; APP: nomes e valores |
| Destaque, diagnóstico e autocompletar contextual | 🟡 | `ExpressionEditor::field` e `parameter`, `crates/ravenrs-widgets-kit/src/expression_editor.rs:83` e :95; `CodeLanguage`, `crates/ravenrs-widgets-chat/src/code_editor.rs:21` | O editor oferece catálogo e inserção; falta completion no cursor e destaque do dialeto do fluxo. `CodeLanguage` oferece texto simples e Rust | BIBLIOTECA: `L09`, `L26` |
| Prévia do resultado | 🟡 | `Scope::evaluate`, `crates/ravenrs-criteria/src/eval.rs:283` | Avaliação de amostra integrada ao editor, erro distinguível de nulo, item selecionado e orçamento de avaliação | BIBLIOTECA: `L09` |
| Nó de código em linguagem de script | ⬜ | Não encontrado runtime geral; busca `B4`: `rhai`, `quickjs`, `boa_engine`, `deno_core`, `mlua` | Runtime isolado, limites de CPU/memória/tempo, cancelamento, diagnóstico e conversão de itens | BIBLIOTECA: crate proposta `ravenrs-script`, `L15`; editor em `L26` |

As macros de planilha são listas declarativas de comandos: `parse_macro`, em `crates/ravenrs-sheet/src/engine/macros/mod.rs:29`, trabalha sobre esse modelo específico. Isso não comprova uma linguagem geral para o nó de código.

O `Environment` de expressões usa `Rc` e estado interno sem segurança entre threads. Cada trabalhador deve criar seu próprio ambiente, sem compartilhar essa instância.

### 4. Motor de execução

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Planejamento topológico e validação de ciclos | ✅ | `Plan::compile`, `crates/ravenrs-flows/src/plan.rs:97`: confere tipos, portas e parâmetros obrigatórios, ordena por dependência e recusa ciclo sem nó de laço; `Plan::back_connections`, `plan.rs:187` | Seguir as voltas do laço fica com os operadores (#534) | BIBLIOTECA: #534 |
| Lista de objetos passando entre nós | ✅ | `Item` e `Item::derive`, `crates/ravenrs-flows/src/item.rs:44` e :66: JSON, arquivos e de onde veio cada item; `Executor::run`, `crates/ravenrs-flows/src/run.rs:330` | App definir o conteúdo dos itens dos seus nós | APP |
| Ramos, merge e laço em lotes | ✅ | `If`, `Switch`, `Merge` e `Batches`, `crates/ravenrs-flows/src/nodes.rs:153`, :177, :220 e :282; o executor dá a volta do laço (`Plan::loop_body`, `crates/ravenrs-flows/src/plan.rs:201`) | App escolher quais nós oferece no catálogo | APP |
| Espera e retomada após reinício | ✅ | `Executor::resume`, `crates/ravenrs-flows/src/run.rs:339`, com `Delay` e `WaitEvent`, `crates/ravenrs-flows/src/nodes.rs:646` e :669; `Runtime::execute` e `signal`, `crates/ravenrs-flows-runtime/src/runtime.rs:293` e :417: retoma do último passo gravado e pergunta ao nó que estava rodando se a chamada aconteceu | Um laço é um passo só: parado no meio, dá a volta de novo desde o começo | BIBLIOTECA: `ravenrs-flows-runtime` |
| Continuar, parar, saída de erro e fluxo de erro | 🟡 | `ERROR_PORT`, `crates/ravenrs-flows/src/catalog.rs:18`; `ErrorPolicy::Route`, `crates/ravenrs-flows/src/definition.rs:38`; `Item::failed`, `crates/ravenrs-flows/src/item.rs:77` | Falta o fluxo de erro (um fluxo inteiro chamado quando outro falha), que depende da execução durável | BIBLIOTECA: #535 |
| Novas tentativas e tempo limite | ✅ | `Retry`, `crates/ravenrs-flows/src/definition.rs:46`, por tipo de falha (`Failure`, `crates/ravenrs-flows/src/catalog.rs:311`); `NodeKind::reconcile`, `catalog.rs:366`; tempo limite por nó e por execução, `Executor::time_limit_ms`, `crates/ravenrs-flows/src/run.rs:316` | App dizer a política de cada conector | APP |
| Subfluxo e execução parcial | ✅ | `Executor::run_from`, `crates/ravenrs-flows/src/run.rs:346`; `Subflow`, `crates/ravenrs-flows/src/nodes.rs:577`: roda outro fluxo com os itens que chegaram, com token filho e o tempo que sobra; o catálogo dado ao subfluxo impede que ele chame a si mesmo | Fixar a revisão do fluxo chamado depende das versões (#535) | APP; #535 |
| Fila e trabalhadores | ✅ | `Runtime::claim`, `crates/ravenrs-flows-runtime/src/runtime.rs:266`: reivindicação atômica num `UPDATE … RETURNING` dentro de uma transação de escrita, com lease renovado a cada passo; vários processos no mesmo arquivo | Fila em PostgreSQL para várias máquinas | BIBLIOTECA: `ravenrs-flows-runtime` |
| Cancelar execução | ✅ | `CancelToken`, `crates/ravenrs-flows/src/time.rs:95`, e `Executor::cancel_with`, `crates/ravenrs-flows/src/run.rs:309`: cada nó roda numa thread própria e é deixado para trás ao cancelar; `RunOutcome`, `run.rs:69` | Matar o processo filho de verdade fica com o executor de processos (#558); gravar o estado terminal, com #535 | BIBLIOTECA: #558, #535 |

**Fronteiras já identificadas:**

- `StateMachine::new`, em `crates/ravenrs-app/src/workflow.rs:15`, configura transições de estado de registros.
- `Step::new`, em `crates/ravenrs-app-requests/src/lib.rs:64`, configura etapas de decisões humanas.
- `Runner::tick`, em `crates/ravenrs-report-server/src/scheduler.rs:116`, inicia envios de relatórios devidos.

Essas APIs fornecem referências úteis, mas não executam um grafo arbitrário de transformações sobre itens.

### 5. Gatilhos

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Manual | 🟡 | `ApiService::endpoint`, `crates/ravenrs-app-server/src/lib.rs:404`; `JobWork`, `crates/ravenrs-app/src/job.rs:13` | Comando de início ligado ao motor, entrada de teste e registro de autoria/modo | BIBLIOTECA: `L10`; APP: ação |
| Agenda por cron e intervalo | 🟡 | `Schedule::parse` e `next_after`, `crates/ravenrs-report-server/src/cron.rs:31` e :66; `Runner::run_forever`, `crates/ravenrs-report-server/src/scheduler.rs:105` | Extrair agendamento genérico, intervalo por fluxo, fuso, atraso, sobreposição e recuperação após parada | BIBLIOTECA: `L14` |
| Webhook HTTP com resposta | 🟡 | `Webhook::new`, `crates/ravenrs-app-server/src/hooks.rs:97`; `ApiService::webhook`, `crates/ravenrs-app-server/src/lib.rs:397` | Atual hook exige envelope e assinatura próprios e retorna confirmação; faltam rotas configuráveis e resposta do fluxo com status, cabeçalhos e corpo | BIBLIOTECA: `L21` |
| Consulta periódica | ⬜ | Não encontrado gatilho; busca `B5`: `pollingtrigger`, `polling_trigger`, `polling` | Cursor persistido, deduplicação, intervalo, backoff e checkpoint da consulta | BIBLIOTECA: `L14`, `L16` |
| Eventos de arquivo | ⬜ | Não encontrado gatilho; busca `B5`: `filewatcher`, `file_watcher`, `notify::` | Observação de diretórios, debounce, política de links, overflow e recuperação de eventos | BIBLIOTECA: `L19` |
| Formulário | 🟡 | `EntityForm::new`, `crates/ravenrs-app-ui/src/form.rs:71` | Formulário desktop pode fornecer entrada; publicação HTTP exige contrato de submissão, validação, audiência e versão | BIBLIOTECA: `L05`, `L22`; APP: campos |
| Chat | 🟡 | `Chat::new`, `crates/ravenrs-widgets-chat/src/chat.rs:416` | Vincular envio a execução, sessão, correlação, resposta em partes e cancelamento | BIBLIOTECA: `L22`, `L25`; APP: conversa e fluxo |

O `Scheduler` visual organiza eventos e recursos. Sua existência não substitui o serviço que dispara execuções por relógio.

### 6. Nós de integração

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Requisição HTTP genérica | 🟡 | `RemoteRequest`, `Transport` e `HttpTransport`, `crates/ravenrs-app-remote/src/transport.rs:6`, :36 e :43 | Transporte atual é orientado a JSON e bearer; faltam cabeçalhos arbitrários, binário, multipart, paginação, resposta completa, cancelamento e limites configuráveis | BIBLIOTECA: `L16`, `L17` |
| Bancos Postgres e SQLite | 🟡 | `SqlBackend`, `crates/ravenrs-app-sql/src/lib.rs:185`; `SqliteBackend`, `crates/ravenrs-app-sqlite/src/lib.rs:276`; `PostgresBackend`, `crates/ravenrs-app-postgres/src/lib.rs:274` | Adaptar consultas e comandos a itens, tipos, transações e credenciais; conexão pertence ao trabalhador. Evitar usar o armazenamento do app como conexão implícita do nó | BIBLIOTECA: `L16`, `L18` |
| E-mail | 🟡 | `Mailer`, `SmtpMailer::new`, `crates/ravenrs-report-server/src/deliver.rs:33` e :52 | Extrair envio genérico hoje dependente dos tipos de relatório; recebimento e polling de caixa de entrada não foram encontrados como conectores | BIBLIOTECA: `L19` |
| Arquivos | 🟡 | `write_to_folder`, `crates/ravenrs-report-server/src/deliver.rs:96` | Provider geral de leitura/escrita/listagem, streams, binários, limites e diretórios autorizados | BIBLIOTECA: `L19` |
| Planilha | 🟡 | `Session::execute_headless`, `crates/ravenrs-sheet/src/engine/mod.rs:1124` | Adaptador entre lotes de objetos e linhas/células, política de cabeçalhos, tipos, fórmulas e arquivo de saída | BIBLIOTECA: `L20` |
| Set, filtrar, ordenar, agrupar, dividir e juntar | ✅ | `Set`, `Group`, `Split` e `Join`, `crates/ravenrs-flows/src/nodes.rs:317`, :423, :508 e :534; `Catalog::builtin`, `nodes.rs:550` | App escolher quais nós oferece | APP |
| Modelo de linguagem, agente, ferramentas e memória | 🟡 | `LocalAssistant::answer`, `crates/ravenrs-ai/src/assistant.rs:171`; `Tool`, `tool_rules` e `read_tool_call`, `crates/ravenrs-ai/src/tools.rs:9`, :35 e :42 | Interface de inferência independente da UI, saída estruturada, ciclo de ferramentas, limites e memória persistida por sessão | BIBLIOTECA: `L25`; APP: ferramentas permitidas e políticas |

`ravenrs-mcp` serve informações sobre o framework: `RavenServer`, em `crates/ravenrs-mcp/src/server.rs:136`. Não foi encontrada nessa API uma implementação de cliente de ferramentas remotas para o agente; esse suporte precisa de contrato próprio.

### 7. Execuções

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Lista com status, duração e modo | 🟡 | `DataGrid::new`, `crates/ravenrs-widgets-grid/src/grid/view.rs:331`; `RunRecord`, `crates/ravenrs-report-server/src/store.rs:68` | Grade disponível; registro existente é específico de relatório. Criar persistência genérica e projeção da execução do fluxo | BIBLIOTECA: `L12`; APP: colunas e modos |
| Filtros e busca | ✅ | `DataGrid::filter_builder`, `filter_row` e `search_panel`, `crates/ravenrs-widgets-grid/src/grid/view.rs:391`, :563 e :569 | App declarar campos consultáveis e ligar filtros ao repositório | APP |
| Detalhe no fluxo com dados de cada nó | 🟡 | `FlowCanvas::selection`, `content` e `locked`, `crates/ravenrs-widgets-charts/src/flow.rs:710`, :859 e :897 | Snapshot imutável da definição executada, tentativas e dados por porta; overlay visual de execução. `locked` é alternável e não constitui modo somente leitura irrevogável | BIBLIOTECA: `L02`, `L06`, `L12` |
| Reexecutar | ✅ | `Runtime::rerun` com `Revision::Original` ou `Current`, `crates/ravenrs-flows-runtime/src/runtime.rs:433`; a execução nova guarda a origem | App mostrar o aviso de efeitos repetidos | APP |
| Apagar e aplicar retenção | ✅ | `Runtime::prune` com `Retention`, `crates/ravenrs-flows-runtime/src/runtime.rs:527`: por idade, estado e quantidade por fluxo, apagando passos, eventos e arquivos | App escolher os prazos | APP |

### 8. Credenciais

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Tipos e formulário por tipo | 🟡 | `FieldKind`, `crates/ravenrs-app/src/entity.rs:25`; `EntityForm::new`, `crates/ravenrs-app-ui/src/form.rs:71` | Esquema de credencial, campos de escrita sem leitura, referência ao segredo e ciclo de troca/revogação | BIBLIOTECA: `L05`, `L23`; APP: catálogo |
| OAuth2 | ⬜ | Não encontrado cliente OAuth2; busca `B7`: `oauth`, `oauth2` | Autorização, callback local, PKCE quando aplicável, state, renovação, revogação e armazenamento protegido dos tokens | BIBLIOTECA: `L24` |
| Teste de conexão | ⬜ | Não encontrado contrato; busca `B7`: `connectiontest`, `connection_test`, `credential_type` | Teste por provider com prazo, cancelamento e diagnóstico que não exponha valores secretos | BIBLIOTECA: `L16`, `L23` |
| Cofre cifrado | ⬜ | Não encontrado; busca `B7`: `vault`, `secret_store`, `secretstore`, `keyring`, `encrypt`, `aes.gcm`, `chacha` | Cifragem autenticada, chave externa ao banco, bloqueio, rotação, recuperação e redação uniforme | BIBLIOTECA: `L23` |
| Compartilhamento | 🟡 | `Role::object_permission` e `member_permission`, `crates/ravenrs-app/src/security/mod.rs:220` e :239 | Reutilizar autorização por registro, mas separar permissão de usar da de administrar ou revelar uma credencial; worker deve revalidar acesso | BIBLIOTECA: `L23`, `L27`; APP: vínculos com projetos |

`FieldInfo::secret`, em `crates/ravenrs-app/src/entity.rs:432`, impede exposição pelos caminhos protegidos e esconde o campo das telas. Não cifra o conteúdo e não fornece formulário de atualização de segredo.

`IdentityProvider`, em `crates/ravenrs-app/src/security/external.rs:61`, verifica identidades externas. Não é um cliente OAuth2 para autorizar integrações.

### 9. Gestão

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Lista de fluxos, tags e pastas | ✅ | `FlowLibrary`, `crates/ravenrs-widgets-charts/src/flow_library.rs:46`, com `FlowShelf`, `crates/ravenrs-graphics/src/flow/document.rs:22`; `DataGrid::new`, `crates/ravenrs-widgets-grid/src/grid/view.rs:331`; `FieldKind::Tags`, `crates/ravenrs-app/src/entity.rs:25`; `TreeView::new`, `crates/ravenrs-widgets-lists/src/tree.rs:199` | `FlowDocument` guarda id, nome e diagrama; tags, pastas e projeto ficam no modelo do app | APP |
| Projetos | 🟡 | `Role::object_permission`, `crates/ravenrs-app/src/security/mod.rs:220` | Modelo de projeto e membros é do app; falta propagação uniforme da autorização aos comandos, trabalhadores e segredos | BIBLIOTECA: `L27`; APP: entidades e associações |
| Ativar e desativar fluxo | 🟡 | `StateMachine::new` e `transition`, `crates/ravenrs-app/src/workflow.rs:15` e :32 | Validar versão publicável, instalar/remover gatilhos e definir tratamento das execuções em andamento | BIBLIOTECA: `L10`, `L14`, `L21`; APP: ciclo de publicação |
| Versões e histórico | ⬜ | Não encontrado versionamento de fluxo executável; busca `B6`: `workflowversion`, `workflow_version` | Revisões imutáveis, rascunho, versão ativa, referência da execução e migração do formato | BIBLIOTECA: `L03`, `L10`, `L12`; APP: política editorial |
| Importar e exportar | 🟡 | `FlowGraph::from_json` e `to_json`, `crates/ravenrs-graphics/src/flow.rs:1178` e :1164 | Documento completo, compatibilidade, resolução de tipos ausentes e remoção de segredos; importar não ativa o fluxo | BIBLIOTECA: `L03`, `L10`; APP: formato |
| Modelos reutilizáveis de fluxo | 🟡 | `FlowGraph::insert`, `crates/ravenrs-graphics/src/flow.rs:1116` | Instanciar definição completa com novos IDs e credenciais a preencher; o conteúdo dos modelos é do exemplo | BIBLIOTECA: `L03`, `L10`; APP: modelos |
| Variáveis e ambiente | 🟡 | `SettingsStore`, `crates/ravenrs-app/src/settings.rs:44`; `Environment::parameter`, `crates/ravenrs-criteria/src/eval.rs:130` | Resolver escopos e precedência, tipar valores e congelar contexto por execução. Configuração em texto não substitui cofre | BIBLIOTECA: `L09`, `L10`, `L23`; APP: nomes e escopos |

### 10. Usuários e permissões

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Papéis e permissões por entidade, registro e campo | ✅ | `Role::type_permission`, `object_permission` e `member_permission`, `crates/ravenrs-app/src/security/mod.rs:197`, :220 e :239 | App declarar papéis e regras para suas entidades | APP |
| Acesso por projeto | 🟡 | `Role::object_permission`, `crates/ravenrs-app/src/security/mod.rs:220` | Aplicar associações de membros também a execuções, versões, gatilhos e credenciais | BIBLIOTECA: `L27`; APP: regras |
| Quem pode editar ou executar | 🟡 | `Role::action_permission`, `crates/ravenrs-app/src/security/mod.rs:267`; `Permissions::can_use_action`, mesmo arquivo:632 | Serviço de execução deve verificar permissão independentemente da tela, inclusive em fila e replay | BIBLIOTECA: `L27`; APP: ações e concessões |
| Auditoria e logs | 🟡 | `AuditTrail::custom`, `crates/ravenrs-app/src/security/audit.rs:137`; `AuditSettings::without_field`, mesmo arquivo:52 | Correlacionar fluxo, revisão, execução, nó, tentativa e ator; redação de segredos e retenção dos logs | BIBLIOTECA: `L27`; APP: eventos auditáveis |

A autorização de ações deve ser explícita: `can_use_action` permite ações que não tenham negação por papel. Uma ação sensível não deve depender somente de estar escondida no editor.

### 11. Servidor

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| API REST protegida e endpoints próprios | ✅ | `ApiService::new` e `endpoint`, `crates/ravenrs-app-server/src/lib.rs:302` e :404 | App registrar comandos de fluxos, execuções e administração; operações longas devem devolver identificador da execução | APP, sobre o servidor |
| Servidor de webhooks | 🟡 | `ApiService::webhook`, `crates/ravenrs-app-server/src/lib.rs:397` | Ampliar contrato de rotas e resposta; enqueue rápido, idempotência e tratamento de prazo | BIBLIOTECA: `L21` |
| Persistência de execuções | ✅ | `Runtime`, `crates/ravenrs-flows-runtime/src/runtime.rs:186`, sobre SQLite: execuções, passos com o JSON de cada nó, eventos e arquivos à parte (`Runtime::put_blob`, `crates/ravenrs-flows-runtime/src/runtime.rs:508`) | Outro banco além do SQLite | BIBLIOTECA: `ravenrs-flows-runtime` |
| Métricas | ⬜ | Não encontrado contrato de métricas de execução; busca `B8`: `metrics`, `prometheus`, `opentelemetry` na camada de aplicação e servidor | Contadores, histogramas e filas com dimensões limitadas; health/readiness do executor e gatilhos | BIBLIOTECA: `L27` |

O servidor atual processa o estado da aplicação em uma thread própria, uma requisição por vez, conforme `crates/ravenrs-app-server/src/http.rs`. Um endpoint não deve bloquear essa thread executando o fluxo inteiro.

### 12. Experiência geral

| Recurso | Estado | Onde está hoje (arquivo:linha) | O que falta | Onde deve entrar |
|---|---|---|---|---|
| Tema claro e escuro | ✅ | `Theme::raven_dark` e `raven_light`, `crates/ravenrs-style/src/lib.rs:426` e :462 | App escolher e persistir preferência | APP |
| Atalhos e ajuda de teclado | ✅ | `ShortcutSheet::new`, `crates/ravenrs-widgets-kit/src/shortcuts.rs:49`; `FlowCanvas::history`, `crates/ravenrs-widgets-charts/src/flow.rs:719` | Mapear comandos do app e resolver conflitos de contexto com editores | APP |
| Barra de comando com Ctrl+K | ✅ | `CommandPalette::new`, `crates/ravenrs-widgets-base/src/floating.rs:1013` | Registrar comandos e suas condições de disponibilidade | APP |
| Notificações | ✅ | `Toasts::show_with`, `crates/ravenrs-widgets-base/src/floating.rs:683` | App traduzir eventos e erros em mensagens com ação apropriada | APP |
| Estados vazios | ✅ | `EmptyState::new` e `action`, `crates/ravenrs-widgets-kit/src/status.rs:521` e :538 | Conteúdo para fluxo vazio, nó sem entrada, catálogo sem resultados e histórico vazio | APP |
| Acessibilidade | 🟡 | `FlowCanvas::label`, `crates/ravenrs-widgets-charts/src/flow.rs:909`; `BuildCx::drag_source`, `crates/ravenrs-widgets-base/src/context.rs:1701`; `Announcer::polite`, `crates/ravenrs-app-a11y/src/announce.rs:72` | Provar navegação completa, alternativa ao arraste, foco entre painéis e anúncios de execução nas novas composições | BIBLIOTECA: aceites de `L01`, `L05`–`L09`, `L26`; APP: composição |
| Internacionalização | ✅ | `Labels::from_texts`, `crates/ravenrs-widgets-base/src/labels.rs:1145`; `Texts::get`, `crates/ravenrs-app-i18n/src/lib.rs:248` | Cadastrar textos do domínio, pluralização necessária e formatos de data/duração | APP; textos dos novos componentes em suas crates |

### 13. Vídeo e treinamento

Levantamento por leitura do código, sem executar testes nem alterar arquivos. Estados: ✅ API disponível; 🟡 base disponível com limitações; ⬜ peça não encontrada nas buscas indicadas. “Sem janela” não significa “sem GPU”. Os estados descrevem a biblioteca; os nós do catálogo ainda precisam de integração.

**(A) Treinamento embutido no aplicativo**

| Recurso | Estado | Onde está hoje (arquivo:linha) | Sem janela? | O que falta | Onde deve entrar |
|---|---|---|---|---|---|
| Gravar a interface real por roteiro | ✅ | `Recorder::new`, `click`, `type_into`, `finish`: `crates/ravenrs-movie/src/record.rs:79`, `:217`, `:228`, `:318` | Sim; cria `Ui`, avança relógio e recolhe primitivas, sem abrir janela | Escrever roteiros do Raven Flows com dados previsíveis e elementos acessíveis por nome | Roteiros no exemplo; melhorias do gravador em `ravenrs-movie` |
| Filme vetorial fiel, sem captura de pixels | 🟡 | `FilmBuilder::frame`: `crates/ravenrs-movie/src/movie.rs:800` | Sim, sem GPU para gravar | O armazenamento omite `Primitive::Path` e pincéis não suportados silenciosamente. Também aceita imagens raster da interface; “nunca pixels” não é uma restrição garantida pela API | `ravenrs-movie`: serialização completa das primitivas e validação do modo exclusivamente vetorial |
| Ponteiro, clique, tecla e caixas explicativas | ✅ | `Recorder::explain`, `guide`: `crates/ravenrs-movie/src/record.rs:185`, `:279`; `Soundtrack::of`: `crates/ravenrs-movie/src/soundtrack.rs:28` | Gravação e síntese do som: sim | Cobrir criação de nós, conexões, expressões, execução e credenciais nos roteiros | Conteúdo no exemplo; comportamento reutilizável em `ravenrs-movie` |
| Player com capítulos, passos e narração previamente gravada | ✅ | `MoviePlayer::new`, `narration`, `cue`: `crates/ravenrs-movie/src/player.rs:697`, `:742`, `:778` | Lógica testável em `Ui` sem janela; assistir e ouvir exige apresentação e áudio | Incorporar os filmes e `.voz` do novo exemplo | Composição no exemplo, usando `ravenrs-movie` |
| Biblioteca pesquisável e abertura no trecho pertinente | ✅ | `MovieLibrary::ask`: `crates/ravenrs-movie/src/search.rs:161`; `MovieAssistant::new`: `crates/ravenrs-movie/src/assistant.rs:91`; `MovieCue::go_to_film`: `crates/ravenrs-movie/src/player.rs:166` | Busca: sim; visualização: dentro da interface | Vincular ajuda contextual às telas do Raven Flows | Exemplo, sobre essas APIs |
| Kokoro `pm_santa` → pacote `.voz` | 🟡 | `SpeechEngine::speak`: `crates/ravenrs-ai/src/speech.rs:368`; `Narration::pack`: `crates/ravenrs-movie/src/narration.rs:183` | Sim, CPU; modelo necessário na geração, dispensável na reprodução do pacote | Generalizar normalização da fala e empacotamento incremental hoje orquestrados nos exemplos | `ravenrs-ai` e `ravenrs-movie` |
| Edição visual e edição por código | ✅ | `MovieEditor::new`: `crates/ravenrs-movie/src/editor.rs:880`; `EditorState::split`: `:373`; `Project::trim_start`: `crates/ravenrs-movie/src/project.rs:377` | Operações de edição: sim; editor visual: interface | Usar o editor quando houver revisão manual dos treinamentos | Composição no exemplo; recursos em `ravenrs-movie` |

O precedente está implementado em `examples/family-finance/src/ui/training.rs:22`: seis filmes incorporados e `narracao.voz`, oferecidos pela página Treinamentos com `MovieAssistant` (`:90`). A navegação chega à página em `examples/family-finance/src/ui/mod.rs:406`; o teste `examples/family-finance/tests/suite/training.rs:794` verifica a abertura pelo assistente a partir de outra tela. O teste de cobertura aceita **90%** das caixas narradas (`:1140`), portanto não comprova cobertura integral.

**(B) Fluxos que produzem vídeo**

| Recurso | Estado | Onde está hoje (arquivo:linha) | Sem janela? | O que falta | Onde deve entrar |
|---|---|---|---|---|---|
| Roteiro → filme de uma tela RavenRS | 🟡 | `Recorder`: `crates/ravenrs-movie/src/record.rs:67` | Sim, construindo a `View` no trabalhador | Hoje o roteiro é código Rust e recebe uma `View`; falta contrato serializável de passos e registro dos alvos graváveis. Não há captura genérica de aplicativos externos | `ravenrs-movie` e componente de catálogo do motor |
| Texto → narração local | ✅ | `SpeechEngine::load`, `speak`, `pieces`: `crates/ravenrs-ai/src/speech.rs:352`, `:368`, `:382` | Sim; bloqueante, CPU, sem dispositivo de áudio | Adaptador do nó, modelo instalado e saída de áudio como artefato | Catálogo do motor, usando `ravenrs-ai` |
| Usar `LocalVoice`/`VoiceManager` para gerar arquivos | 🟡 | `LocalVoice::say`, `play_ready`: `crates/ravenrs-ai/src/voice.rs:113`, `:133`; `VoiceManager::settings`: `:382` | São voltados à UI: `UiWaker`, fila privada e reprodução via `Sound` | Para geração, usar `SpeechEngine`; caso seja necessário serviço assíncrono compartilhado, expor resultados e erros independentemente da UI | `ravenrs-ai` |
| Carregar e atualizar `.voz` dinamicamente | 🟡 | `Narration::new`, `pack_onto`: `crates/ravenrs-movie/src/narration.rs:155`, `:191` | Sim, mas o carregamento exige bytes `'static` e usa `Rc` | Aceitar armazenamento possuído, leitura validada e ciclo de vida finito; exemplos usam `Box::leak` | `ravenrs-movie` |
| Filme → vídeo codificado | 🟡 | `Video::write`: `crates/ravenrs-movie-video/src/lib.rs:276` | Sim, **com GPU disponível**; sem GPU retorna `VideoError::NoGpu` | Já produz MOV/H.264/PCM. Falta cancelamento cooperativo e tratamento de saída parcial para trabalhos interrompidos | `ravenrs-movie-video` |
| Filme → MP4 | 🟡 | Exportador atual: `Video::write`, `crates/ravenrs-movie-video/src/lib.rs:276`; base de contêiner: `Brand::Mp4`, `crates/ravenrs-deck/src/craft/media/isobmff/mux.rs:12` | Contêiner: sim, CPU; renderização do filme: GPU | Integrar saída MP4 ao exportador e definir áudio compatível com o destino. O muxador existente não está ligado a `Video`; trocar a extensão não resolve | `ravenrs-movie-video`, reutilizando ou extraindo o componente de mídia de `ravenrs-deck` |
| Trilha com efeitos e todas as falas | 🟡 | `Soundtrack::of`: `crates/ravenrs-movie/src/soundtrack.rs:28`; player: `MoviePlayer::narration`, `crates/ravenrs-movie/src/player.rs:742` | Sim, CPU | Exportação mistura efeitos e falas dos hotspots, mas não percorre falas dos cartões de abertura/encerramento como o player. Falas ausentes são relatadas, sem síntese automática | `ravenrs-movie`: sequência de falas compartilhada entre player e exportação |
| Legenda `.srt` e capítulos | 🟡 | `subtitles`, `chapters`: `crates/ravenrs-movie-video/src/text.rs:12`, `:41` | Sim, CPU | SRT vem dos hotspots, não de alinhamento da fala; não inclui toda fala de cartões nem todo `OverlayKind::Caption`. Na concatenação, conferir arredondamento por quadro contra os tempos do vídeo | `ravenrs-movie-video`, consumindo a sequência comum de falas |
| Corte, montagem e versão vertical | ✅ | `Project::split`, `trim_end`: `crates/ravenrs-movie/src/project.rs:307`, `:382`; `Movie::following`: `crates/ravenrs-movie/src/social.rs:185`; `VideoFormat::VERTICAL`: `crates/ravenrs-movie-video/src/lib.rs:63` | Edição: sim, CPU; exportação: GPU | Adaptador e escolha dos trechos. O formato vertical reorganiza o filme; não corta automaticamente um vídeo raster importado. Limite de duração é apenas sinalizado | Catálogo do motor, sobre `ravenrs-movie` e `ravenrs-movie-video` |
| Miniatura PNG | ✅ | `Video::thumbnail`: `crates/ravenrs-movie-video/src/lib.rs:259`; `Movie::poster_ms`: `crates/ravenrs-movie/src/movie.rs:306` | Sim, com GPU | Seleção do momento e persistência do artefato | Catálogo do motor |
| Conferir fala com reconhecedor independente | ✅ | `Transcriber::load`, `transcribe`: `crates/ravenrs-ai/src/hearing.rs:54`, `:86` | Sim, CPU; outro modelo instalado; entrada mono a 16 kHz | Adaptador de verificação e critérios adequados de transcrição, incluindo o áudio final exportado | `ravenrs-ai` e infraestrutura de testes |
| Publicar com credencial da pessoa | ⬜ | Não encontrado; buscas por `upload`, `publish`, `oauth`, `credential` em movie, movie-video, channel-video e segurança de `ravenrs-app` | A implementar como operação de rede sem janela | Conector de serviço de vídeo, autorização, renovação de token, envio retomável, progresso e identificação do vídeo publicado | Componentes reutilizáveis de conectores e credenciais da biblioteca de fluxos |
| Regravar somente treinamentos afetados por uma versão | ⬜ | Não encontrado; buscas por `manifest`, `fingerprint`, `Script` e orquestração nos crates de filme e channel-video | A implementar sem janela | Manifesto de dependências entre roteiro, tela, tema, voz e versão; cache de artefatos e execução seletiva. Já existe atualização de falas por texto, mas não seleção de filmes por mudança | `ravenrs-movie` e motor de fluxos |

**Custos e dependências observáveis no repositório:**

- **Gravação:** simula quadros a 30 Hz, sem esperar o tempo real do filme (`crates/ravenrs-movie/src/record.rs:10`, `:150`). Não encontrei medição de tempo de processamento. A duração das caixas usa a gravação mais 0,6 s; sem gravação, estima leitura por caracteres (`:109`).
- **Narração:** `crates/ravenrs-ai/README.md:25` documenta aproximadamente **2,5 segundos de fala por segundo de processamento**, modelo de **329 MB** e pico de **1,7 GB** em Apple M2 Pro. É referência documentada, não medição desta análise. A síntese retorna PCM a 24 kHz; Kokoro roda em CPU e usa fonemização própria, sem ferramenta externa. Precisa dos pesos, configuração e arquivos das vozes. `ModelFolder::download` já oferece download com hash, progresso e cancelamento (`crates/ravenrs-ai/src/models.rs:203`).
- **Codificação:** renderiza, transfere pixels da GPU, converte para YUV e codifica cada quadro sequencialmente. `Soundtrack::of` aloca a trilha inteira em memória: a 48 kHz/f32/mono, aproximadamente **11,5 MB por minuto**, antes de buffers adicionais. OpenH264 é compilado da fonte; o exportador não chama codificador externo e exige Rust 1.89 (`crates/ravenrs-movie-video/Cargo.toml:7`). O README declara macOS, Linux e Windows; não foram validados aqui.
- **Tempos medidos:** a geração de voz imprime tempo total em `examples/family-finance/tests/suite/training.rs:1058`; a exportação imprime duração, tamanho e tempo em `examples/channel-video/src/main.rs:272`. Não há números de execução nesses trechos. O relógio de exportação é compartilhado entre saídas, portanto o segundo valor pode ser acumulado.
- **Reprodução:** o pacote `.voz` dispensa Kokoro instalado. Quando falta uma fala, o player tenta voz do sistema: `say` no macOS, PowerShell/System.Speech no Windows e `espeak-ng` no Linux (`crates/ravenrs-platform-native/src/sound.rs:258`, `:277`, `:297`). Essa alternativa não preenche a trilha exportada.

#### Lacunas de vídeo da biblioteca

Tamanhos relativos: **P** pequena, **M** média, **G** grande.

- **`ravenrs-movie`**
  - **G · #550:** preservar `Primitive::Path`, pincéis e demais dados vetoriais; emitir erro ou diagnóstico quando houver primitivas incompatíveis. Destrava filmes fiéis do canvas sem converter a tela em imagem.
  - **M · #550:** modo de gravação exclusivamente vetorial, com rejeição explícita de imagens raster. Destrava o requisito “nunca pixels” verificável.
  - **M · #551:** roteiro serializável, validação e registro de fábricas de telas graváveis. Destrava o nó “roteiro → filme” configurável por dados.
  - **M · #552:** `.voz` com bytes possuídos e erros de leitura explícitos. Destrava trabalhos repetidos sem vazamento permanente nem aceitação silenciosa de pacote inválido.
  - **M · #553:** sequência única de falas, incluindo cartões e tempos, consumida por player, trilha e legendas. Destrava equivalência entre treinamento embutido e vídeo publicado.

- **`ravenrs-ai`**
  - **M · #96 (já existia):** normalização reutilizável de números, datas, valores e siglas antes da fonemização. Hoje parte disso está em `examples/family-finance/src/ui/spoken.rs:129`; `phonemes_pt_br` deixa números de fora (`crates/ravenrs-ai/src/phonemes.rs:795`). Destrava narração correta sem depender do exemplo financeiro.
  - **M · referência local à #97; escopo remoto não conferido:** consolidar avaliação da fala por reconhecedor independente e aplicá-la ao pacote final. O teste em `examples/channel-video/src/main.rs:379` já compara três vozes com Whisper, mas está ignorado e verifica presença de 80% das palavras de uma frase, sem avaliar ordem e repetições. Destrava evidência de inteligibilidade por treinamento.

- **`ravenrs-movie-video`**
  - **M · #554:** integrar contêiner MP4 e política de áudio, aproveitando a infraestrutura ISO BMFF existente. Destrava entrega MP4 real.
  - **M · #554:** cancelamento, limites de recursos e descarte seguro de arquivo incompleto. Destrava interrupção de execuções pelo motor.
  - **M · #553:** legendas completas e tempos comuns para concatenação, áudio e quadros. Destrava exportações editadas sem omissão de falas ou deriva acumulada.
  - **P · #554:** teste de exportação que exija GPU no ambiente de validação. Hoje o teste retorna sem falhar quando recebe `NoGpu` (`crates/ravenrs-movie-video/tests/video.rs:56`); isso pode deixar a codificação sem exercício.

- **Motor, catálogo, conectores e credenciais da biblioteca de fluxos**
  - **M · #556:** adaptadores de vídeo com entradas/saídas serializáveis, referências a artefatos, progresso e cancelamento. `Movie` e `Narration` usam `Rc`; construir esses objetos dentro do trabalhador e transportar bytes ou referências a arquivos entre trabalhadores.
  - **M · #556:** manifesto e cache por dependências de treinamento. Destrava a regeneração seletiva por versão.
  - **G · #557:** conector de publicação com credenciais pessoais e envio retomável. Destrava publicação como etapa rastreável do fluxo.

#### Nós de vídeo propostos para o catálogo

| Nó | Entrada | Saída | Peça do RavenRS | Pronto? |
|---|---|---|---|---|
| Gravar tela por roteiro | Alvo RavenRS, passos, dados de demonstração, tema e tamanho | `.ravenmovie` | `Recorder::new`, `crates/ravenrs-movie/src/record.rs:79` | Base sem janela; falta contrato de roteiro e fidelidade vetorial completa |
| Narrar texto | Texto, modelo, voz `pm_santa` e configurações | PCM e artefato de áudio | `SpeechEngine::speak`, `crates/ravenrs-ai/src/speech.rs:368` | Base sem janela pronta; falta adaptador e normalização geral |
| Montar pacote de voz | Textos originais, gravações e pacote anterior opcional | `.voz` | `Narration::pack_onto`, `crates/ravenrs-movie/src/narration.rs:191` | Empacotamento pronto; ajustar carregamento dinâmico |
| Editar filme | Filme, cortes, cartões e efeitos | `.ravenmovie` editado | `Project`, `crates/ravenrs-movie/src/project.rs:236` | Base sem janela pronta |
| Exportar vídeo | Filmes, `.voz`, formato e taxa de quadros | Vídeo e relatório | `Video::write`, `crates/ravenrs-movie-video/src/lib.rs:276` | MOV disponível com GPU; MP4 e cancelamento pendentes |
| Gerar legendas e capítulos | Filmes editados | `.srt` e texto de capítulos | `subtitles`/`chapters`, `crates/ravenrs-movie-video/src/text.rs:12`, `:41` | Parcial: hotspots e capítulos disponíveis; completar falas |
| Preparar corte vertical | Filme, intervalo e aproximação | Filme editado e vídeo vertical | `Movie::following`, `crates/ravenrs-movie/src/social.rs:185`; `VideoFormat::VERTICAL`, `crates/ravenrs-movie-video/src/lib.rs:63` | Base pronta para filme RavenRS; renderização exige GPU |
| Gerar miniatura | Filme, instante e formato | PNG | `Video::thumbnail`, `crates/ravenrs-movie-video/src/lib.rs:259` | Base pronta com GPU |
| Conferir narração | Áudio, texto esperado e idioma | Transcrição e resultado da comparação | `Transcriber::transcribe`, `crates/ravenrs-ai/src/hearing.rs:86` | Reconhecimento disponível; falta política de avaliação |
| Publicar vídeo | Arquivo, metadados, legendas, capa e referência à credencial | Identificador, endereço e estado do envio | Conector a implementar | Pendente; buscas registradas na tabela B |
| Selecionar treinamentos afetados | Manifesto, versão e mudanças | Lista de trabalhos e artefatos reutilizáveis | Manifesto/cache a implementar | Pendente; buscas registradas na tabela B |

#### Telas e fases

As telas e fases abaixo são propostas; ainda não há código do exemplo.

| Fase | Uso e telas | Como provar |
|---|---|---|
| **1 — Treinamento da primeira jornada** | **A:** central Treinamentos; ajuda contextual no canvas e catálogo de nós. Filme sobre criar gatilho, conectar nós, configurar expressão e executar | Roteiro sobre a interface real com dados fixos; teste sem janela verificando ações, capítulos, caixas e abertura no trecho certo |
| **2 — Cobertura operacional** | **A:** ajuda no inspetor de nós, expressões, credenciais e detalhes da execução; filmes de diagnóstico e repetição | Testes por roteiro; nenhum segredo real nos dados gravados; cobertura integral das falas e duração suficiente de caixas e cartões |
| **3 — Produção de mídia** | **B:** nós no catálogo; parâmetros no inspetor; progresso e artefatos na execução. Prévia de `.ravenmovie` com `MoviePlayer` | Exportação sem janela em ambiente com GPU obrigatória; decodificar vídeo, conferir áudio e tempos e revisar quadros de momentos críticos em formatos largo e vertical |
| **4 — Qualidade da fala** | **A e B:** incorporar `.voz` aprovado e disponibilizar relatório da conferência | Reconhecedor independente sobre a voz empacotada e sobre a trilha exportada, incluindo abertura/encerramento; conferir texto, ordem, omissões e números, além da revisão auditiva |
| **5 — Versão e publicação** | **B:** fluxo “nova versão → selecionar afetados → gravar → narrar faltantes → exportar novidades → conferir → publicar”; resultados nos detalhes da execução | Segunda execução reutiliza artefatos; alteração isolada invalida somente dependências pertinentes; testes do conector com serviço simulado, falha, retomada e cancelamento |

Há precedentes para as provas: os testes financeiros verificam roteiros e sincronismo (`examples/family-finance/tests/suite/training.rs:651`, `:749`); o teste de vídeo decodifica H.264 e compara o último quadro com uma miniatura (`crates/ravenrs-movie-video/tests/video.rs:56`). Isso não substitui conferir quadros do canvas real, sobretudo caminhos vetoriais atualmente omitidos.

#### Não verificado

- Não executei testes, síntese, reprodução, exportação ou download; não conferi visualmente quadros nem ouvi/transcrevi os pacotes existentes.
- Não medi tempos desta máquina, uso máximo de memória ou funcionamento efetivo em macOS, Linux e Windows.
- `rg -n "#[0-9]+" crates/ravenrs-movie*` não encontrou referências. A busca global por #74, #85, #96, #97 e #98 encontrou #97 no teste de voz e ocorrências de #74/#98 em outros componentes. Não consultei as issues remotas; não atribuí título, estado ou escopo a esses números.
- Li a base ISO BMFF de `ravenrs-deck`, mas não validei sua integração com o exportador, o codec de áudio a adotar nem compatibilidade com um serviço de vídeo.
- Não verifiquei contratos de trabalhador, catálogo ou cofre de credenciais de uma futura biblioteca de fluxos; os componentes indicados para essas lacunas são destinos propostos.
- O README de `ravenrs-ai` ainda afirma que reconhecimento não existe, embora `Transcriber` esteja implementado. As conclusões acima seguem o código lido.

### 14. Nós de agente por CLI (Codex CLI e Claude CLI)

Levantamento somente por leitura do código. 🟡 indica uma base parcial, não um nó pronto. Os nomes de crates novas abaixo são propostas. “Sem janela” distingue execução sem interface de componentes visuais; processos locais exigem um trabalhador nativo.

IA local continua sendo o padrão. Escolher Codex CLI ou Claude CLI exige consentimento explícito para usar a assinatura ou chave da pessoa. O app chama exclusivamente o executável instalado; o login acontece fora dele, sem guardar nem digitar credenciais.

| Recurso | Estado | Onde está hoje (arquivo:linha) | Sem janela? | O que falta | Onde deve entrar |
|---|---|---|---|---|---|
| 1. Processo com argumentos, pasta, stdout/stderr simultâneos e atualização sem bloquear a UI | 🟡 | `examples/raven-code/src/cargo.rs:272`, função pública `start`: inicia `cargo` numa thread, define argumentos e pasta, drena os dois canais em threads e espera o término. | Sim, o executor aceita um callback sem UI; não garante ausência de console no Windows. | Extrair executor genérico da biblioteca; parâmetros tipados, eventos por canal, código de saída completo e erros de leitura. Hoje o exemplo conserva essencialmente sucesso/falha e acumula texto sem limite. | Nova `ravenrs-process`; migrar `raven-code` para ela. |
| 1. Stdin e prompt grande | 🟡 | `crates/ravenrs-pagamento/src/auxiliar.rs:368`, `CanalProcesso::iniciar`, abre stdin/stdout; `CanalProcesso` implementa envio e leitura de linhas de forma bloqueante. | Sim, nativo; precisa executar fora da thread da UI. | Transporte genérico: escrever prompt, fechar stdin, drenar stdout/stderr em paralelo e tratar pipe fechado. Codex recebe o prompt por `-`; Claude precisa de transporte compatível com o formato de entrada escolhido. | `ravenrs-process`; montagem do protocolo em nova `ravenrs-agent-cli`. |
| 1. Cancelamento, timeout e árvore de processos | 🟡 | `crates/ravenrs-app/src/job.rs:224`, `Job::cancel`, sinaliza cancelamento cooperativo; `crates/ravenrs-pagamento/src/auxiliar.rs:386`, `CanalProcesso::matar`, mata e espera somente o filho direto. | A sinalização e o encerramento dispensam janela; `Job` pertence ao ciclo de ações do app. | Timeout, interrupção seguida de morte forçada, grupo de processos no Unix e mecanismo equivalente no Windows; coletar filhos e não deixar leitores presos em pipes herdados. Cancelar `Job` não mata uma CLI. | `ravenrs-process`, integrado a `ravenrs-app`; relacionado à #496. |
| 1. Ambiente filtrado e execução sem console nativo | ⬜ | Busca em `crates` e `examples`: `env_clear`, `process_group`, `CREATE_NO_WINDOW`, `taskkill`, `killpg`; não foi localizada uma abstração genérica. | Requisito do executor proposto. | Ambiente por lista permitida, passagem controlada das variáveis necessárias à CLI e lançamento sem console no Windows. Usar argumentos separados, sem interpolação em shell. | `ravenrs-process`. |
| 2. Descoberta e diagnóstico das CLIs | ⬜ | Buscas: `find_executable`, `find_binary`, `which::`, `auth_status`, `login_status`, `minimum_version`, `codex exec`, `claude -p`; nenhum adaptador encontrado. | Deve funcionar no trabalhador nativo. | Resolver PATH ou caminho explícito; consultar versão e capacidades; distinguir ausente, incompatível, sem login e falha operacional. Ações: instalar externamente, atualizar ou fazer login num terminal fora do app. Não inferir “sem login” de qualquer saída não zero. | Resolução em `ravenrs-process`; diagnóstico específico em `ravenrs-agent-cli`. |
| 3. Eventos JSONL e `stream-json` | 🟡 | `examples/raven-code/src/cargo.rs:95`, `read_line`, interpreta JSON do compilador por linha. `crates/ravenrs-nfe/src/sandbox/gravacao.rs:196`, `Reprodutor::de_jsonl`, lê gravação JSONL inteira, específica do domínio. | Sim; são leitores sem UI. | Decodificadores próprios para Codex e Claude: fragmentação de bytes, última linha sem newline, limites de tamanho, eventos desconhecidos, ferramentas, erro, conclusão e mensagem final. O leitor de cargo não reconhece esses protocolos. | Enquadramento de linhas em `ravenrs-process`; protocolos em `ravenrs-agent-cli`. |
| 3. Esquema de saída e validação independente | ⬜ | Buscas: `jsonschema`, `json-schema`, `json_schema`, `StreamDeserializer`, `Deserializer::from_reader`; não foi encontrado validador genérico de JSON Schema. Geração de schemas para ferramentas não comprova validação de resultados. | Deve ser inteiramente sem janela. | Entregar arquivo a `--output-schema` e schema a `--json-schema`; validar novamente o resultado no app, com dialeto definido e erros por caminho. Separar conclusão do processo de resultado válido. | Nova `ravenrs-data-validation`; adaptação das opções em `ravenrs-agent-cli`. |
| 4. Política por nó, limites e proteção do log | ⬜ | Buscas: `codex exec`, `claude -p`, `stream-json`, `env_clear`; nenhum contrato de política para agentes CLI localizado. | Deve ser aplicada no trabalhador, inclusive sem painel aberto. | Só leitura por padrão; pastas autorizadas, ferramentas permitidas/negadas, tempo, voltas e orçamento. Mapear apenas capacidades comprovadas da versão instalada; recusar restrições que não possam ser garantidas. Registrar metadados e eventos sanitizados, com retenção e limites; não registrar ambiente, tokens ou conteúdo bruto indiscriminadamente. | `ravenrs-agent-cli`, com imposição de prazo/cancelamento em `ravenrs-process`. |
| 4. Aprovação humana durável antes de escrita | 🟡 | `crates/ravenrs-app-requests/src/lib.rs:191`, `RequestsModule::submit`, abre pedido/tarefa; `:224`, `decide`, verifica papel e versão, registra decisão idempotente e emite evento no mesmo commit. `crates/ravenrs-app/src/outbox.rs:255`, `Dispatcher::run_at`, reconcilia envios incertos. | Sim, com `ObjectSpace` persistente; a decisão pode chegar posteriormente pela UI. | Vincular aprovação à execução, política, diretórios, revisão-base e hash do artefato. Impedir execução antes da decisão e invalidar aprovação se o conteúdo mudar. A base da #258 não é um motor durável de processos externos. | Integração em `ravenrs-app-requests`/`ravenrs-app`; execução em `ravenrs-agent-cli` e futuro motor de fluxos. |
| 5. Worktree descartável, diff e aplicação aprovada | ⬜ | Buscas: `git2`, `gix::`, `libgit`, `git worktree`, `git diff`, `git status`, `git apply`, `Command::new("git")`; nenhum serviço correspondente encontrado. Referências a worktrees em cache de compilação não gerenciam git. | Deve funcionar sem janela. | Criar e limpar worktree isolada; fixar revisão-base; coletar arquivos novos e alterações; limitar caminhos/tamanho; preservar artefato durante aprovação. Aplicar somente o diff aprovado, após conferir base e conflitos. Worktree não substitui sandbox. | Nova `ravenrs-git`, usando `ravenrs-process`, com aprovação em `ravenrs-app-requests`. |
| 6. Console ao vivo e visualização do resultado | 🟡 | `crates/ravenrs-widgets-chat/src/chat.rs:323`, `ChatMessage::assistant_writing`, recebe texto compartilhado entre threads; `:416`, `Chat::new`. `crates/ravenrs-widgets-chat/src/code_editor.rs:288`, `CodeEditor::read_only`. O painel de `raven-code` usa esse editor em `examples/raven-code/src/app.rs:973`. | A apresentação é visual; a execução deve continuar sem ela. | Console reutilizável com eventos, ferramentas, tempo, custo quando informado, filtros e histórico limitado. O “Terminal” atual é texto somente leitura, sem sessão de terminal ou entrada de processo. | Console em `ravenrs-widgets-chat`; ligação no painel do nó; #496 relacionada à parada/entrada. |
| 6. Tabelas e inspector de dados | 🟡 | `crates/ravenrs-widgets-grid/src/grid/view.rs:56`, tipo público `DataGrid`, oferece a base tabular. | Componente visual; dados devem existir independentemente dele. | Inspector de JSON aninhado, arquivos, schema e diff. Buscas `DataInspector`, `JsonView`, `JsonTree`, `PropertyGrid`, `inspector` nos widgets não localizaram um inspector genérico para esses resultados. | Inspector reutilizável em `ravenrs-widgets-kit`, composto com `ravenrs-widgets-grid` e `ravenrs-widgets-chat`. |
| 6. Parar e retomar sessão do agente | ⬜ | Buscas: `codex exec`, `claude -p`, `stream-json`; nenhum controlador dessas sessões encontrado. | O controle deve funcionar sem janela. | Persistir identificador da sessão, diretório, versão e política; implementar `resume` por provedor sem confundir retomada com repetição. Mostrar estados cancelado, interrompido, retomável e concluído; conciliar retenção com `--ephemeral`. | `ravenrs-agent-cli` e persistência do futuro motor; botões no console. |
| 7. Expressões para montar prompt a partir dos itens | 🟡 | `crates/ravenrs-criteria/src/eval.rs:18`, `Record`, possui implementação para `serde_json::Value` com feature `json`; `:283`, `Scope::evaluate`, avalia expressões. `crates/ravenrs-widgets-kit/src/expression_editor.rs:78`, `ExpressionEditor::new`, oferece editor com campos e parâmetros. | Avaliação: sim. Editor: visual. `Environment` deve ser criado por execução/thread. | Contexto de itens do fluxo, interpolação explícita, prévia do prompt e limites de tamanho; tratar texto de issue como entrada sem autoridade para ampliar permissões. | Extensão em `ravenrs-criteria`; contexto e montagem no futuro `ravenrs-flows`. |
| 7. Resultado tipado para o próximo nó | 🟡 | `crates/ravenrs-app/src/json.rs:8`, `value_to_json`, e `:20`, `object_from_json`, convertem valores/registros. A conversão de entrada não preserva objetos e arrays aninhados como um item arbitrário. | Sim. | Contrato de item com texto final, JSON validado, referências a arquivos, diff, sessão, término e métricas opcionais; persistência, proveniência e roteamento de falhas. Não converter JSON inválido em sucesso textual silenciosamente. | Futuro `ravenrs-flows`; produção do resultado em `ravenrs-agent-cli`. |

#### Lacunas da biblioteca para os nós de agente

Tamanhos relativos: **P** pequeno, **M** médio, **G** grande. Crates indicadas como novas são propostas, não dependências existentes.

- **`ravenrs-process` — nova**
  - **G · #558, relacionada à #496:** executor nativo genérico, stdin, ambiente filtrado, drenagem concorrente, eventos limitados, saída completa, timeout e encerramento da árvore. Destrava agentes CLI e a reutilização pelo `raven-code`.
  - **P · #558:** resolução de executável e consulta de versão com prazo. Destrava diagnóstico reproduzível no trabalhador.

- **`ravenrs-agent-cli` — nova**
  - **G · #559:** adaptadores Codex/Claude, negociação de capacidades, eventos normalizados, resultado final e sessões retomáveis. Destrava os dois nós sem chamar APIs externas.
  - **G · #559:** política por execução, escolha explícita de uso pago, limites e sanitização antes de persistir eventos. Destrava execução com permissões verificáveis. Custo ausente deve aparecer como desconhecido; não inventar orçamento monetário para Codex a partir das opções fornecidas.

- **`ravenrs-data-validation` — nova**
  - **M · #560:** validação de JSON Schema com dialeto, limites e erros estruturados. Destrava passagem confiável de JSON entre nós, independentemente da promessa de formato feita pela CLI.

- **`ravenrs-git` — nova**
  - **G · #561:** worktree descartável, artefato de diff, verificação da revisão-base, aplicação e limpeza recuperável. Destrava alterações revisáveis sem escrever diretamente no checkout de trabalho.

- **`ravenrs-app-requests` e `ravenrs-app`**
  - **M · #562 sobre a base da #258:** aprovação vinculada ao conteúdo e à política, espera persistente e reconciliação do efeito externo. Destrava escrita e publicação após decisão humana, sem repetir automaticamente efeitos incertos.

- **`ravenrs-widgets-chat`, `ravenrs-widgets-kit` e `ravenrs-widgets-grid`**
  - **M · #563, relacionada à #496:** console do agente e inspector reutilizáveis, com parada, retomada, ferramentas, métricas e artefatos. Destrava acompanhamento no painel do nó. A #497 trata LSP e não substitui executor, console ou protocolo das CLIs.

- **`ravenrs-criteria` e futuro `ravenrs-flows`**
  - **M · #533 e #525:** contexto de expressões sobre itens e contrato persistente de resultados/artefatos. Destrava prompts dinâmicos e encadeamento. O exemplo apenas compõe essas APIs.

#### Nós propostos

| Nó | Parâmetros principais | Saída | Política padrão | Pronto? |
|---|---|---|---|---|
| Codex (exec) | Executável, versão compatível, prompt por stdin/`-`, modelo, `--cd`, sandbox, diretórios adicionais, schema, prazo e sessão. | Texto final, JSON validado, eventos e métricas disponíveis; arquivos/diff quando autorizados. | `read-only`; uso pago explicitamente escolhido; sem ampliação automática de acesso. | Não; falta adaptador e executor genérico. |
| Claude (print) | Executável, prompt/entrada, modelo, `stream-json`, schema, modo de permissão, ferramentas permitidas/negadas, diretórios, orçamento e sessão. | Texto/JSON final validado, eventos, ferramentas e custo quando informado. | Perfil restritivo comprovado para a versão instalada; bloquear se não garantir só leitura. Uso pago explícito. | Não; falta adaptador e política verificável. |
| Revisão de código | Provedor escolhido, revisão-base, caminhos, diff de entrada, instruções e schema de achados. | Achados estruturados com arquivo, posição, severidade e justificativa. | Só leitura, sem aplicação de correções; nenhum provedor pago escolhido automaticamente. | Não; depende dos adaptadores e de `ravenrs-git`. |
| Aplicar diff com aprovação | Artefato imutável, hash, revisão-base, destinos permitidos e decisão humana. | Resultado da aplicação, arquivos afetados ou conflitos. | Suspenso até aprovação; conferir novamente conteúdo/base; recusar divergências. | Não; faltam serviço git e vínculo da aprovação. |

#### Exemplo de fluxo

1. **Issue nova:** o conector entrega identificador, etiqueta, título e corpo; o motor filtra a etiqueta X e deduplica o evento.
2. **Preparar contexto:** resolver repositório/revisão e montar o prompt por expressões. Fixar política de só leitura e esquema `{ lacunas: [...], readme_markdown: "..." }`.
3. **Codex só lendo:** após escolha explícita do provedor pago e diagnóstico da CLI, executar `codex exec` com sandbox `read-only`, `--json`, diretório e `--output-schema`; enviar o prompt por stdin com `-`. O agente retorna o conteúdo proposto do README, sem gravá-lo.
4. **Validar JSON pelo esquema:** extrair o resultado final e validar na biblioteca. Falha de processo, conclusão ausente ou JSON inválido interrompe o caminho de sucesso.
5. **Gravar README:** um nó de arquivo cria a proposta numa worktree descartável, limitado ao caminho autorizado. Essa escrita exige autorização prévia para esse efeito isolado; sem ela, abre-se uma tarefa de aprovação antes de gravar.
6. **Revisar e aprovar:** coletar diff completo e apresentar a proposta de publicação. Persistir decisão vinculada ao hash, revisão-base, branch e destino; alterações posteriores exigem nova aprovação.
7. **Abrir PR em rascunho:** somente depois da aprovação, o nó git/conector cria commit, publica a branch e abre a PR. Credenciais pertencem ao conector; o agente não recebe autoridade para publicar. Em resultado incerto, reconciliar antes de tentar novamente.

#### Fase de construção e prova

Esta área entra **depois do motor de execução e itens, das credenciais/consentimentos e do console básico**. A sequência interna é: executor genérico → adaptadores e validação → resultado encadeável → worktree e aprovação → publicação pelo conector. O console específico do agente consome os mesmos eventos do trabalhador.

A prova inicial deve rodar sem janela, rede, login ou assinatura: uma CLI falsa configurável recebe argumentos, ambiente, pasta e stdin, e reproduz fixtures JSONL de ambos os protocolos. Os testes devem cobrir:

- Prompt grande e fechamento de stdin; stdout/stderr simultâneos, bytes fragmentados e última linha sem newline.
- Eventos desconhecidos, JSON malformado, resultado incompatível com o schema, término sem mensagem final e saída não zero.
- Demora, timeout, cancelamento e subprocesso descendente; verificar que toda a árvore termina e os leitores se encerram.
- Versão incompatível, executável ausente e erro simulado de autenticação; nenhuma tentativa de login pelo app.
- Retomada com identificador persistido, reinício durante aprovação e efeito externo incerto sem repetição cega.
- Ausência de escrita/publicação antes da autorização correspondente; diff alterado ou base divergente invalida a aplicação.
- Segredos artificiais em argumentos, ambiente e eventos; confirmar sanitização dos registros.

Fixtures devem ser sintéticas ou sanitizadas e versionadas por protocolo. Validar também limites de memória do console e continuidade da execução com o painel fechado.

#### Não verificado

- Não foram executadas CLIs reais, builds ou testes, nem alterados arquivos. As opções disponíveis são as informadas no pedido; versões mínimas e formatos exatos dos eventos ainda precisam de fixtures e testes de compatibilidade.
- Não foram verificados login, cobrança, suporte efetivo a orçamento/voltas, combinação de opções de retomada ou garantia de só leitura dos modos do Claude. `--add-dir` não foi tratado como mecanismo de isolamento.
- A disponibilidade de um ID de sessão sob `--ephemeral`, os campos de custo e a recuperação após cancelamento dependem do protocolo/versão e não foram presumidos.
- Ausência de janela de console no Windows e encerramento de descendentes precisam de prova nas plataformas nativas suportadas.
- Os estados remotos das issues não foram consultados: #496 e #497 foram consideradas abertas conforme o contexto; #258 identifica a base de aprovação durável, sem presumir que cubra agentes CLI.
- As outras áreas e seus conectores não foram auditados aqui. Motor, credenciais, escrita de arquivo e publicação de PR são dependências.

## Issues abertas

As lacunas viraram issues filhas da #523, uma por entrega, com história, contexto, o que fazer, dependências e critérios de aceite. Estas já existiam e não foram repetidas: #453 (L08), #96 (números na fala), #41 (realce e autocompletar do `ExpressionEditor`), #430 (histograma de durações), #66 (motor de agenda), #28 (cofre do sistema) e #227 (agenda do app). A L04 foi entregue na #397. A L26 está na #564.

**Canvas e documento**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| L01 | #527 | M | FlowCanvas: soltar a conexão no vazio abre a busca e cria o nó já ligado |
| L02 | #528 | M | FlowCanvas: mostrar uma execução vinda de fora, com contagem de itens e modo só leitura firme |
| L03 | #529 | G | Documento de fluxo: histórico, área de transferência e JSON com os dados do domínio |

**Painel do nó e expressões**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| L05 | #530 | G | Formulário de parâmetros compostos e dinâmicos gerado por esquema |
| L06 | #531 | G | Inspetor de dados aninhados com modos tabela, JSON e esquema |
| L07 | #532 | M | Arrastar um campo do inspetor para um parâmetro gera a expressão |
| L09 | #533 | G | Expressões de fluxo: interpolação no texto, contexto de itens e prévia do resultado |

**Motor**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| L10 | #525 | G | ravenrs-flows: grafo executável com itens, portas e proveniência |
| L11 | #534 | G | ravenrs-flows: operadores de controle e transformação de itens |
| L12 | #535 | G | ravenrs-flows-runtime: execução durável com fila, checkpoints, retomada e retenção |
| L13 | #536 | G | Contexto de execução com cancelamento propagado, tempo limite e novas tentativas |
| L14 | #537 | G | Gatilhos por agenda e por consulta periódica, com fuso, atraso e sem sobreposição |
| L15 | #538 | G | ravenrs-script: runtime de script isolado para o nó de código |

**Conectores e servidor**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| L16 | #539 | G | ravenrs-connectors: contrato de conector com parâmetros, credencial, opções e teste |
| L17 | #540 | M | Conector HTTP genérico: cabeçalhos, corpo binário, paginação e resposta completa |
| L18 | #541 | M | Conectores SQL para itens: consulta e comando no Postgres e no SQLite |
| L19 | #542 | G | Conectores de e-mail, arquivos e observação de pastas |
| L20 | #543 | M | Conector de planilha sem janela: itens para células e de volta |
| L21 | #544 | G | Webhooks de automação: rotas configuráveis e resposta controlada pelo fluxo |
| L22 | #545 | G | Formulário e chat como gatilhos de fluxo |

**Segurança e IA**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| L23 | #546 | G | ravenrs-secrets: cofre cifrado, referência de credencial e permissão de uso |
| L24 | #547 | G | Cliente OAuth2 para integrações: autorizar, renovar e revogar |
| L27 | #548 | G | Autorização no trabalhador e observabilidade correlacionada das execuções |
| L25 | #549 | G | ravenrs-ai: inferência sem UI, agente com ferramentas e memória por sessão |

**Vídeo e treinamento (área 13)**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| V1 | #550 | G | ravenrs-movie: guardar todas as primitivas vetoriais e oferecer gravação só vetorial |
| V2 | #551 | M | ravenrs-movie: roteiro serializável e registro de telas graváveis |
| V3 | #552 | M | ravenrs-movie: pacote de voz .voz com bytes próprios e erros de leitura |
| V4 | #553 | M | ravenrs-movie: uma sequência única de falas para o player, a trilha e as legendas |
| V5 | #554 | M | ravenrs-movie-video: MP4, cancelamento e descarte de arquivo incompleto |
| V6 | #555 | M | ravenrs-ai: conferir a narração com um reconhecedor independente |
| V7 | #556 | M | Nós de vídeo do catálogo e regravação só dos treinamentos afetados |
| V8 | #557 | G | Conector de publicação de vídeo com a credencial da pessoa e envio retomável |

**Nós de agente (área 14)**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| A1 | #558 | G | ravenrs-process: executor de processo com entrada, saída em fluxo, tempo limite e parada da árvore |
| A2 | #559 | G | ravenrs-agent-cli: nós do Codex CLI e do Claude CLI com política por nó |
| A3 | #560 | M | ravenrs-data-validation: validação de JSON Schema com erros por caminho |
| A4 | #561 | G | ravenrs-git: worktree descartável, diff como artefato e aplicação conferida |
| A5 | #562 | M | ravenrs-app-requests: aprovação vinculada ao conteúdo antes de escrever ou publicar |
| A6 | #563 | M | Console do agente no painel do nó: eventos, ferramentas, tempo, parar e retomar |

**Componentes expostos pela cobertura**

| ID | Issue | Tamanho | Entrega |
|---|---|---|---|
| C1 | #564 | M | CodeEditor: realce de JSON, shell e da linguagem de script dos nós |
| C2 | #565 | M | Chat: interromper a resposta enquanto ele está ocupado |
| C3 | #566 | M | CurveEditor: domínio dos eixos, unidades e restrição de Y crescente |
| C4 | #567 | P | InputOtp: estado desabilitado durante a verificação |
| C5 | #568 | G | Recorrência em minutos e horas, com fuso, exceções e prévia das próximas ocorrências |

## Lacunas da biblioteca

Esta lista cobre as áreas 1 a 12. As lacunas das áreas 13 e 14 estão em [Lacunas de vídeo da biblioteca](#lacunas-de-vídeo-da-biblioteca) e [Lacunas da biblioteca para os nós de agente](#lacunas-da-biblioteca-para-os-nós-de-agente); as que os usos de cada componente expõem estão em [Lacunas expostas pelos usos](#lacunas-expostas-pelos-usos).

**Tamanhos relativos:** P = extensão delimitada; M = contrato com implementação e testes em várias camadas; G = subsistema com persistência, concorrência ou execução. Não são estimativas de prazo.

Uma issue relacionada não comprova cobertura integral. As associações abaixo vêm dos documentos locais; o estado remoto das issues não foi consultado.

### `ravenrs-widgets-charts` e `ravenrs-graphics`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L01 | Finalização de conexão no vazio, com evento público e criação/conexão transacional | M | #527; relacionar #238 | Origem, coordenadas de conteúdo, cancelamento e aceitação por teclado; callback não deve contornar guard nem histórico |
| L02 | Apresentação reutilizável de execução e modo somente leitura firme | M | #528 | Contrato de estado externo para executando, sucesso, cancelado, fixado e contagens por porta; `FlowCanvas::content` permanece ponto de composição |
| L04 | ~~Pontos de dobra editáveis e exportação visual~~ | — | entregue (#397) | `FlowEdge::waypoint` e `FlowExportFormat` já estão no `master`; nada a abrir |

### `ravenrs-graphics`: documento e histórico

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L03 | Transações de edição, histórico e clipboard extensíveis ao documento completo | G | #529; relacionar #238 | API para payload de domínio, remapeamento de IDs/referências e edição atômica de geometria e parâmetros, sem colocar regras de automação no modelo geométrico |

### `ravenrs-app-spec`, `ravenrs-app` e `ravenrs-app-ui`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L05 | Esquema e formulário de parâmetros compostos e dinâmicos | G | #530; #402 é referência relacionada ao inspector | Valores embutidos, coleção fixa/repetível, visibilidade, dependências, opções assíncronas, validação e commit/cancel do agregado; aproveitar `EntityInfo` e editores existentes |

### `ravenrs-widgets-kit` e `ravenrs-widgets-lists`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L06 | Inspector de dados aninhados com modos tabela, JSON e esquema | G | #531 | Fonte comum de dados e caminhos, arrays, busca, amostragem/limites, distinção entre ausente e nulo e suporte a grandes documentos |
| L07 | Transferência de referência de campo para editor de parâmetro | M | #532 | Payload público sobre `DragPayload`, caminho tipado, escape e ação equivalente por teclado; depende de L06 e L09 |
| L09 | Editor contextual de expressões com completion, destaque e prévia | G | #533 | Dialeto e resolvedor de contexto, diagnósticos por posição, amostra selecionada e orçamento de avaliação; reaproveitar o catálogo existente |

### `ravenrs-widgets-grid`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L08 | Preservação e apresentação fiel de valores do inspector | M | #453; #451 para volume e #454 para exportação remota | Contrato de inteiros grandes, decimais, datetime e nulo; tabela deve ser projeção do JSON original, sem conversão destrutiva para `f64` |

### `ravenrs-criteria`

A parte semântica de **L09** pertence aqui: interpolação `{{ }}`, contexto de namespaces, acesso a nós/itens/variáveis e resultado com diagnóstico. A parte visual permanece no editor. Deve haver uma decisão explícita sobre o dialeto aceito; delimitadores semelhantes não prometem compatibilidade com outra linguagem.

### Crate proposta `ravenrs-flows`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L10 | Contratos e planejamento de grafo executável | G | #525; relacionar #79 e #82 | Definição versionada, catálogo/executor, portas, lotes, proveniência, dados fixados, execução parcial e subfluxos, sem dependência da UI |
| L11 | Operadores reutilizáveis de controle e transformação | G | #534 | L10; semântica explícita de ramos, merge, lotes, set, filtro, ordenação, agrupamento, divisão, junção e propagação de erro |

### Crate proposta `ravenrs-flows-runtime`, sobre a camada de aplicação

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L12 | Execução durável, fila, checkpoints, replay e retenção | G | #535; relacionar #79, #241 e #255 | L10, persistência de estados e dados, reivindicação atômica por worker, leases, recuperação e coleta de blobs |
| L13 | Contexto de execução com cancelamento, timeout e tentativas | G | #536; relacionar #255 | Token propagado aos providers, relógio controlável, classificação de falha e resultado desconhecido; timeout não pode ser somente uma marca no banco |
| L14 | Serviço genérico de agenda e polling | G | #66 cobre parte; complemento em #537 | Extrair cron/fuso do servidor de relatórios, definir intervalos, cursores, atraso, sobreposição e início idempotente via L12 |

A #258 fornece tarefas e decisões humanas; pode integrar uma espera por aprovação, mas não substitui L10–L14.

### Crate proposta `ravenrs-script`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L15 | Runtime de script isolado e sem UI | G | #538 | Escolha de linguagem, limites, cancelamento verificável e conversão de valores; acesso externo somente pelas capacidades concedidas |

### Crate proposta `ravenrs-connectors`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L16 | Contrato de conector: parâmetros, credenciais, opções e teste | G | #539; relacionar #82 | L05, L10, L13 e L23; descoberta de capacidades, erros estruturados e execução fora da thread de UI |
| L17 | Cliente HTTP genérico | M | #540 | Extrair capacidades úteis de `ravenrs-app-remote` sem acoplar o conector ao protocolo desse cliente; corpo binário, cabeçalhos, autenticação e limites |
| L18 | Adaptadores SQL para itens | M | #541 | Reusar `SqlBackend` e os dois backends; parâmetros vinculados, tipos preservados, transação e conexão por worker |
| L19 | Providers de e-mail, arquivos e observação de diretórios | G | #542; #261/#447 relacionadas ao ciclo de arquivos | Extrair envio do relatório e oferecer leitura/escrita/recebimento/observação por capacidades; usar L13 e L23 |
| L20 | Adaptador de planilha sem janela | M | #543 | Reusar `Session::execute_headless`; mapeamento de objetos, cabeçalhos, tipos e políticas de fórmulas |

### `ravenrs-app-server`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L21 | Ingresso HTTP de automação e resposta controlada pelo fluxo | G | #544; relacionar #256 | Rotas/métodos, corpo e cabeçalhos, resposta imediata ou correlacionada, prazo e enqueue durável; preservar o hook assinado existente |
| L22 | Contratos de submissão de formulário e sessão de chat | G | #545; relacionar #260 e #259 | L05, L10 e L12; validação, versão, identidade/audiência e correlação da resposta. O widget de chat continua em sua crate |

### Crate proposta `ravenrs-secrets`

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L23 | Cofre, referências de credencial e autorização de uso | G | #546 | Cifragem autenticada, gestão de chave, redação, escrita sem revelação, rotação e uso sob identidade do executor; integrar L16 e L27 |
| L24 | Cliente OAuth2 | G | #547 | L23, callback local, state, PKCE quando aplicável, renovação serializada de tokens e revogação |

### `ravenrs-ai` e integração opcional com ferramentas remotas

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L25 | Inferência sem UI, agente com ferramentas e memória | G | #549; #266 é relacionada, com escopo distinto | Interface de inferência desacoplada de `UiWaker`, limites de passos/tokens, ferramenta com esquema, persistência por sessão e cliente remoto opcional |

O servidor documental de `ravenrs-mcp` não deve ser convertido implicitamente em executor de ferramentas do app.

### `ravenrs-widgets-chat`: editor de código

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L26 | Linguagem de script, completion e diagnóstico no editor | G | #564 | Linguagem escolhida em L15, provider de completion e API pública de destaque; não usar os tipos internos de pintura como contorno no exemplo |

### `ravenrs-app`, `ravenrs-app-server` e infraestrutura de execução

| ID | Peça única | Tamanho | Issue | O que a destrava |
|---|---|---|---|---|
| L27 | Autorização em comandos/workers e observabilidade correlacionada | G | #548; relacionar #79, #253 e #241 | Identidade revalidada, decisão de editar/executar/usar segredo, auditoria sem vazamento, logs correlacionados e métricas de cardinalidade limitada |

**Referências de backlog:** #238 é evolução do editor de grafos, não aceite de motor executável; #396/#397 aparecem como dobra/exportação; #79 é encaminhamento amplo; #258 é processo de decisões humanas. Não foram encontradas referências a #496 ou #497 nos documentos, crates e exemplos pesquisados. Seu conteúdo e estado permanecem não verificados.

## O que é só do app

O exemplo deve conter composição, fixtures e regras de produto. Os motores, editores e providers acima ficam na biblioteca.

| Modelo ou regra | Responsabilidade do exemplo |
|---|---|
| Documento do fluxo | ID, nome, descrição, tags, pasta, projeto, versão do formato, revisão, nós, conexões e apresentação |
| Nó configurado | ID estável, tipo e versão, parâmetros literais ou expressões, referência de credencial, política de erro, tentativas e estado desativado |
| Catálogo | Quais tipos demonstrar, categorias, textos, ícones, valores iniciais e associação aos executores públicos |
| Portas | Significado das saídas de verdadeiro/falso/erro e restrições entre os tipos oferecidos |
| Item | Conteúdo JSON de cada objeto e referências a binários; identidade e proveniência seguem o contrato da biblioteca |
| Dados fixados | Conteúdo de amostra por nó, autoria e revisão; não usar silenciosamente em execução ativa |
| Execução | Fluxo/revisão, modo, ator, projeto, entrada, tempos e política de retenção; estados e checkpoints seguem o runtime |
| Publicação | Rascunho separado da revisão ativa; validar antes de ativar; importar não instala gatilhos |
| Reexecução | Escolha explícita entre revisão original e atual; registrar origem do replay e possibilidade de repetir efeitos |
| Projetos e papéis | Entidades de projeto/membro e concessões de visualizar, editar, executar e administrar |
| Credenciais | Nome, tipo, projeto e referência opaca ao cofre; o documento exportado nunca leva o segredo |
| Variáveis | Nomes, valores não secretos, escopos e precedência; segredos continuam no cofre |
| Modelos | Pequeno conjunto de fluxos demonstrativos, com credenciais e variáveis a preencher |
| Políticas operacionais | Limites de concorrência, tamanho de item, duração, retenção e pastas acessíveis |
| Conteúdo de UI | Comandos, colunas, mensagens, estados vazios, traduções e composição dos componentes |

A separação de módulos pode seguir `examples/raven-code`: domínio e operações sem tela em módulos próprios, composição em `app`, executável fino e testes de comportamento separados. `examples/dock-workbench` demonstra composição de uma janela inteira sobre `DockArea`.

## Cobertura de componentes

O objetivo é validar o máximo de componentes do RavenRS com uso real: componente forçado não valida nada.

| Conjunto | Natural | Forçado | Não cabe | Total | Natural (%) |
|---|---:|---:|---:|---:|---:|
| Catálogo | 160 | 14 | 7 | 181 | 88,4% |
| Fora do catálogo: widgets públicos dos motores e da aplicação | 21 | 2 | 0 | 23 | 91,3% |

Esta é uma matriz de **cobertura proposta**, baseada na leitura das definições e APIs, não uma afirmação de funcionalidades implementadas em `raven-flows`. “Atende” significa que a API suporta o uso descrito; persistência, execução, autenticação e chamadas aos motores continuam sendo responsabilidades da aplicação.

Usos internos contam quando fazem parte de uma interação real: por exemplo, `Kbd` em `ShortcutSheet`, `SplitHandle` em `SplitPanes`, `Timeline` em `MovieEditor` e `Ribbon` em `Designer`. O minimapa interno de `FlowCanvas` **não** conta como uso de `Minimap`.

As lacunas devem ser resolvidas no componente responsável e, quando necessário, em seu modelo de apoio. O exemplo compõe os componentes existentes e conecta dados e ações, sem criar componentes visuais próprios.

### Por tela

- **Lista de fluxos:** `DataGrid` para pesquisar, ordenar e selecionar fluxos; `CardView` como apresentação alternativa; `SegmentedControl` para trocar a apresentação; `FavoriteToggle`, `TagBox` e `InlineRename` para favoritos, etiquetas e nomes; `SelectionBar` para ações em lote; `Pagination` para a navegação paginada.
- **Editor:** `FlowCanvas` para nós, portas, conexões, seleção, agrupamento e histórico; `DockArea` para distribuir os painéis; `DocumentTabs` para fluxos abertos; `ToolRail` para ferramentas; `Menubar`, `Toolbar`, `ButtonGroup`, `SplitButton`, `ContextMenu` e `CommandPalette` para comandos; `ShortcutSheet`, `ShortcutHintBar` e `Kbd` para atalhos; `StatusBar` para estado de salvamento e seleção.
- **Painel do nó:** `FormLayout`, `FormField`, `Label`, `Accordion`, `Disclosure` e `Collapse` para parâmetros; `Tabs` para parâmetros, entrada e saída; `TextField`, `TextArea`, `NumberField`, `Stepper`, `Select`, `Combobox`, `LookupField`, `Checkbox`, `RadioGroup`, `Toggle`, `ToggleButton` e `ToggleGroup` para valores; `CurveEditor` para atraso crescente entre tentativas; `Sheet` para abrir o painel quando faltar espaço.
- **Expressões:** `ExpressionEditor` para campos, parâmetros e fórmulas na linguagem de critérios; `TreeView` para explorar a entrada; `Playground` para experimentar transformações com dados de amostra; `CodeEditor` para editar amostras e inspecionar resultados; `Splitter` para comparar expressão e resultado.
- **Execuções:** `DataGrid`, `FilterBuilder` e `DateRangePicker` para histórico; `Stat`, `AreaChart`, `BarChart`, `Chart`, `ChartCard` e `ChartReference` para indicadores; `Histogram` para distribuição de latências; `SankeyChart` para itens encaminhados entre ramos.
- **Detalhe da execução:** `FlowCanvas` bloqueado para o percurso executado; `Timeline` para duração e sobreposição das etapas; `VirtualList` e `DaySeparator` para registros; `Table` e `PropertyRow` para metadados; `SplitPanes` e seu `SplitHandle` para entrada, saída e registros; `AttachmentStrip`, `FileBrowser` e `ImageLightbox` para artefatos.
- **Credenciais:** `ListView`, `DetailView` e `EntityForm` para cadastro; `TextField` seguro para segredos; `InputOtp` para um desafio de segundo fator de um serviço conectado; `DatePicker` para validade; `ConfirmDialog` para revogação.
- **Variáveis:** `EntityPane` para listar e editar variáveis tipadas; `DataGrid` para inspeção em lote; `CodeEditor` para valores estruturados; `HelpTip` e `HoverCard` para explicar escopo e referências.
- **Modelos:** `CardView` para modelos de fluxos; `Image`, `AspectRatio`, `Badge`, `FavoriteToggle` e `ScrollEdgeArrow` para prévias, categorias e favoritos; `Wizard`, `StepIndicator` e `StepBadge` para configurar um fluxo a partir de um modelo.
- **Configurações e tema:** `ThemeEditor` para ajustes; `ThemeSwitcher` para opções explícitas; `ThemePicker` como acesso compacto; `ColorPicker`, `ColorSwatch` e `SwatchGrid` para cores de categorias; `ReleaseNotes` para mudanças da aplicação; `List`, `Section` e `Footer` para organização e informações de versão.
- **Agenda:** `RecurrenceEditor`, `DatePicker`, `TimePicker` e `DateTimePicker` para gatilhos; `Calendar` para escolher exceções; `EventCalendar` para ocorrências; `YearView` para localizar dias com agendamento; `Scheduler` para janelas de execução por recurso.
- **Treinamentos:** `MoviePlayer` para filme vetorial com narração; `MovieAssistant` para localizar explicações; `MovieChat` para perguntas vinculadas a trechos; `MovieEditor` para manter os filmes; `AudioPlayerButton` para conferir a narração.
- **Nós de vídeo:** `MovieEditor`, `Composition`, `MovieFrame` e `SocialFrame` para editar e conferir saídas; `Filmstrip` para quadros; `ImageField` e `FileUpload` para recursos; `MaskedField` para timecode; `NinePointAnchor` para posição de legenda ou marca; `Slider` e `ValueSlider` para parâmetros; `CompareView` para comparar quadros de versões.
- **Nós de agente:** `Chat` para testar instruções e acompanhar respostas; `AttachmentStrip` para arquivos de contexto; `CodeEditor` para instruções estruturadas; `CodeBlock` para copiar comandos de instalação; `FloatingPanel` para acompanhar a conversa sem fechar o editor.
- **IA local:** `ModelPicker` para baixar, testar, selecionar e remover modelos; `ProgressBar` e `MultiProgress` para downloads e preparação; `Spinner` para operações sem progresso mensurável.
- **Relatórios:** `Designer` para editar o modelo; `ParameterPanel` para parâmetros; `DocumentViewer` para o documento renderizado; `Ribbon`, `Ruler` e `ColorPair` para edição; `PagedDocumentView` para PDFs e slides rasterizados pelos motores.
- **Planilhas:** `Spreadsheet` para preparar tabelas de entrada, fórmulas e conferir resultados tabulares.
- **Inspetor de artefatos:** `ZoomPanView`, `ZoomControl` e `Minimap` compartilham o enquadramento de imagens grandes; `ImageLightbox` abre imagens; `FileBrowser` navega pelos arquivos produzidos.
- **Início e boas-vindas:** `DashboardView` e `TileLayout` para resumos reorganizáveis; `HeroBanner` para o primeiro fluxo; `EmptyState` para ausência de dados; `SplashScreen` durante inicialização; `Anchor`, `Link` e `RichText` para ajuda navegável.
- **Ajuda e diagnóstico:** `FlightRecorder` e `BugReporter` para revisar e relatar uma interação problemática; `Dialog` para detalhes; `Barcode` para abrir no celular a URL de um webhook de teste; `Sticky` para manter os comandos de um roteiro de ajuda visíveis.
- **Estrutura compartilhada:** `AppShell`, `Sidebar`, `SidebarBrand`, `TitleBar`, `PageHeader`, `Breadcrumb`, `Card`, `Grid`, `HStack`, `VStack`, `ZStack`, `Spacer`, `Divider` e `ScrollView`; `Button`, `IconButton`, `DropdownMenu`, `Popover`, `Tooltip`, `Icon`, `IconTile`, `Text`, `Avatar`, `OverlayBadge`, `Badge` e `Dot`; `Alert`, `Toaster` e `Skeleton` para mensagens e carregamento. `ActionContainer` e `ActionButton` apresentam ações registradas no modelo da aplicação.

### Componente a componente

“Sim” refere-se estritamente ao uso indicado. Componentes classificados como Forçado ou Não cabe não devem ser adicionados para elevar a contagem.

#### Catálogo

`HStack` e `VStack` são `pub struct` geradas pela macro `stack!`: a declaração fica em `stack.rs:19`, com as respectivas invocações indicadas abaixo.

| Componente | Definição (arquivo:linha) | Decisão | Tela e uso | Atende? / lacuna exposta |
|---|---|---|---|---|
| Accordion | `crates/ravenrs-widgets-kit/src/nav.rs:2031` | Natural | Painel do nó: grupos de parâmetros recolhíveis. | Sim. |
| Alert | `crates/ravenrs-widgets-base/src/display.rs:516` | Natural | Painel do nó: configuração inválida e orientação para corrigir. | Sim. |
| Anchor | `crates/ravenrs-widgets-base/src/page_motion.rs:217` | Natural | Ajuda: destinos para links de seções do roteiro. | Sim. |
| AreaChart | `crates/ravenrs-widgets-charts/src/chart.rs:2538` | Natural | Execuções: tendência compacta do volume por intervalo regular. | Sim. |
| AspectRatio | `crates/ravenrs-widgets-base/src/choice.rs:1147` | Natural | Modelos e vídeo: preservar proporção das prévias. | Sim. |
| AttachmentStrip | `crates/ravenrs-widgets-chat/src/conversation.rs:117` | Natural | Nós de agente: arquivos selecionados como contexto, com remoção. | Sim. |
| AudioPlayerButton | `crates/ravenrs-widgets-chat/src/conversation.rs:382` | Natural | Treinamentos: ouvir e conferir um trecho de narração. | Sim. |
| Avatar | `crates/ravenrs-widgets-base/src/display.rs:149` | Natural | Credenciais: identificar a conta conectada por imagem ou iniciais. | Sim. |
| Badge | `crates/ravenrs-widgets-base/src/display.rs:62` | Natural | Lista de fluxos: ativo, rascunho e quantidade de falhas. | Sim. |
| BarChart | `crates/ravenrs-widgets-charts/src/chart.rs:2503` | Natural | Execuções: comparar falhas por tipo de nó. | Sim. |
| Barcode | `crates/ravenrs-widgets-charts/src/chart.rs:2818` | Natural | Webhook de teste: QR da URL para disparar pelo celular. | Sim; a aplicação fornece a URL. |
| Breadcrumb | `crates/ravenrs-widgets-kit/src/nav.rs:1741` | Natural | Detalhe da execução: voltar ao fluxo e ao histórico. | Sim. |
| Button | `crates/ravenrs-widgets-base/src/button.rs:189` | Natural | Editor: salvar, executar e cancelar ações. | Sim. |
| ButtonGroup | `crates/ravenrs-widgets-base/src/button.rs:461` | Natural | Editor: agrupar comandos de alinhamento dos nós. | Sim. |
| Calendar | `crates/ravenrs-widgets-base/src/calendar.rs:58` | Natural | Agenda: selecionar um dia de exceção e adicioná-lo às exclusões. | Sim para seleção de uma data. |
| Canvas | `crates/ravenrs-widgets-base/src/canvas.rs:28` | Forçado | Desenho manual duplicaria os componentes de grafo, gráficos e filmes. | — |
| Card | `crates/ravenrs-widgets-base/src/card.rs:13` | Natural | Início: agrupar um resumo e suas ações. | Sim. |
| CardView | `crates/ravenrs-widgets-kit/src/tiles.rs:572` | Natural | Lista de fluxos e Modelos: cartões selecionáveis com abertura. | Sim. |
| Carousel | `crates/ravenrs-widgets-base/src/extras.rs:70` | Forçado | Ocultar modelos em slides não melhora sua busca ou comparação. | — |
| Chart | `crates/ravenrs-widgets-charts/src/chart.rs:1240` | Natural | Execuções: séries de sucesso, erro e duração ao longo do tempo. | Sim. |
| ChartCard | `crates/ravenrs-widgets-charts/src/chart.rs:2586` | Natural | Execuções: gráfico com título, período, ação e observação. | Sim. |
| ChartReference | `crates/ravenrs-widgets-charts/src/chart.rs:1115` | Natural | Execuções: linha do limite de duração esperado. | Sim; é configuração do gráfico. |
| Chat | `crates/ravenrs-widgets-chat/src/chat.rs:396` | Natural | Nós de agente: testar instruções e acompanhar resposta incremental. | Parcial: falta ação nativa de interromper a resposta em andamento. |
| Checkbox | `crates/ravenrs-widgets-base/src/inputs.rs:166` | Natural | Painel do nó: incluir cabeçalhos ou campos opcionais. | Sim. |
| CodeBlock | `crates/ravenrs-widgets-kit/src/release_notes.rs:23` | Natural | Nós de agente: copiar um comando de instalação ou diagnóstico. | Sim; é texto copiável, não editor multilinha. |
| CodeEditor | `crates/ravenrs-widgets-chat/src/code_editor.rs:100` | Natural | Painel do nó e Variáveis: editar JSON e comandos. | Parcial: `CodeLanguage` oferece apenas Plain e Rust; faltam JSON e shell. |
| Collapse | `crates/ravenrs-widgets-base/src/motion.rs:499` | Natural | Painel do nó: abrir parâmetros condicionais quando uma opção é ativada. | Sim. |
| ColorPair | `crates/ravenrs-widgets-base/src/choice.rs:911` | Natural | Relatórios: editar cores de texto e fundo de um estilo. | Sim. |
| ColorPicker | `crates/ravenrs-widgets-base/src/color_picker.rs:122` | Natural | Configurações: escolher a cor de uma categoria de fluxos. | Sim. |
| ColorSwatch | `crates/ravenrs-widgets-base/src/choice.rs:398` | Natural | Configurações: escolher uma cor predefinida para uma categoria. | Sim. |
| ColorWheel | `crates/ravenrs-widgets-imaging/src/imaging.rs:709` | Não cabe | Ajuste de matiz e saturação para tratamento de imagem não faz parte do produto. | Coberto em `examples/component-gallery`. |
| Combobox | `crates/ravenrs-widgets-base/src/floating.rs:1190` | Natural | Painel do nó: pesquisar uma operação entre muitas opções locais. | Sim. |
| CommandPalette | `crates/ravenrs-widgets-base/src/floating.rs:998` | Natural | Editor: localizar comandos e abrir fluxos pelo teclado. | Sim. |
| CompareView | `crates/ravenrs-widgets-imaging/src/imaging/viewing.rs:79` | Natural | Nós de vídeo: comparar o mesmo quadro de duas versões renderizadas. | Sim para imagens; não sincroniza vídeos. |
| ConfirmDialog | `crates/ravenrs-widgets-kit/src/commands.rs:814` | Natural | Credenciais: confirmar revogação; fluxos: confirmar exclusão. | Sim, inclusive confirmação digitada. |
| ContextMenu | `crates/ravenrs-widgets-base/src/floating.rs:24` | Natural | Lista de fluxos: duplicar, exportar e excluir pelo menu contextual. | Sim. |
| CountUp | `crates/ravenrs-widgets-base/src/page_motion.rs:86` | Forçado | Animar contagens operacionais não acrescenta informação ao indicador. | — |
| CropOverlay | `crates/ravenrs-widgets-imaging/src/imaging/transform.rs:1286` | Não cabe | Recorte fotográfico interativo está fora do escopo dos nós de vídeo vetorial. | Coberto em `examples/component-gallery`. |
| CurveEditor | `crates/ravenrs-widgets-imaging/src/imaging.rs:946` | Natural | Painel do nó: definir a curva de atraso entre tentativas. | Parcial: faltam eixos com unidades e restrição opcional de crescimento de Y. |
| DataGrid | `crates/ravenrs-widgets-grid/src/grid/view.rs:56` | Natural | Lista de fluxos e Execuções: consulta, seleção, filtros e exportação. | Sim; possui suporte remoto, edição e identidade por chave. |
| DatePicker | `crates/ravenrs-widgets-base/src/calendar.rs:343` | Natural | Credenciais: validade; Agenda: primeiro dia do gatilho. | Sim. |
| DateRangePicker | `crates/ravenrs-widgets-base/src/calendar.rs:533` | Natural | Execuções: filtrar o histórico por período e predefinições. | Sim. |
| DateTimePicker | `crates/ravenrs-widgets-kit/src/editors.rs:1912` | Natural | Painel do nó: executar ou esperar até uma data e hora locais. | Sim para horário local; o fuso é configuração explícita do gatilho. |
| DaySeparator | `crates/ravenrs-widgets-kit/src/status.rs:912` | Natural | Detalhe da execução: separar registros por dia. | Sim. |
| Dialog | `crates/ravenrs-widgets-base/src/overlay.rs:1325` | Natural | Diagnóstico: mostrar detalhes de um erro ou teste de conexão. | Sim. |
| Disclosure | `crates/ravenrs-widgets-kit/src/nav.rs:1860` | Natural | Painel do nó: seção de tentativas com ativação e estado alterado. | Sim. |
| Divider | `crates/ravenrs-widgets-base/src/display.rs:274` | Natural | Editor: separar comandos e regiões do painel. | Sim. |
| DockArea | `crates/ravenrs-widgets-dock/src/dock/area.rs:278` | Natural | Editor: organizar canvas, parâmetros e console em painéis. | Sim; layout vinculável, guardas e eventos de alteração. |
| DocumentTabs | `crates/ravenrs-widgets-lists/src/lists/document_tabs.rs:98` | Natural | Editor: alternar fluxos abertos, fechar, renomear e reordenar. | Sim. |
| Dot | `crates/ravenrs-widgets-kit/src/status.rs:39` | Natural | Editor: indicar alteração pendente de salvamento. | Sim, com rótulo acessível. |
| DropZone | `crates/ravenrs-widgets-kit/src/files.rs:148` | Natural | Lista de fluxos: importar arquivos arrastados para a tela. | Sim; aceita limites e informa recusas. |
| DropdownMenu | `crates/ravenrs-widgets-base/src/overlay.rs:478` | Natural | Lista de fluxos: ações secundárias de cada registro. | Sim. |
| EmptyState | `crates/ravenrs-widgets-kit/src/status.rs:505` | Natural | Lista de fluxos: orientar a criação do primeiro fluxo. | Sim. |
| EventCalendar | `crates/ravenrs-widgets-scheduler/src/event_calendar.rs:153` | Natural | Agenda: consultar ocorrências previstas e abrir o fluxo correspondente. | Sim; a aplicação calcula as ocorrências. |
| ExpressionEditor | `crates/ravenrs-widgets-kit/src/expression_editor.rs:59` | Natural | Expressões: mapear campos e parâmetros com validação de critérios. | Parcial: falta avaliação de amostra e apresentação do resultado dentro do editor. |
| Eyedropper | `crates/ravenrs-widgets-imaging/src/imaging/overlays.rs:1022` | Não cabe | Coletar cores de pixels não é uma interação necessária da automação. | Coberto em `examples/component-gallery`. |
| FavoriteToggle | `crates/ravenrs-widgets-base/src/choice.rs:490` | Natural | Lista de fluxos e Modelos: marcar favoritos. | Sim. |
| FileBrowser | `crates/ravenrs-widgets-lists/src/lists/file_browser.rs:153` | Natural | Detalhe da execução: navegar pelos artefatos produzidos. | Sim; listagem e abertura são fornecidas pela aplicação. |
| FileUpload | `crates/ravenrs-widgets-kit/src/files.rs:422` | Natural | Painel do nó: selecionar arquivos de entrada. | Sim. |
| Filmstrip | `crates/ravenrs-widgets-lists/src/lists/filmstrip.rs:58` | Natural | Nós de vídeo: escolher quadros renderizados para inspeção. | Sim; miniaturas vêm do motor. |
| FilterBuilder | `crates/ravenrs-widgets-kit/src/filter_builder.rs:325` | Natural | Execuções: combinar condições de estado, duração e fluxo. | Sim na linguagem de critérios. |
| FloatingPanel | `crates/ravenrs-widgets-chat/src/chat.rs:731` | Natural | Nós de agente: acompanhar conversa em painel móvel e minimizável. | Sim. |
| FloatingToolbar | `crates/ravenrs-widgets-kit/src/commands.rs:407` | Natural | Nós de vídeo: comandos de copiar quadro, ajustar vista e abrir edição sobre a prévia. | Sim. |
| FlowCanvas | `crates/ravenrs-widgets-charts/src/flow.rs:599` | Natural | Editor: editar o grafo; execução: mostrar grafo bloqueado e estados dos nós. | Sim; histórico, validação, guardas, seleção, estados e minimapa já existem. |
| Footer | `crates/ravenrs-widgets-base/src/stack.rs:842` | Natural | Configurações: versão, licença e links de diagnóstico. | Sim. |
| FormField | `crates/ravenrs-widgets-base/src/extras.rs:1143` | Natural | Painel do nó: rótulo, obrigatoriedade, ajuda e erros dos parâmetros. | Sim. |
| FormLayout | `crates/ravenrs-widgets-kit/src/form_layout.rs:115` | Natural | Painel do nó: grupos, colunas e resumo dos erros de configuração. | Sim. |
| GlassCard | `crates/ravenrs-widgets-base/src/card.rs:163` | Forçado | Transparência não resolve uma necessidade da tela; o desfoque ainda é planejado. | — |
| Grid | `crates/ravenrs-widgets-base/src/stack.rs:782` | Natural | Início: organizar indicadores em colunas de mesma largura. | Sim. |
| HStack | `crates/ravenrs-widgets-base/src/stack.rs:19`, invocação `:111` | Natural | Estrutura: alinhar rótulos, campos e ações horizontalmente. | Sim; struct gerada pela macro. |
| HelpTip | `crates/ravenrs-widgets-base/src/overlay.rs:189` | Natural | Painel do nó: explicar tentativas, escopo e comportamento em erro. | Sim. |
| HeroBanner | `crates/ravenrs-widgets-kit/src/status.rs:597` | Natural | Boas-vindas: orientar a criação ou importação do primeiro fluxo. | Sim. |
| Histogram | `crates/ravenrs-widgets-imaging/src/imaging.rs:217` | Natural | Execuções: distribuição de latências a partir de contagens por faixa. | Parcial: faltam domínio, unidades e identificação das faixas; a API atual privilegia canais de imagem. |
| HoverCard | `crates/ravenrs-widgets-base/src/floating.rs:84` | Natural | Variáveis: mostrar tipo, escopo e exemplo ao inspecionar uma referência. | Sim para conteúdo complementar. |
| Icon | `crates/ravenrs-widgets-base/src/icon.rs:10` | Natural | Estrutura: identificar tipos de nó e comandos. | Sim. |
| IconButton | `crates/ravenrs-widgets-base/src/button.rs:507` | Natural | Editor: desfazer, refazer e abrir configurações com nome acessível. | Sim. |
| IconTile | `crates/ravenrs-widgets-kit/src/status.rs:423` | Natural | Modelos: identificar visualmente a categoria de um fluxo. | Sim. |
| Image | `crates/ravenrs-widgets-base/src/image.rs:61` | Natural | Modelos e artefatos: mostrar miniaturas e quadros. | Sim. |
| ImageField | `crates/ravenrs-widgets-kit/src/files.rs:649` | Natural | Nós de vídeo: escolher uma imagem de marca ou capa. | Sim, com a feature `decode`. |
| ImageLightbox | `crates/ravenrs-widgets-imaging/src/imaging/viewing.rs:424` | Natural | Detalhe da execução: abrir imagens produzidas e navegar entre elas. | Sim. |
| InlineRename | `crates/ravenrs-widgets-kit/src/quick_edit.rs:345` | Natural | Lista de fluxos: renomear sem abrir um formulário completo. | Sim. |
| InputOtp | `crates/ravenrs-widgets-base/src/extras.rs:981` | Natural | Credenciais: responder ao segundo fator de um serviço conectado. | Parcial: falta estado desabilitado durante a verificação do código. |
| Kbd | `crates/ravenrs-widgets-base/src/kbd.rs:13` | Natural | Editor: teclas exibidas por `ShortcutSheet` e `ShortcutHintBar`. | Sim; uso interno desses componentes. |
| KeyBar | `crates/ravenrs-widgets-kit/src/counter.rs:56` | Forçado | Comandos globais sempre ativos duplicariam a estrutura de atalhos do editor. | — |
| Label | `crates/ravenrs-widgets-base/src/choice.rs:1071` | Natural | Painel do nó: nomes de campos e indicação de obrigatoriedade. | Sim, inclusive dentro de `FormField`. |
| Link | `crates/ravenrs-widgets-base/src/links.rs:17` | Natural | Ajuda: abrir documentação e navegar para âncoras internas. | Sim. |
| List | `crates/ravenrs-widgets-kit/src/data.rs:14` | Natural | Configurações: relação curta de serviços e seus estados. | Sim. |
| LookupField | `crates/ravenrs-widgets-kit/src/lookup.rs:120` | Natural | Painel do nó: localizar credencial ou fluxo em catálogo remoto. | Sim; respostas antigas são descartadas e falhas podem ser informadas. |
| Lottie | `crates/ravenrs-widgets-imaging/src/lottie.rs:27` | Forçado | Uma animação decorativa não substitui o treinamento em filme vetorial. | — |
| Marquee | `crates/ravenrs-widgets-imaging/src/imaging/overlays.rs:835` | Forçado | Contorno animado de seleção duplicaria as seleções dos editores existentes. | — |
| MaskedField | `crates/ravenrs-widgets-kit/src/editors.rs:1616` | Natural | Nós de vídeo: entrada de timecode no formato hora:minuto:segundo. | Sim para a máscara; a aplicação valida o intervalo temporal. |
| Menubar | `crates/ravenrs-widgets-base/src/extras.rs:830` | Natural | Editor: menus de arquivo, edição e visualização. | Sim. |
| Minimap | `crates/ravenrs-widgets-imaging/src/imaging.rs:1840` | Natural | Inspetor de artefatos: navegar por uma imagem grande compartilhando `ZoomView`. | Sim; não é o minimapa interno de `FlowCanvas`. |
| ModelPicker | `crates/ravenrs-widgets-kit/src/model_picker.rs:129` | Natural | IA local: baixar, cancelar, testar, usar e remover modelos. | Sim; execução e download são callbacks da aplicação. |
| MultiProgress | `crates/ravenrs-widgets-kit/src/status.rs:728` | Natural | IA local: acompanhar downloads e preparação de vários arquivos. | Sim. |
| NinePointAnchor | `crates/ravenrs-widgets-base/src/choice.rs:778` | Natural | Nós de vídeo: escolher o canto ou centro de uma legenda ou marca. | Sim; a aplicação converte a posição em parâmetros do motor. |
| NumberField | `crates/ravenrs-widgets-kit/src/editors.rs:1261` | Natural | Painel do nó: timeout, limite de itens e tamanho do lote. | Sim, com limites, precisão e sufixos. |
| OverlayBadge | `crates/ravenrs-widgets-kit/src/status.rs:112` | Natural | Credenciais: sobrepor estado da conexão ao avatar da conta. | Sim. |
| PageHeader | `crates/ravenrs-widgets-base/src/stack.rs:305` | Natural | Lista de fluxos: título, contagem e ação de criar. | Sim. |
| PagedDocumentView | `crates/ravenrs-widgets-lists/src/lists/paged.rs:43` | Natural | Relatórios: consultar páginas de PDF e slides rasterizados pelos motores. | Sim para páginas como imagens; não oferece seleção de texto. |
| Pagination | `crates/ravenrs-widgets-base/src/choice.rs:234` | Natural | Lista de fluxos em cartões: trocar páginas de resultados. | Sim. |
| Playground | `crates/ravenrs-widgets-chat/src/playground.rs:161` | Natural | Expressões: executar transformações locais sobre amostras e conferir resultado. | Sim com `Runner` que retorna componentes existentes; não é terminal de CLI. |
| Popover | `crates/ravenrs-widgets-base/src/overlay.rs:251` | Natural | Editor: opções rápidas de execução junto ao botão correspondente. | Sim. |
| ProgressBar | `crates/ravenrs-widgets-base/src/display.rs:317` | Natural | IA local e vídeo: mostrar download ou renderização com progresso conhecido. | Sim. |
| PropertyRow | `crates/ravenrs-widgets-base/src/stack.rs:225` | Natural | Detalhe da execução: identificador, início, duração e versão do fluxo. | Sim. |
| Protractor | `crates/ravenrs-widgets-imaging/src/imaging/transform.rs:1010` | Não cabe | Endireitar imagens por ângulo não integra o fluxo de automação proposto. | Coberto em `examples/component-gallery`. |
| QuadField | `crates/ravenrs-widgets-kit/src/quad_field.rs:35` | Forçado | Um campo procedural de quadriláteros acrescentaria um efeito sem função operacional. | — |
| RadioGroup | `crates/ravenrs-widgets-base/src/inputs.rs:287` | Natural | Painel do nó: escolher política de falha entre interromper, continuar e repetir. | Sim. |
| Rating | `crates/ravenrs-widgets-kit/src/status.rs:197` | Forçado | Pontuar fluxos com estrelas exigiria uma função de avaliação sem necessidade definida. | — |
| RecurrenceEditor | `crates/ravenrs-widgets-scheduler/src/event_calendar.rs:1006` | Natural | Agenda: configurar a repetição de um gatilho. | Parcial: faltam recorrências subdiárias e configuração temporal com fuso e exceções. |
| ReleaseNotes | `crates/ravenrs-widgets-kit/src/release_notes.rs:253` | Natural | Configurações: consultar mudanças e instruções de atualização. | Sim. |
| Ribbon | `crates/ravenrs-widgets-dock/src/ribbon.rs:214` | Natural | Relatórios: comandos de edição e saída dentro de `Designer`. | Sim; uso interno do designer. |
| RichText | `crates/ravenrs-widgets-chat/src/rich_text.rs:234` | Natural | Ajuda e agentes: apresentar instruções e respostas formatadas com links. | Sim para exibição. |
| RubberBand | `crates/ravenrs-widgets-imaging/src/imaging/overlays.rs:505` | Forçado | Seleção retangular adicional duplicaria a interação já oferecida por `FlowCanvas`. | — |
| RuleOfThirdsGrid | `crates/ravenrs-widgets-imaging/src/imaging/overlays.rs:410` | Não cabe | Guias de composição fotográfica não são necessárias ao editor de automação. | Coberto em `examples/component-gallery`. |
| Ruler | `crates/ravenrs-widgets-imaging/src/imaging.rs:415` | Natural | Relatórios: medir posicionamento na superfície de `Designer`. | Sim; uso interno do designer. |
| SankeyChart | `crates/ravenrs-widgets-charts/src/sankey.rs:160` | Natural | Execuções: quantificar itens encaminhados entre ramos e tentativas. | Sim para grafo acíclico; tentativas precisam de identidades distintas nos dados. |
| ScanField | `crates/ravenrs-widgets-kit/src/counter.rs:110` | Não cabe | Captura contínua de leitor físico não é um gatilho desktop necessário neste domínio. | Coberto em `examples/component-gallery`. |
| Scheduler | `crates/ravenrs-widgets-scheduler/src/scheduler/view.rs:207` | Natural | Agenda: visualizar janelas de execução por máquina ou fila. | Sim; recursos, eventos, restrições e alterações são suportados. |
| ScrollEdgeArrow | `crates/ravenrs-widgets-base/src/stack.rs:574` | Natural | Modelos: percorrer uma faixa de categorias sem ocultar a possibilidade de rolagem. | Sim. |
| ScrollLinked | `crates/ravenrs-widgets-base/src/scroll_scene.rs:137` | Forçado | Efeitos vinculados à rolagem não acrescentam informação às telas operacionais. | — |
| ScrollScene | `crates/ravenrs-widgets-base/src/scroll_scene.rs:87` | Forçado | Uma narrativa presa à rolagem duplicaria o treinamento já servido por filmes. | — |
| ScrollView | `crates/ravenrs-widgets-base/src/stack.rs:463` | Natural | Painel do nó: percorrer parâmetros que excedem a altura disponível. | Sim. |
| Section | `crates/ravenrs-widgets-base/src/display.rs:787` | Natural | Configurações: agrupar execução, armazenamento e integrações. | Sim. |
| SegmentedControl | `crates/ravenrs-widgets-base/src/inputs.rs:861` | Natural | Lista de fluxos: alternar cartões e tabela. | Sim. |
| Select | `crates/ravenrs-widgets-base/src/overlay.rs:1005` | Natural | Painel do nó: escolher método de requisição ou formato de saída. | Sim. |
| SelectionBar | `crates/ravenrs-widgets-chat/src/conversation.rs:260` | Natural | Lista de fluxos: indicar seleção e oferecer exportação ou exclusão em lote. | Sim. |
| Sheet | `crates/ravenrs-widgets-base/src/floating.rs:133` | Natural | Painel do nó: apresentar parâmetros sobre o editor em janela estreita. | Sim. |
| ShimmerText | `crates/ravenrs-widgets-kit/src/status.rs:793` | Forçado | Brilho animado em mensagens de espera duplica os indicadores de atividade. | — |
| ShortcutHintBar | `crates/ravenrs-widgets-kit/src/shortcuts.rs:183` | Natural | Editor: lembrar atalhos pertinentes à seleção atual. | Sim; apresenta dicas, sem registrar comandos. |
| ShortcutSheet | `crates/ravenrs-widgets-kit/src/shortcuts.rs:41` | Natural | Editor: consultar atalhos agrupados por tarefa. | Sim. |
| Sidebar | `crates/ravenrs-widgets-kit/src/nav.rs:489` | Natural | Estrutura: navegar entre fluxos, execuções, credenciais e configurações. | Sim. |
| SidebarBrand | `crates/ravenrs-widgets-kit/src/nav.rs:923` | Natural | Estrutura: identificar aplicação e ambiente na navegação. | Sim. |
| Skeleton | `crates/ravenrs-widgets-base/src/display.rs:465` | Natural | Lista de fluxos: reservar espaço durante carregamento inicial. | Sim. |
| Slider | `crates/ravenrs-widgets-base/src/inputs.rs:406` | Natural | Treinamentos: posição de reprodução; vídeo: volume da narração. | Sim. |
| SortableList | `crates/ravenrs-widgets-lists/src/lists/sortable.rs:39` | Natural | Painel do nó: reordenar regras de roteamento avaliadas em sequência. | Sim. |
| Spacer | `crates/ravenrs-widgets-base/src/stack.rs:430` | Natural | Estrutura: distribuir título e ações nas barras. | Sim. |
| Spinner | `crates/ravenrs-widgets-base/src/extras.rs:19` | Natural | Credenciais e IA local: teste de conexão ou carregamento sem percentual. | Sim. |
| SplashScreen | `crates/ravenrs-widgets-base/src/floating.rs:327` | Natural | Inicialização: mostrar abertura do armazenamento e preparação dos serviços. | Sim; permite atraso antes de aparecer. |
| SplitButton | `crates/ravenrs-widgets-kit/src/commands.rs:700` | Natural | Editor: executar normalmente ou escolher execução com dados de teste. | Sim. |
| SplitHandle | `crates/ravenrs-widgets-base/src/extras.rs:306` | Natural | Detalhe da execução: ajustar divisões criadas por `SplitPanes`. | Sim; uso interno de `SplitPanes`. |
| SplitPanes | `crates/ravenrs-widgets-base/src/extras.rs:519` | Natural | Detalhe da execução: dimensionar entrada, saída e registros separadamente. | Sim, com tamanhos vinculáveis. |
| Splitter | `crates/ravenrs-widgets-base/src/extras.rs:682` | Natural | Expressões: dividir editor e resultado da amostra. | Sim para divisão local de dois painéis. |
| Stat | `crates/ravenrs-widgets-base/src/display.rs:720` | Natural | Início e Execuções: taxa de sucesso, total e duração média. | Sim. |
| StatusBar | `crates/ravenrs-widgets-base/src/stack.rs:1205` | Natural | Editor: salvamento, ambiente e quantidade de nós selecionados. | Sim. |
| StepBadge | `crates/ravenrs-widgets-kit/src/wizard.rs:186` | Natural | Modelos: número e estado de cada etapa de configuração. | Sim. |
| StepIndicator | `crates/ravenrs-widgets-kit/src/wizard.rs:337` | Natural | Modelos: acompanhar e revisar etapas de configuração. | Sim. |
| Stepper | `crates/ravenrs-widgets-base/src/inputs.rs:734` | Natural | Painel do nó: quantidade pequena de tentativas. | Sim. |
| Sticky | `crates/ravenrs-widgets-base/src/scroll_scene.rs:35` | Natural | Ajuda: manter comandos do roteiro visíveis durante a leitura. | Sim. |
| SwatchGrid | `crates/ravenrs-widgets-base/src/choice.rs:596` | Natural | Configurações: paleta de cores para categorias de fluxos. | Sim. |
| Table | `crates/ravenrs-widgets-kit/src/data.rs:255` | Natural | Detalhe da execução: resumo curto e estático de consumo por etapa. | Sim; consultas extensas ficam com `DataGrid`. |
| Tabs | `crates/ravenrs-widgets-kit/src/nav.rs:52` | Natural | Painel do nó: parâmetros, entrada e saída. | Sim. |
| TagBox | `crates/ravenrs-widgets-kit/src/tag_box.rs:37` | Natural | Lista de fluxos: atribuir etiquetas existentes ou novas. | Sim. |
| Text | `crates/ravenrs-widgets-base/src/text.rs:19` | Natural | Estrutura: títulos, descrições e valores com tipografia do tema. | Sim. |
| TextArea | `crates/ravenrs-widgets-kit/src/editors.rs:336` | Natural | Painel do nó: descrições, instruções e texto de entrada multilinha. | Sim. |
| TextField | `crates/ravenrs-widgets-base/src/text_field.rs:28` | Natural | Lista de fluxos: busca; Credenciais: segredo com apresentação segura. | Sim; apresentação segura não substitui armazenamento protegido. |
| ThemeEditor | `crates/ravenrs-widgets-kit/src/theme.rs:112` | Natural | Configurações e tema: editar tokens e conferir legibilidade. | Sim; importação e exportação de diferenças já existem. |
| ThemePicker | `crates/ravenrs-widgets-kit/src/theme.rs:26` | Natural | Estrutura: trocar tema por controle compacto. | Sim. |
| ThemeSwitcher | `crates/ravenrs-widgets-base/src/inputs.rs:983` | Natural | Configurações e tema: comparar e escolher entre três temas. | Sim. |
| Ticker | `crates/ravenrs-widgets-base/src/page_motion.rs:21` | Forçado | Uma faixa em movimento prejudicaria a leitura de estados e registros. | — |
| TileLayout | `crates/ravenrs-widgets-kit/src/tiles.rs:137` | Natural | Início: mover e redimensionar resumos operacionais. | Sim, inclusive como base de `DashboardView`. |
| TimePicker | `crates/ravenrs-widgets-kit/src/editors.rs:1782` | Natural | Agenda: escolher o horário diário do gatilho. | Sim para horário; recorrência e fuso são tratados no gatilho. |
| Timeline | `crates/ravenrs-widgets-scheduler/src/timeline.rs:177` | Natural | Detalhe da execução: duração das etapas; vídeo: trilhas de `MovieEditor`. | Sim. |
| TitleBar | `crates/ravenrs-widgets-base/src/stack.rs:1032` | Natural | Estrutura desktop: título do fluxo e controles da janela. | Sim; ações são conectadas ao host. |
| Toaster | `crates/ravenrs-widgets-base/src/floating.rs:747` | Natural | Estrutura: confirmar salvamento, importação e cópia. | Sim. |
| Toggle | `crates/ravenrs-widgets-base/src/inputs.rs:55` | Natural | Lista de fluxos: ativar ou suspender um fluxo. | Sim. |
| ToggleButton | `crates/ravenrs-widgets-base/src/choice.rs:17` | Natural | Painel do nó: fixar uma amostra de entrada para testes. | Sim. |
| ToggleGroup | `crates/ravenrs-widgets-base/src/choice.rs:138` | Natural | Execuções: selecionar múltiplos estados visíveis no histórico. | Sim. |
| ToolRail | `crates/ravenrs-widgets-base/src/stack.rs:1401` | Natural | Editor: escolher ferramentas de seleção, conexão e inclusão. | Sim. |
| Toolbar | `crates/ravenrs-widgets-kit/src/commands.rs:250` | Natural | Editor: comandos principais com atalhos e menu de excedentes. | Sim. |
| Tooltip | `crates/ravenrs-widgets-base/src/overlay.rs:53` | Natural | Editor: explicar botões de ícone e seus atalhos. | Sim. |
| TransformHandles | `crates/ravenrs-widgets-imaging/src/imaging/transform.rs:592` | Não cabe | Transformação livre de camadas gráficas está fora do escopo; os editores usados já controlam sua geometria. | Coberto em `examples/component-gallery`. |
| TreeView | `crates/ravenrs-widgets-lists/src/tree.rs:169` | Natural | Expressões: explorar campos e caminhos da entrada estruturada. | Sim; a aplicação converte o esquema em nós. |
| ValueSlider | `crates/ravenrs-widgets-kit/src/quick_edit.rs:39` | Natural | Nós de vídeo: ajustar velocidade por arraste e informar valor exato. | Sim. |
| VirtualList | `crates/ravenrs-widgets-lists/src/lists/virtual_list.rs:230` | Natural | Detalhe da execução: muitos registros com acompanhamento do final. | Sim; oferece `follow_end` e comandos de rolagem. |
| Wizard | `crates/ravenrs-widgets-kit/src/wizard.rs:73` | Natural | Modelos: configurar credenciais, entradas e revisão antes de criar o fluxo. | Sim. |
| YearView | `crates/ravenrs-widgets-lists/src/lists/year_view.rs:76` | Natural | Agenda: localizar dias com agendamento e abrir o mês correspondente. | Sim; marcações são calculadas pela aplicação. |
| ZStack | `crates/ravenrs-widgets-base/src/stack.rs:134` | Natural | Prévia de vídeo: sobrepor comandos existentes ao quadro mostrado. | Sim. |
| ZoomControl | `crates/ravenrs-widgets-base/src/zoom.rs:184` | Natural | Inspetor de artefatos: controlar o zoom compartilhado da imagem. | Sim. |
| ZoomPanView | `crates/ravenrs-widgets-imaging/src/imaging.rs:1574` | Natural | Inspetor de artefatos: examinar quadros e imagens maiores que a janela. | Sim usando a modalidade de imagem. |

#### Widgets públicos fora do catálogo

Entram nesta contagem os tipos públicos de interface acessíveis pela API dos crates inspecionados. `Surface` e `Properties`, do designer, têm declaração `pub`, mas pertencem a módulos privados sem reexportação; são detalhes internos, não componentes disponíveis para o exemplo.

| Componente | Definição (arquivo:linha) | Decisão | Tela e uso | Atende? / lacuna exposta |
|---|---|---|---|---|
| ActionButton | `crates/ravenrs-app-ui/src/container.rs:221` | Natural | Cadastros: apresentar isoladamente uma ação registrada, respeitando sua disponibilidade. | Sim. |
| ActionContainer | `crates/ravenrs-app-ui/src/container.rs:77` | Natural | Credenciais e Variáveis: reunir ações das categorias do modelo. | Sim; inclui atalhos e tratamento de excedentes. |
| AppShell | `crates/ravenrs-app-ui/src/shell.rs:209` | Natural | Estrutura: navegação e telas de cadastro geradas pelo modelo. | Sim; telas especializadas são compostas com widgets existentes. |
| BugReporter | `crates/ravenrs-movie/src/flight.rs:410` | Natural | Diagnóstico: revisar descrição, detalhes e filme antes de relatar um problema. | Sim; permite excluir, editar e marcar o filme. |
| Composition | `crates/ravenrs-movie/src/composition.rs:136` | Natural | Nós de vídeo: conferir um instante da composição editada. | Sim; compartilha a composição usada por player e editor. |
| DashboardView | `crates/ravenrs-app-ui/src/dashboard.rs:19` | Natural | Início: reunir indicadores e listas declarados no modelo da aplicação. | Sim. |
| Designer | `crates/ravenrs-report-designer/src/lib.rs:44` | Natural | Relatórios: editar o modelo usado pelo nó de geração de relatório. | Sim; `embedded` recebe relatório e callbacks de salvar e fechar. |
| DetailView | `crates/ravenrs-app-ui/src/detail_view.rs:22` | Natural | Credenciais: formulário da conta conectada a partir dos metadados. | Sim com configuração adequada dos editores dos campos. |
| DocumentViewer | `crates/ravenrs-report-viewer/src/lib.rs:321` | Natural | Relatórios: visualizar e pesquisar o documento renderizado pelo motor de relatórios. | Sim; não é um leitor genérico de arquivos PDF. |
| EntityForm | `crates/ravenrs-app-ui/src/form.rs:60` | Natural | Credenciais: criar ou editar um cadastro em janela de formulário. | Sim; valida e entrega o registro ao responsável pela gravação. |
| EntityPane | `crates/ravenrs-app-ui/src/pane.rs:97` | Natural | Variáveis: lista e formulário integrados para variáveis tipadas. | Sim; recebe persistência, versão e callbacks. |
| FlightRecorder | `crates/ravenrs-movie/src/flight.rs:155` | Natural | Diagnóstico: manter trecho recente da interação para revisão no relato de problema. | Sim; gravação pode ser pausada. |
| ListView | `crates/ravenrs-app-ui/src/list_view.rs:88` | Natural | Credenciais: consulta com pesquisa, filtros, ordenação e abertura do cadastro. | Sim. |
| ModelEditor | `crates/ravenrs-app-ui/src/model_editor.rs:29` | Forçado | Edita metadados da aplicação; não é editor de fluxo nem seletor de modelo de IA. | — |
| MovieAssistant | `crates/ravenrs-movie/src/assistant.rs:78` | Natural | Treinamentos: localizar uma explicação e assistir ao trecho correspondente com narração. | Sim; recebe biblioteca, rótulos, narração e navegação por cue. |
| MovieChat | `crates/ravenrs-movie/src/chat.rs:78` | Natural | Treinamentos: fazer perguntas e abrir momentos pertinentes da biblioteca. | Sim; oferece integração opcional com gerador de respostas. |
| MovieEditor | `crates/ravenrs-movie/src/editor.rs:849` | Natural | Treinamentos e vídeo: editar cortes, anotações, capítulos e trilhas do filme. | Sim para filmes do motor; não é editor genérico de vídeo raster. |
| MovieFrame | `crates/ravenrs-movie/src/player.rs:450` | Natural | Nós de vídeo: conferir o quadro com as marcações e explicações que aparecerão na saída. | Sim. |
| MoviePlayer | `crates/ravenrs-movie/src/player.rs:676` | Natural | Treinamentos: reproduzir filme vetorial narrado e seguir para a próxima lição. | Sim; narração, conclusão e próxima lição já têm API. |
| PageFan | `crates/ravenrs-report-viewer/src/fan.rs:37` | Forçado | Espalhar páginas em leque seria apresentação decorativa para uma seleção operacional de relatórios. | — |
| ParameterPanel | `crates/ravenrs-report-viewer/src/parameters.rs:51` | Natural | Relatórios: informar parâmetros tipados antes de gerar o documento. | Sim. |
| SocialFrame | `crates/ravenrs-movie/src/social.rs:23` | Natural | Nós de vídeo: conferir saída vertical ou quadrada com título e explicação legíveis. | Sim para a composição de quadro proposta pelo componente. |
| Spreadsheet | `crates/ravenrs-spreadsheet/src/lib.rs:129` | Natural | Planilhas: preparar entradas tabulares, editar fórmulas e inspecionar resultados. | Sim; possui estado compartilhado e modo somente leitura. |

Os demais crates solicitados foram inspecionados, mas não acrescentam widgets públicos à contagem:

| Crate | O que oferece | Uso real possível |
|---|---|---|
| `ravenrs-pdf` | Motor sem interface de leitura, organização e rasterização de PDF; descrição em `crates/ravenrs-pdf/src/lib.rs:1`. | Nós de extração, combinação e separação; páginas exibidas por `PagedDocumentView`. |
| `ravenrs-word` | Modelo e importação/exportação de documentos, sem layout de páginas ou interface; descrição em `crates/ravenrs-word/src/lib.rs:1`. | Nós de geração de documentos e preenchimento de modelos; inspeção textual com componentes existentes. |
| `ravenrs-deck` | Modelo, comandos, animação, renderização e exportação de apresentações; descrição em `crates/ravenrs-deck/src/lib.rs:1`. | Nós que geram apresentações ou quadros; prévias com `Image`, `Filmstrip` e `PagedDocumentView`. |
| `ravenrs-project` | Geração de projeto Cargo a partir de código de playground; `Project` em `crates/ravenrs-project/src/lib.rs:49`. | Exportar um experimento de playground quando ele representar um projeto executável; não representa o projeto de automação. |
| `ravenrs-widgets-scheduler` | Os quatro widgets públicos de interface encontrados já pertencem ao catálogo. | `EventCalendar`, `RecurrenceEditor`, `Scheduler` e `Timeline`, contabilizados acima. |

`App` e `SecuredApp`, de `ravenrs-app-ui`, são construtores de interface, e não widgets adicionais. Estados, modelos, parâmetros, recursos e estruturas de documento também não aumentam a contagem.

### Lacunas expostas pelos usos

Tamanhos estimados: **P** para ajuste localizado de API e estado; **M** para comportamento novo com interação e validação; **G** para mudança coordenada de modelo, semântica e interface.

- **`ravenrs-widgets-chat`**
  - **M — `CodeEditor`:** acrescentar JSON e shell a `CodeLanguage`, com seus realçadores. A edição, o cursor e os diagnósticos devem continuar no próprio componente.
  - **M — `Chat`:** oferecer ação de interrupção enquanto estiver ocupado, com callback e estado acessível. O processo de agente é interrompido pela aplicação; o comando visual pertence ao chat.

- **`ravenrs-widgets-kit`**
  - **M — `ExpressionEditor`:** aceitar um contexto de amostra e um avaliador, apresentando resultado, tipo ou erro de avaliação. Deve preservar a linguagem de critérios existente; usar outra linguagem exigiria suporte explícito, não apenas trocar os exemplos exibidos.

- **`ravenrs-widgets-imaging`**
  - **M — `CurveEditor`:** acrescentar domínio e formatação dos eixos, unidades e restrição opcional de Y crescente. O componente já interpola os pontos em `[0, 1]`; isso não impede que a sequência de pontos escolhida pelo usuário diminua.
  - **M — `Histogram`:** suportar domínio das faixas, unidades e identificação de cada intervalo para uso com latências. As contagens já podem ser fornecidas; os indicadores de acúmulo nos extremos não substituem a leitura das faixas.

- **`ravenrs-widgets-base`**
  - **P — `InputOtp`:** acrescentar estado desabilitado para o período de verificação, bloqueando edição e nova conclusão.
  - **M — modelo de recorrência:** ampliar `RecurrenceRule`, hoje baseado em dias, semanas, meses e anos, para representar recorrências subdiárias e as informações temporais exigidas pelo gatilho.

- **`ravenrs-widgets-scheduler`**
  - **G — `RecurrenceEditor`, em conjunto com seu modelo:** editar recorrências em minutos e horas, fuso e exceções, com explicação da regra e prévia das próximas ocorrências. A semântica precisa definir transições de horário e horários inexistentes ou repetidos. O editor deve incorporar essa capacidade, sem um componente paralelo criado no exemplo.

As demais integrações descritas não justificam, pela leitura da API, uma alteração visual antecipada. Renderizar documentos, calcular ocorrências, executar processos, guardar segredos e alimentar bindings são trabalhos de integração, não lacunas automaticamente atribuíveis aos widgets.

### Fora do exemplo

- **Tratamento e composição fotográfica:** `ColorWheel`, `CropOverlay`, `Eyedropper`, `Protractor` e `RuleOfThirdsGrid`. Todos têm uso explícito em `examples/component-gallery`, nos arquivos `src/samples/color.rs` e `src/samples/imaging.rs`. O exemplo de automação não precisa assumir funções de tratamento fotográfico para cobri-los.
- **Transformação gráfica livre:** `TransformHandles`. Já aparece em `examples/component-gallery/src/samples/imaging.rs`. O produto usa os controles geométricos dos próprios editores de fluxo, filme e relatório.
- **Leitura física contínua:** `ScanField`. Já aparece em `examples/component-gallery/src/samples/navigation.rs`. Um campo que retém foco para receber leituras de scanner não é necessário às telas propostas.

Os 14 componentes do catálogo classificados como **Forçado**, além de `ModelEditor` e `PageFan` fora dele, também ficam fora da implementação: seus motivos estão registrados individualmente na matriz.

## Telas

**Ainda não há tela desenhada. O desenho passa pelo Claude Design**, depois da definição dos contratos e estados que os componentes precisam representar.

| Tela | Componentes RavenRS | Lacunas que bloqueiam o recorte completo |
|---|---|---|
| Lista de fluxos | `DataGrid`, `TreeView`, formulários por entidade, `CommandPalette`, `EmptyState` | L10 para documento válido; L27 para acesso por projeto |
| Editor | `DockArea`, `FlowCanvas`, `TreeView`, toolbar, menus e notificações | L01–L03, L10; L04 é complemento de exportação e desenho |
| Painel do nó | `DockArea`, formulários, `DataGrid`, `ExpressionEditor`, `CodeEditor` | L05–L10, L15 e L26 |
| Execuções | `DataGrid`, filtros, ações, notificações e estados vazios | L12, L13 e L27 |
| Detalhe da execução | `FlowCanvas`, `DockArea`, inspector, grade e editor somente leitura | L02, L06, L08 e L12 |
| Credenciais | Formulários por tipo, listas, diálogos e notificações | L05, L16, L23, L24 e L27 |
| Variáveis | `DataGrid`, formulário e validação | L09/L10 para resolução; L23 para referências secretas |
| Configurações | Formulários, seleção de tema, ajuda de atalhos e listas administrativas | L12–L14 e L27; a composição básica já é possível |

Treinamentos, nós de vídeo, nós de agente, relatórios, planilhas, IA local e início entram nas mesmas telas ou em painéis próprios; a lista completa, com os componentes de cada uma, está em [Por tela](#por-tela).

Agenda, submissão de formulário e chat podem aparecer como painéis de configuração e teste do editor; não precisam virar telas independentes no primeiro recorte.

## Ordem de construção

Os caminhos de galeria marcados como **propostos** são entregas a criar. Nenhum teste desta tabela foi executado durante o levantamento.

| Fase | Biblioteca fechada antes | Entrega do exemplo | Como se prova |
|---|---|---|---|
| F0 — Contratos e desenho | Especificar L03, L05, L09, L10 e estados de L12; consolidar schema/versionamento | Modelo de dados, fixtures e desenho das oito telas, ainda sem execução real | Fixtures de fluxo válido/inválido e critérios de aceitação; revisar a composição existente em `component-gallery → Charts → FlowCanvas` e `Layout → DockArea` |
| F1 — Documento editável | L01 e L03; parte de L02 para leitura firme | Criar, conectar, mover, anotar, copiar/colar documento completo e salvar/reabrir | Testes sem janela de JSON e remapeamento; testes de comportamento em `crates/ravenrs-widgets/tests/suite/flow.rs`; ampliar a demonstração existente de `FlowCanvas` |
| F2 — Parâmetros e dados | L05–L09 | Painel de nó com parâmetros, entrada/saída e expressões com prévia; fixação de amostras | Schema inválido, opções dependentes fora de ordem, cancelamento de coleção, caminhos escapados e arraste por teclado; galeria proposta `Forms → ParameterForm` e `Data → DataInspector` |
| F3 — Execução determinística local | L10, L11 e núcleo de L13 | Gatilho manual, transformações, ramos, merge, lotes, execução de nó e subfluxo | Testes sem janela com relógio e providers falsos; provar ordem, item/proveniência, ramos vazios, junção, erro e cancelamento; galeria proposta `Charts → FlowCanvas → Execution states` para a apresentação |
| F4 — Histórico durável | L12, restante de L02 e L13, fronteiras atômicas necessárias de L27 | Lista/detalhe, wait, reinício, replay, exclusão e retenção | Encerrar e reabrir armazenamento em cada checkpoint; disputar trabalho com dois workers; expirar lease; comprovar ausência de duplicação nos efeitos reconciliáveis; galeria proposta `Data → Execution inspector` |
| F5 — Credenciais e conectores básicos | L16–L20 e L23 | HTTP, SQLite, Postgres, e-mail de saída, arquivos e planilha com teste de conexão | Servidores/adaptadores de teste, prazo e cancelamento; verificar redação, tipos e referências de segredo; formulário e inspector já disponíveis na galeria |
| F6 — Gatilhos ativos e API | L14, L21, parte aplicável de L22, L24 e L27 | Ativar/desativar, cron/intervalo, polling, eventos de arquivo, webhook com resposta e OAuth2 | Relógio controlado, horário ambíguo, atraso, cursor, evento repetido, timeout HTTP e renovação concorrente; galeria proposta `Forms → Trigger configuration` para configuração visual |
| F7 — Código, formulário e chat | L15, L26 e restante de L22 | Nó de código, formulário de entrada e teste conversacional | Script infinito termina pelo limite; erro aponta linha; submissão valida revisão e identidade; chat cancela e correlaciona resposta; galeria proposta `Editors → Script` e ampliação de `Chat` |
| F8 — IA | L25 | Modelo de linguagem, agente com poucas ferramentas e memória de sessão | Modelo/ferramenta falsos para limites, erro e continuidade; integração real opcional e separada; galeria proposta `Chat → Tool execution` |
| F9 — Consolidação | Restante de L27; L04 se incluída no recorte | Gestão por projeto, auditoria, métricas, acessibilidade, i18n e exportação visual opcional | Testes sem janela de permissão em UI/API/worker, árvore acessível, foco e traduções; percorrer todas as composições da galeria nos dois temas; validar desktop manualmente depois |
| F10 — Vídeo e treinamento | Lacunas de vídeo da área 13, com o Kokoro como narração de tudo, inclusive dos nós | Treinamentos embutidos (uso A) já desde as primeiras fases; nós de vídeo (uso B) depois de F3 a F5 | Fases 1 a 5 da área 13: roteiro sem janela, quadros conferidos, fala transcrita por reconhecedor independente |
| F11 — Nós de agente | Lacunas da área 14 (`ravenrs-process`, `ravenrs-agent-cli`, validação por esquema, `ravenrs-git`, console) | Nós Codex, Claude, revisão de código e aplicar diff com aprovação | CLI falsa no teste, com JSONL gravado, demora, falha e cancelamento; nenhuma assinatura gasta |

Para cada peça visual nova, o aceite inclui:

1. Teste de comportamento do componente, incluindo teclado, erro, vazio e cancelamento.
2. Demonstração na galeria consumindo somente API pública.
3. Uso pelo exemplo sem acesso a detalhes internos.
4. Verificação visual posterior ao desenho, além dos testes sem janela.

Para infraestrutura sem UI, a galeria demonstra apenas sua projeção visual; a prova principal é o teste de contrato sem janela.

## Fora do escopo

| Recorte excluído | Motivo |
|---|---|
| Centenas de integrações | Poucos conectores representativos bastam para validar schema, credenciais, erros, binários e cancelamento |
| Nuvem multi-inquilino, cobrança e administração de frota | O objetivo é desktop local primeiro; esses sistemas ampliam substancialmente operação e isolamento |
| Colaboração simultânea no canvas | Exigiria protocolo de documento concorrente e presença; versões locais e projetos já exercitam o núcleo necessário |
| Compatibilidade integral de formato ou expressão com outra plataforma | O exemplo deve declarar e versionar seu próprio contrato |
| Marketplace e execução de extensões arbitrárias | Exigem distribuição, confiança, atualização e isolamento além do catálogo demonstrativo |
| Editor de código completo | O recorte é editar e diagnosticar scripts de nós |
| Garantia universal de efeito externo exatamente uma vez | Depende de idempotência ou reconciliação oferecida pelo destino; resultados desconhecidos devem permanecer explícitos |
| Reproduzir todos os controles de uma planilha | O nó usa o motor sem janela; uma superfície completa não é requisito da integração |
| Alta disponibilidade distribuída | Fila durável e recuperação local são necessárias; operação de cluster não é requisito inicial |
| Editor gráfico de uso geral | Dobra manual e exportação são extensões úteis, mas não bloqueiam a primeira automação executável |
| Paridade desktop/WebAssembly do executor | Este exemplo é desktop. APIs devem declarar capacidades por alvo; threads, arquivos, sockets e cofre precisam de tratamento específico no navegador |

## Como este levantamento foi feito

| Item | Registro |
|---|---|
| Data | **15/11/2026** |
| Commit base | **`2908a1c76`**, obtido com `git rev-parse --short HEAD` |
| Atualização | Revisto contra o `master` em `d2c0ee0ce`, depois dos PRs #511 (FlowCanvas: `FlowRun`, `FlowToolbox`, `FlowInspector`, `FlowLibrary`, exportação) e #512 (Raven Code: terminal, `CodeEditor` com sugestões). 61 números de linha traduzidos pelo diff de cada arquivo, três reescritos à mão; as áreas 1, 2, 4 e 9 mudaram de estado e a L04 saiu da lista, já entregue |
| Correção | A recontagem do resumo da revisão anterior trocou, por engano, as 15 linhas da tabela da área 14 (que também começam com `| N. `) pelos números do resumo. Elas foram recuperadas do primeiro commit do levantamento e traduzidas para o `master` atual pelo mesmo diff; a recontagem agora só mexe na seção "Estado em resumo". A #525 (`ravenrs-flows`) mudou o estado de sete linhas das áreas 1, 2 e 4 |
| Ferramenta | **Codex CLI** (`codex exec -s read-only`), em quatro rodadas: áreas 1 a 12, área 13, área 14 e cobertura de componentes |
| Conferência | Claude Code abriu cada `arquivo:linha` citado e conferiu o símbolo na linha (202 definições de componente e 202 referências das áreas). Corrigiu 32 números de linha deslocados de 1 a 18 linhas e refez por amostragem as buscas de ausência; nenhum símbolo citado era inventado |
| Operações | `rg`, listagem de arquivos, leitura de fontes e consultas de estado do Git |
| Alterações | O Codex não criou nem editou arquivo; este README foi montado a partir das quatro respostas |
| Situação do exemplo | Ainda não há código: a pasta guarda só este levantamento e fica fora dos membros do workspace (`exclude` no `Cargo.toml` da raiz) até o código começar |
| Evidência positiva | Tipos e funções públicas abertos e lidos; presença de nome em busca ou documentação não foi suficiente |
| Evidência negativa | Ausência delimitada pelas buscas abaixo; não equivale a prova absoluta sobre qualquer implementação possível |
| Documentos consultados | `PARITY.md`, `COMPONENT-AUDIT.md`, `COMPONENT-INVENTORY.md`, `APP-FRAMEWORK.md`, `APP-PARITY.md`, `APP-COVERAGE.md`, `SOFTWARE-COVERAGE.md` e referências locais de análise de dados |
| Exemplos consultados | Organização e testes de `examples/raven-code`, `examples/dock-workbench` e demonstrações da galeria |
| Divergências documentais | Foram priorizadas as fontes atuais: descrições antigas ainda apresentam como pendentes recursos já existentes no canvas |

### Registro das buscas de ausência

As buscas usaram `rg`, com variantes de caixa quando aplicável. Resultados em outros domínios foram inspecionados antes de concluir ausência no recorte.

| Código | Escopo | Termos procurados | Resultado relevante |
|---|---|---|---|
| B1 | `crates`, `examples`, `docs` | `pinned_data`, `pin_data`, `pindata`, `on_connect_end`, `connection_drop`, `connect_to_empty` | Não encontrados contratos de fixação de dados ou criação ao soltar conexão |
| B2 | Modelo e widget de fluxo | `bend`, `waypoint`, `export` | Referência textual a um possível exportador; não encontrada API de exportação ou dobra manual |
| B3 | `crates`, `examples`, `docs` | `WorkflowEngine`, `workflow_engine`, `ExecutionEngine`, `execution_engine`, `NodeExecutor`, `node_executor`, `topological`, `execution_queue`, `executionqueue`, `subworkflow`, `sub_workflow`, `errorworkflow`, `error_workflow`, `batchloop`, `batch_loop` | Referências topológicas em renderização e recálculo de planilha; não encontrado motor de automação de itens |
| B4 | Workspace e manifesto | `rhai`, `quickjs`, `boa_engine`, `deno_core`, `mlua`; arquivos de script e macros | Não encontrado runtime geral correspondente; scripts/macros de planilha têm escopo próprio |
| B5 | Aplicação, exemplos e documentação | `polling`, `pollingtrigger`, `polling_trigger`, `filewatcher`, `file_watcher`, `notify::`, `formtrigger`, `form_trigger`, `chattrigger`, `chat_trigger` | Não encontrados gatilhos executáveis especializados; formulários e chat fornecem componentes |
| B6 | Aplicação, exemplos e documentação | `workflowversion`, `workflow_version`, `execution_history`, `flowtemplate`, `flow_template` | Não encontrados versionamento, replay ou catálogo de modelos de fluxo executável |
| B7 | Camada de aplicação, IA, servidor de relatórios e manifesto; termos de teste também em `crates/examples/docs` | `oauth`, `oauth2`, `vault`, `secret_store`, `secretstore`, `keyring`, `encrypt`, `aes.gcm`, `chacha`, `connectiontest`, `connection_test`, `credential_type` | Comentário sobre autenticação externa e configuração de transporte; não encontrados cliente OAuth2, cofre ou teste genérico de credencial |
| B8 | `ravenrs-app*`, servidor de relatórios e manifesto | `metrics`, `prometheus`, `opentelemetry`, `retention`, `execution_history`, `cron` | Cron e retenção de relatórios encontrados; não encontrado contrato de métricas de execução de fluxo |
| B9 | `docs`, `crates`, `examples` | `#238`, `#396`, `#397`, `#496`, `#497` e variantes `issues/…`; demais números nos documentos indicados | Referências locais às três primeiras; nenhuma referência encontrada a #496/#497 |

Também foram procurados `fixed_collection`, `fixedcollection`, `parameterform`, `parameter_form`, `jsonviewer`, `json_viewer`, `schema_viewer`, `autocomplete`, `completion`, `drag_source` e `drop_target`. Foram encontradas as primitivas de arraste e os editores descritos; não foi comprovado o painel especializado completo.

### O que não foi verificado

- Compilação, testes, galeria em execução, renderização, desempenho ou consumo de memória.
- Comportamento em janela real, leitores de tela, sistemas operacionais e WebAssembly.
- Conexões reais com bancos, e-mail, HTTP, ferramentas remotas ou modelos de linguagem.
- Correção criptográfica, isolamento de scripts ou garantias de concorrência de uma implementação futura.
- Conteúdo, estado e critérios de aceite das issues remotas, incluindo #496 e #497.
- Desenho das telas, instalação, empacotamento e distribuição do exemplo.
- Ausências fora dos termos e caminhos pesquisados.

As fases e os tamanhos são propostas de construção sustentadas pela leitura; não representam implementação entregue.
