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
| Básicos | `Button`: emphasis, sizes, icons, accessible names and descriptions, disabled/loading/selected states, width and keyboard activation. `Text`: the full type scale, custom size/weight/color, alignment, truncation and reactive bound/dynamic content. `Toggle` (Chaves): with a label, compact, with only an accessible name for a cell of a grid, disabled and read only. `InputOtp` (Código de verificação): digits only, the code complete once, and the boxes locked while it is verified, keeping what was typed. `Checkbox` (Caixas de seleção): plain, a select all that shows mixed over its children and checks or unchecks them all, an answer that may be unknown (unknown, yes, no), labels lined up in a column, disabled and read only. `Card` (Cartões): its inside space, corners, fill, hairline and shadow, a card that is clicked, the one chosen of a group, and the tones that tint its surface and hairline. `Avatar`: initials on a color from the name, sizes, a picture, a ring for the chosen one of a row, and the circle or rounded square, with the picture cropped to it. `DropdownMenu` (Menus): items with an icon, a key, a check mark, a view at the end or a submenu, one off that says why, a destructive one, separators, titled sections and a header, opened from a button in four styles and sizes, from only an icon, from any view (the account at the foot of a sidebar) or by the app |
| Validação | Rules of every kind, warnings and notes, checking on leaving a field |
| Aparência condicional | Colors, enabled and visible, on fields and actions; words of the screens replaced by key, and keys of the center's own ("Dizer olá") |
| Actions | Simple, with a question, with a value, with a choice, with a window, an address, by criteria, inside a form, messages, one that runs in the background, and "Fechar e dobrar", declared with no code |
| Editores de campo | Every kind of field, and fields that come from the model |
| Listas | Chart, month, timeline and tree; variants and filters; editing on the list; many rows; a panel written in code (Painel em código) |
| Relações | Lists that belong to a record, link and unlink, a filtered reference, cloning |
| Temas | The token editor, changing the whole application at once |
| Kit | `ExpressionEditor` with every feature: fields and parameters offered, the lines shown, and the preview over a sample order, a result or why it failed. `ParameterForm` with every feature: a schema of text, number, yes or no, choice, a field shown in one mode, an expression, a group, a list of headers, and options of the account that arrive, load or fail, and a form inside a dialog that gives back each change at once, with no Aplicar (`live`) |
| Listas e árvores | `DataInspector` with every feature: orders as table, tree, JSON and schema, the item and field chosen kept between them, absent told from null, a 19-digit number exact, five thousand rows cut with a notice; Log de build, a `LogView` over a `LogBuffer`: ANSI colors, follows the end, search, stages that fold, a line pointed at by its number, and a long line that runs on as in a terminal, with the list scrolling across |
| Gráficos e fluxos | `FlowCanvas` with every feature (options, rules, events, check, run, export, connections as cables), and `CablePlug` in eight uses (flow, processes, a rack, a switchboard, a synthesizer, a network rack, matching columns, a tip tied to a button) and a lab with every knob. `CABLE_SNAPSHOTS=<folder> cargo test -p raven-feature-center --test cabos -- --ignored` draws each with the GPU into PNGs. `PatchPanel` in three uses: the back of a rack with a front that turns around, a switchboard whose cords go home and work in pairs, and a panel with its own rule |
| Atualização | `NewsDialog` of `ravenrs-app-update` (feature `ui`): the news of an example app with a newer version, the one installed and an older one, opened on the version running, on the newest and as the details of a failed update, with the version button and the notice of the status bar every RavenRS app shows | `ServiceSpec` and `ServiceManager` (Serviço do sistema): the unit of systemd, the plist of launchd and the task of Windows of the example app, with and without the memory limit, for a user or the whole system, and the commands each would run to install, see and remove it, in a dry run.
| Doca e faixa | `DockArea` in three uses: an editor whose documents stay in the center, with a document that will not close while unsaved, one that closes for good, the arrangement kept as text and restored; a photo desk where anything goes anywhere, floats or hides at an edge; and a fixed page of columns that only the app opens and closes. `Ribbon` over a report: the app menu, quick access, large, small and icon-only commands, toggles, a drop-down, a command turned off, a dialog launcher, the contextual page of a table, the search and the open page |
| Sem armazenamento | A form with no storage behind it |
| Estados e lembretes | A state machine, and a reminder that comes up 30 seconds after opening |
| Segurança | The same documents as each of the three people |
| Auditoria | Each mode side by side, a field left out, changes in collections |
| Acessibilidade | Names and descriptions for screen readers from the model, the focus order of a form, an action that says its keys and announces what it did |
| Servidor | API web: the options of the web API, `ApiOptions`, with the origins a browser may call from and how many threads answer at the same time and the live channel, with an event published on it and the webhooks of other services let in with no token, with their headers (`new`, `allow_origin`, `workers`, `live`, `open`, `pass_header`); desktop only |
| Servidor | Serviço da API: the service of the web API, `ApiService`, each feature asked through `handle` as a request from outside: signing in with the lifetime of a session and the limit of tries, a guest let in by an invitation (`authenticator`), a setting per person, an endpoint, a signed webhook, the description with its title and an entity hidden, how roles add up, the words of the rules, the audit, and a route that changes while it runs (`fallback`, answered with `ApiResponse::raw`) (`new`, `title`, `rule_words`, `role_merge`, `audit`, `authenticator`, `hide`, `session_lifetime`, `sign_ins_a_minute`, `setting`, `webhook`, `endpoint`, `fallback`, `handle`); desktop only |
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

## End-to-end tests (Raven Test Studio)

The scenarios of this tool are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: in the showcase, in memory, Ana is filled in at the door; it signs in, goes to the buttons, counts an activation and pins. The test session opens the showcase (`--modo vitrine`), so nothing goes to the SQLite file of the whole center. The tool brings them (`SCENARIOS`), and `tests/suite/scenarios.rs` plays them on every `cargo test`.

```bash
raven-test-studio --project tools/raven-feature-center --run-all --no-cache --format json --out target/e2e/raven-feature-center
```

The tool answers the Studio when opened with `--e2e-serve`, with no window; its name on the channel is "Raven Feature Center" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`).

