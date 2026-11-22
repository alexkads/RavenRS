# Feature center

Every feature of the application framework, one small demonstration each, in the navigation by what it shows. Each list says what it is there to show and what to try. It is how to see what the framework does, how to review its screens, and how to know it all still works.

```bash
cargo run -p raven-feature-center
```

```bash
./scripts/build-web.sh raven-feature-center && python3 -m http.server -d tools/raven-feature-center/web 8099
```

On the desktop it keeps its records in SQLite, in `RAVEN_CENTRO` or a file in the temporary folder, so delete that file to start over. In the browser it keeps them in memory and starts over when the page reloads.

## Signing in

These accounts exist only here. The password of all three is `raven-demo`.

| Name | Role | What to notice |
|---|---|---|
| `ana` | Administrators | Everything, with Administration: users, roles, the audit trail and the model editor |
| `bia` | Vendas | Reads every document, sees no cost, and changes only her own |
| `caio` | Leitura | Only reads, does not see ana's documents or the Audit group, and has one action taken away |

## What is here

| Group | Demonstrations |
|---|---|
| Básicos | `Button`: emphasis, sizes, icons, accessible names and descriptions, disabled/loading/selected states, width and keyboard activation. `Text`: the full type scale, custom size/weight/color, alignment, truncation and reactive bound/dynamic content |
| Validação | Rules of every kind, warnings and notes, checking on leaving a field |
| Aparência condicional | Colors, enabled and visible, on fields and actions; words of the screens replaced by key, and keys of the center's own ("Dizer olá") |
| Actions | Simple, with a question, with a value, with a choice, with a window, an address, by criteria, inside a form, messages, one that runs in the background, and "Fechar e dobrar", declared with no code |
| Editores de campo | Every kind of field, and fields that come from the model |
| Listas | Chart, month, timeline and tree; variants and filters; editing on the list; many rows; a panel written in code (Painel em código) |
| Relações | Lists that belong to a record, link and unlink, a filtered reference, cloning |
| Temas | The token editor, changing the whole application at once |
| Kit | `ExpressionEditor` with every feature: fields and parameters offered, the lines shown, and the preview over a sample order, a result or why it failed. `ParameterForm` with every feature: a schema of text, number, yes or no, choice, a field shown in one mode, an expression, a group, a list of headers, and options of the account that arrive, load or fail |
| Listas e árvores | `DataInspector` with every feature: orders as table, tree, JSON and schema, the item and field chosen kept between them, absent told from null, a 19-digit number exact, five thousand rows cut with a notice; Log de build, a `LogView` over a `LogBuffer`: ANSI colors, follows the end, search, stages that fold and a line pointed at by its number |
| Gráficos e fluxos | `FlowCanvas` with every feature (options, rules, events, check, run, export, connections as cables), and `CablePlug` in eight uses (flow, processes, a rack, a switchboard, a synthesizer, a network rack, matching columns, a tip tied to a button) and a lab with every knob. `CABLE_SNAPSHOTS=<folder> cargo test -p raven-feature-center --test cabos -- --ignored` draws each with the GPU into PNGs. `PatchPanel` in three uses: the back of a rack with a front that turns around, a switchboard whose cords go home and work in pairs, and a panel with its own rule |
| Atualização | `NewsDialog` of `ravenrs-app-update` (feature `ui`): the news of an example app with a newer version, the one installed and an older one, opened on the version running, on the newest and as the details of a failed update, with the version button and the notice of the status bar every RavenRS app shows |
| Doca e faixa | `DockArea` in three uses: an editor whose documents stay in the center, with a document that will not close while unsaved, one that closes for good, the arrangement kept as text and restored; a photo desk where anything goes anywhere, floats or hides at an edge; and a fixed page of columns that only the app opens and closes. `Ribbon` over a report: the app menu, quick access, large, small and icon-only commands, toggles, a drop-down, a command turned off, a dialog launcher, the contextual page of a table, the search and the open page |
| Sem armazenamento | A form with no storage behind it |
| Estados e lembretes | A state machine, and a reminder that comes up 30 seconds after opening |
| Segurança | The same documents as each of the three people |
| Auditoria | Each mode side by side, a field left out, changes in collections |
| Acessibilidade | Names and descriptions for screen readers from the model, the focus order of a form, an action that says its keys and announces what it did |
| Servidor | API web: the options of the web API, `ApiOptions`, with the origins a browser may call from and how many threads answer at the same time (`new`, `allow_origin`, `workers`); desktop only |
| Balcão | Dados da loja, a record that is one of a kind and opens on its form, and Caixa de exemplo, a counter window with its notice and function keys inside the center (`cargo run -p raven-feature-center -- caixa` opens it on its own) |
| Registros prontos | Pessoas, Eventos, Países and Endereços from `ravenrs-app-library`, with their rules |
| Relatórios | Reports people design and keep, and panels |
| Administração | Users, roles, the audit trail and the model editor, for `ana`; Trocar senha, for everyone |

## Opening it another way

The same center opens in four modes, with the same parameters on the command line (`--name value`) and in the address of the page (`?name=value`). None of them takes an address: they only choose what is on the screen (#587).

| Parameter | Values | What it does |
|---|---|---|
| `modo` | `completo` (default) | The whole center, after signing in, in SQLite on the desktop and in memory in the browser |
| | `vitrine` | The same, always in memory, with ana filled in at the door and a notice that it starts over: the site's demonstration |
| | `componentes` | Only the pages of the components, with no sign-in and no application framework |
| | `playground` | The examples, to edit and run |
| `pagina` | an id | The page it opens on: a view (`AuditFull_ListView`) or a dashboard of the center, or a page of a component (`CablePlugDemo`). In the playground, the example it opens on |
| `componente` | a name | In the playground, the example of that component on its own (`Button`) |
| `incorporado` | (no value) | In `componentes`, only the page, to sit inside another page |
| `tema` | `escuro`, `claro`, `vidro` | The look |
| `idioma` | `pt-BR`, `en` | The language of the screens |

```bash
cargo run -p raven-feature-center -- --modo vitrine --pagina AuditFull_ListView
```

```bash
cargo run -p raven-feature-center -- --modo componentes --pagina CablePlugDemo --incorporado --tema claro
```

On the web, `index.html?modo=componentes&pagina=CablePlugDemo&incorporado`. A wrong value is refused, saying what it takes; a name it does not know in the address is left alone.

What goes in the build is chosen by Cargo features: `app`, the center, its storage and its web API; `widgets`, the pages of the components; `playground`, the examples. All three are on by default. The pages of the site take less:

```bash
./scripts/build-web.sh raven-feature-center --no-default-features --features widgets,playground
```

## Over a web API

```bash
cargo run -p raven-feature-center --bin api
```

The same center, with the same roles, people and audit trail, answers as a web API on `127.0.0.1:8090`, in memory or in the SQLite file given after `--`. It prints the address of its OpenAPI document. Sign in at `/api/auth/token` with the accounts above. To let a page on another origin call it, set `RAVEN_API_ORIGIN` to that origin.

The center can also be the window of that server: build it with `RAVEN_API_URL=http://127.0.0.1:8090`, and it signs in there and reads every record from there. The address comes from the build and never from the page, so no link can send a password somewhere else. In the tour, `over_a_server_the_counter_window_has_its_control_too` opens the window over a server in memory.

## It is also the test

`tests/suite/tour.rs` goes through all of this, as each of the three people, in memory and in SQLite.

Every component of the library is shown here, with every feature (#579). A component that is new or changes gets its page in the same pull request: the skill `ravenrs-feature-center` says how. `recursos.json` lists each component, the page that shows it and the features it calls. `scripts/feature-center-coverage.py` checks that list against the code, and `scripts/check.sh` runs it. In `tests/suite/accessibility.rs`, `every_demonstration_has_a_step_in_the_tour` checks that every page of the navigation has a step in the tour.
