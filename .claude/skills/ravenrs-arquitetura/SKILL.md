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
| Formas vetoriais, layout de gráfico, códigos de barras | `crates/ravenrs-graphics/src/`: `vector`, `chart`, `barcode` | Testes no próprio crate; ver quem usa em `ravenrs-report` |
| Exportar relatório para arquivo | `crates/ravenrs-report-export/src/lib.rs`, que chama os exportadores | Editor e servidor usam só esse ponto de entrada |
| Motor de relatórios | `model.rs`, `engine.rs`, `document.rs` em `crates/ravenrs-report/src/` | `docs/REPORTING.md`, `docs/REPORTING-PARITY.md` |
| Servidor de relatórios | `crates/ravenrs-report-server/src/`: `http`, `scheduler`, `store`, `cron`, `deliver` | `docs/REPORT-SERVER.md`, `crates/ravenrs-report-server/tests/server.rs` |
| Visualizador de relatórios | `crates/ravenrs-report-viewer/src/`: `lib` (`DocumentViewer`), `search`, `parameters` | `crates/ravenrs-report-viewer/tests/viewer.rs` |
| Filmes de treinamento e relato de bug | `crates/ravenrs-movie/src/`: `movie`, `project`, `composition`, `record`, `player`, `editor`, `flight` | Testes em `crates/ravenrs-movie/tests/`; filme é lista de desenho, nunca pixels |
| Editor de relatórios | `examples/report-designer/src/` | `examples/report-designer/tests/designer.rs` |
| Ícones | `scripts/gen-icons.py` regenera a fonte e o `IconName` | Não edite `icon_set.rs` à mão |

Ao entregar ou adiar algo, atualize a tabela de status em §11 do `ARCHITECTURE.md`. Foi a parte que mais ficou defasada.

## Fronteiras a preservar

Estas regras são o que mantém o projeto testável e portátil, por isso vale parar e discutir antes de quebrar qualquer uma:

- **Widgets e renderer só se encontram no `Scene`.** `ravenrs-widgets` nunca toca a GPU e `ravenrs-render` não conhece widgets. Assim os testes de widgets rodam sem GPU e o renderer continua substituível.
- **`ravenrs-criteria` não depende de nada do RavenRS** e não recebe nada específico de um módulo. Filtros, regras de segurança e um futuro equivalente ao XAF vão usar a mesma linguagem.
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
- O modelo DevExpress orienta o comportamento, mas a implementação é própria. Não copie código nem documentação de lá ou de outros projetos.

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

O portão é `scripts/check.sh`: o projeto não tem CI hospedado, então rode antes de publicar. Ele cobre formatação, clippy em desktop e wasm32, testes, imagens de referência, documentação e builds web.

Mudança visual intencional exige regenerar as imagens de referência e revisar o diff dos PNGs:

```bash
RAVENRS_UPDATE_GOLDEN=1 cargo test -p ravenrs --test visual
```

Para ver no navegador, gere com `scripts/build-web.sh <exemplo>` e use as configurações `gallery-web` e `designer-web` de `.claude/launch.json`.
