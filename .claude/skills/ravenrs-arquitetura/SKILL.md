---
name: ravenrs-arquitetura
description: Mapa da arquitetura do RavenRS, framework de GUI em Rust com wgpu para desktop e WebAssembly. Diz onde cada mudança entra, quais fronteiras entre crates preservar e como verificar. Use sempre que for criar ou alterar componentes, BuildCx, renderer, plataformas, tokens, a linguagem de expressões (ravenrs-criteria), o motor de relatórios (ravenrs-report) ou o editor de relatórios, ou ao responder como o RavenRS funciona por dentro, mesmo que o pedido não cite "arquitetura".
---

# Arquitetura do RavenRS

Esta skill é um mapa. O detalhe fica nos documentos do repositório, para existir uma só fonte da verdade. Leia o documento indicado antes de mudar a área.

| Documento | Quando ler |
|---|---|
| `ARCHITECTURE.md` | Grafo de crates (§3), módulos (§3.1), pipeline de frame (§4), reatividade (§5), extensão de componentes (§6), status (§11), decisões (§12) |
| `docs/QUALITY.md` | Definição de pronto, checagens e orçamentos de desempenho |
| `CONTRIBUTING.md` | Passo a passo para adicionar um componente e estilo de código |
| `docs/PARITY.md` | O que existe e o que falta em componentes |
| `docs/REPORTING.md`, `docs/REPORTING-PARITY.md` | Desenho, fases e paridade dos relatórios |
| `docs/design-system/README.md` | Regras visuais e tokens |

Se o código contradisser um documento, confie no código e corrija o documento na mesma mudança.

## Onde cada mudança entra

| Tarefa | Onde mexer | Também atualizar |
|---|---|---|
| Novo componente | Módulo da família em `crates/ravenrs-widgets/src/` (tabela em §3.1), reexportado em `lib.rs` | Testes em `crates/ravenrs-widgets/tests/`, `examples/component-gallery`, `README.md`, `docs/PARITY.md`, auditoria em `docs/QUALITY.md` |
| Serviço para componentes (entrada, overlay, rolagem) | `context.rs` (`BuildCx`) e `ui.rs` | §6.1 do `ARCHITECTURE.md` |
| Animação | Matemática em `ravenrs-animation`; runtime em `ravenrs-widgets/src/motion.rs`; componentes usam `cx.motion` e `theme.motion` | §6.5 do `ARCHITECTURE.md`, `tests/motion.rs` |
| Acessibilidade | `ui/access.rs` e `ravenrs-core/src/access.rs`; pontes em `access.rs` de cada plataforma | Testes de acessibilidade |
| `DataGrid` | `crates/ravenrs-widgets/src/grid/`; lógica pura em `query.rs` | Tabela de paridade em §6.2, `benches/grid_query.rs` |
| Nova primitiva de desenho | `ravenrs-core/src/scene.rs`, `ravenrs-render/src/renderer.rs`, `crates/ravenrs-render/shaders/` | `crates/ravenrs-render/tests/offscreen.rs` |
| Token ou tema | `docs/design-system/tokens.json` e `ravenrs-style`, sempre juntos | `docs/design-system/README.md` |
| Plataforma | `ravenrs-platform-native` e `ravenrs-platform-web`; confira se a outra precisa do mesmo | §10 do `ARCHITECTURE.md` |
| Função ou operador de expressão | `crates/ravenrs-criteria/src/`; funções no catálogo de `functions.rs` | Ida e volta em `format.rs`, `eval.rs`, `validate.rs`, `sql.rs` conforme o caso |
| Formas vetoriais, layout de gráfico, códigos de barras | `crates/ravenrs-graphics/src/`: `vector`, `chart`, `barcode` | Testes no próprio crate; ver quem usa em `ravenrs-report` e em `crates/ravenrs-widgets/src/chart.rs` (`Chart`, `Barcode`, feature `charts`) |
| Modelo da aplicação, sem GUI | `crates/ravenrs-app/src/`: `entity`, `model/`, `space`, `action`, `controller`, `frame`, `module` | Testes no próprio crate, `tests/layers_properties.rs` e `tests/behavior.rs`; plano em `~/.claude/plans/agora-no-est-gio-que-iridescent-cray.md` |
| Atributos de `#[derive(BusinessObject)]` | `crates/ravenrs-app-derive/src/lib.rs`, tipos em `crates/ravenrs-app/src/object.rs` | Erros em `crates/ravenrs-app/tests/ui/` (`TRYBUILD=overwrite` ao mudar, revisando o `.stderr`); `docs/APP-FRAMEWORK.md` |
| Registros em SQLite | `crates/ravenrs-app-sqlite/src/lib.rs`, testes em `tests/space.rs`; `ChangeSet` e `run_query` em `crates/ravenrs-app/src/space.rs` são compartilhados entre os `ObjectSpace` | |
| Permissões, entrada e auditoria | `crates/ravenrs-app/src/security/`; telas em `crates/ravenrs-app-ui/src/security.rs` e `model_editor.rs` | `crates/ravenrs-app/tests/security.rs`, `crates/ravenrs-app-ui/tests/security.rs`, revisão com as skills `securitymaxxing:authz` e `authn` |
| Lista como gráfico, agenda ou árvore | `crates/ravenrs-app-ui/src/list_editors.rs` | `crates/ravenrs-app-ui/tests/list_editors.rs`, `examples/quick-plan` |
| Estados e lembretes | `crates/ravenrs-app/src/workflow.rs`; janela em `crates/ravenrs-app-ui/src/reminders.rs` | `crates/ravenrs-app/tests/workflow.rs`, `crates/ravenrs-app-ui/tests/workflow.rs` |
| Recurso novo do framework | Uma demonstração em `examples/feature-center/src/model.rs` | `examples/feature-center/tests/tour.rs` |
| Registros prontos | `crates/ravenrs-app-library/src/lib.rs`, palavras em `pt_br.json` | `crates/ravenrs-app-library/tests/library.rs` |
| Relatório de uma lista | `crates/ravenrs-app-report/src/lib.rs`, sobre `Session::list_table` e `document_of` | `crates/ravenrs-app-report/tests/report.rs` |
| Telas geradas do modelo | `crates/ravenrs-app-ui/src/`: `shell`, `list_view`, `detail_view`, `nested`, `dashboard`, `form`, `editors`, `container`, `system`, `session` | `crates/ravenrs-app-ui/tests/e2e.rs`, `relations.rs` e `center.rs`, que dirigem a aplicação inteira sem janela; o que faltar de componente vai antes para `ravenrs-widgets` |
| Exportar o que um `DataGrid` mostra | `crates/ravenrs-grid-export/src/lib.rs`, sobre `GridTable` e `ravenrs-report-export` | Widgets não conhecem relatórios; este crate é a única ponte |
| Exportar relatório para arquivo | `crates/ravenrs-report-export/src/lib.rs`, que chama os exportadores | Editor e servidor usam só esse ponto de entrada |
| Motor de relatórios | `model.rs`, `engine.rs`, `document.rs` em `crates/ravenrs-report/src/` | `docs/REPORTING.md`, `docs/REPORTING-PARITY.md` |
| Servidor de relatórios | `crates/ravenrs-report-server/src/`: `http`, `scheduler`, `store`, `cron`, `deliver` | `docs/REPORT-SERVER.md`, `crates/ravenrs-report-server/tests/server.rs` |
| Visualizador de relatórios | `crates/ravenrs-report-viewer/src/`: `lib` (`DocumentViewer`), `search`, `parameters` | `crates/ravenrs-report-viewer/tests/viewer.rs` |
| Filmes de treinamento e relato de bug | `crates/ravenrs-movie/src/`: `movie`, `project`, `composition`, `record`, `player`, `editor`, `flight` | Testes em `crates/ravenrs-movie/tests/`; filme é lista de desenho, nunca pixels |
| Editor de relatórios | `crates/ravenrs-report-designer/src/`; `examples/report-designer` só o executa | `examples/report-designer/tests/designer.rs` |
| Relatórios guardados e painéis no framework | `crates/ravenrs-app-report/src/`: `stored`, `analytics` | `crates/ravenrs-app-report/tests/stored.rs` |
| Desempenho: cenas, gate, tempos | `crates/ravenrs-perf/src/scenes/`, `perf/baseline.json`; `Ui::stats`, `RenderStats` | Skill `ravenrs-desempenho`, `docs/QUALITY.md`, `docs/PERFORMANCE.md` |
| Ferramentas do painel de diagnóstico | `crates/ravenrs-widgets/src/diagnostics.rs`, dados em `crates/ravenrs-core/src/diagnostics.rs` | `crates/ravenrs-widgets/tests/performance_tools.rs`, `docs/PERFORMANCE.md` |
| Playground e interpretador | `crates/ravenrs-live/src/` (`eval`, `bridge`, `registry`); `#[expose]` em `crates/ravenrs-live-derive`; `LiveRunner` e adaptadores em `crates/ravenrs-widgets/src/live.rs` | `scripts/gen-live.py` depois de componente novo; `examples/component-gallery/live-pending.txt`; `crates/ravenrs-live/tests/eval.rs`, `crates/ravenrs-widgets/tests/live.rs` |
| Ícones | `scripts/gen-icons.py` regenera a fonte e o `IconName` | Não edite `icon_set.rs` à mão |

Ao entregar ou adiar algo, atualize a tabela de status em §11 do `ARCHITECTURE.md`. Foi a parte que mais ficou defasada.

## Fronteiras a preservar

Estas regras são o que mantém o projeto testável e portátil, por isso vale parar e discutir antes de quebrar qualquer uma:

- **Widgets e renderer só se encontram no `Scene`.** `ravenrs-widgets` nunca toca a GPU e `ravenrs-render` não conhece widgets. Assim os testes de widgets rodam sem GPU e o renderer continua substituível.
- **`ravenrs-criteria` não depende de nada do RavenRS** e não recebe nada específico de um módulo. Filtros, regras de segurança e um futuro framework de aplicações vão usar a mesma linguagem.
- **Lógica útil a mais de um motor vai para um crate compartilhado, nunca para uma cópia.** `ravenrs-criteria`, `ravenrs-graphics` e `ravenrs-report-export` não conhecem quem os chama. Antes de escrever lógica num motor, procure se outro já tem.
- **`ravenrs-report` não depende de GPU nem de janela**, porque também roda em servidores. Layout gera um `Document` uma vez, e visualizador e exportadores desenham esse mesmo resultado.
- **Tudo que não é cola de plataforma compila para `wasm32-unknown-unknown`:** sem threads, sem `SharedArrayBuffer` e sem bloquear. Fontes são embutidas, porque o navegador não expõe as do sistema.
- **Estado é de uma thread só:** `Rc` e `RefCell`, sem travas. Fontes de estado não mantêm observadores vivos.
- **Nada roda entre frames.** Um app ocioso não consome CPU nem GPU, então frames só são pedidos quando a UI é invalidada.
- **Componentes usam só a API pública** (`View`, `Component`, `BuildCx`), e os de terceiros têm os mesmos poderes.

## A casa segue o padrão

Os módulos próprios, como o editor de relatórios, servem de exemplo para quem usa o framework:

- Use os componentes de `ravenrs-widgets` e os tokens. O que faltar entra antes na biblioteca, e só depois é usado no módulo.
- Evite `Paint::Quad`, `Paint::Custom`, cores literais e medidas soltas. A exceção é conteúdo de documento ou superfície de edição, como o papel do relatório, e mesmo assim as cores vêm de tokens.
- O produto de referência orienta o comportamento, mas a implementação é própria. Não copie código nem documentação de lá ou de outros projetos, e não cite o nome dele em nada que vai para o repositório: descreva o recurso pelo que ele faz.

## Convenções

- Código, rustdoc e documentos do repositório ficam em inglês.
- `unsafe` é proibido no workspace; `unwrap()` e `expect()` só em testes.
- API pública: builders que recebem e devolvem `self`, `Binding<T>` para valores editados, closures para eventos, rustdoc em todo item público.
- `wasm-bindgen` é fixado com `=` e precisa bater com a CLI usada em `scripts/build-web.sh`.

## Como verificar

Durante o trabalho, rode só o que a mudança afeta:

```bash
cargo test -p ravenrs-widgets
```

```bash
cargo clippy -p ravenrs-platform-web --target wasm32-unknown-unknown -- -D warnings
```

O portão é `scripts/check.sh`, porque o projeto não tem CI hospedado. Sem argumento, ele é rápido: formatação, mais clippy e testes dos pacotes que a mudança atinge. Com `--full`, roda antes de publicar e cobre clippy em desktop e wasm32, todos os testes com as imagens de referência, documentação e builds web.

Mudança visual intencional exige regenerar as imagens de referência e revisar o diff dos PNGs:

```bash
RAVENRS_UPDATE_GOLDEN=1 cargo test -p ravenrs --test visual
```

Para ver no navegador, gere com `scripts/build-web.sh <exemplo>` e use as configurações `gallery-web` e `designer-web` de `.claude/launch.json`.
