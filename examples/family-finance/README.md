# Raven Finanças

A desktop app for a family's expenses and bills to pay, with its data in one SQLite file on the computer. It has two jobs: to show what RavenRS can do in a real app, and to find what RavenRS is still missing. When a component falls short, the fix goes into the framework, not here.

```bash
cargo run -p family-finance
```

The first run opens on a sample family, so there is something to explore. **Ajustes → Começar do zero** clears it for real use. The database is `~/.raven-financas/financas.db`, or the path given as the first argument.

## What each screen uses

| Screen | RavenRS |
|---|---|
| Painel | `Stat`, `Alert`, `AreaChart`, `BarChart`, `Grid`, `List`, `responsive` |
| Lançamentos | `DataGrid` (filter row, search, grouping, totals, cell templates, CSV), `SegmentedControl`, `DropdownMenu`, `ravenrs-criteria` |
| Contas a pagar | `DataGrid`, `Dialog`, `FormField`, `DatePicker`, `Select`, `Stepper` |
| Agenda | `Calendar`, `Badge` |
| Orçamentos | `ProgressBar`, `Alert`, `Dialog` |
| Relatórios | `ravenrs-report`, `ravenrs-report-viewer`, `ravenrs-report-export`, charts and barcodes from `ravenrs-graphics` |
| Cadastros | `Tabs`, `ListItem`, `FormField` |
| Ajustes | `ThemeSwitcher`, `Sheet`, `Accordion` |
| Everywhere | `Sidebar`, `CommandPalette` (⌘K), `Toaster`, `Tooltip`, `Localized` |

The filter on Lançamentos is the part to read first. What the user types, such as `[Valor] > 100 And [Categoria] = 'Mercado'`, is checked against a schema and translated by `ravenrs-criteria` into SQL with bound parameters, so typed text can never change the query ([src/filter.rs](src/filter.rs)).

## How the code is laid out

| Path | What it holds |
|---|---|
| `src/db.rs` | Schema, migrations and every query. Money is whole cents; writes that touch several rows run in one transaction |
| `src/model.rs`, `src/money.rs` | The records and the rules that need no database |
| `src/filter.rs` | Expressions to SQL |
| `src/reports.rs` | The report definitions |
| `src/update.rs` | Finding and installing a new version |
| `src/ui/` | One file per screen; `store.rs` has what they share |
| `tests/db.rs`, `tests/app.rs` | The rules on a real database, and the app driven through `Ui`, without a window |
| `tests/screens.rs` | Draws every screen to a PNG file |

```bash
FF_SCREENSHOTS=$PWD/target/screens cargo test -p family-finance --test screens
```

## Installing and updating

The app is not packaged. It installs and updates by compiling the master branch on the user's machine, the way VintageLightbox does, using `install.cmd`, one file that runs on Windows, macOS and Linux.

```bash
curl -fsSLO https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd
```

```bash
sh install.cmd
```

On Windows, download the file and double-click it.

At startup, the app reads `novidades.json` from master. If that version is newer, a banner says what changed and an installed copy starts compiling at low priority. The installed app is only replaced after the new binary answers `--versao`, and the old one stays beside it as `.anterior`. A version that failed is not retried by itself for a day. A copy built with `cargo run` only mentions the new version.

**To release**, make one commit with three changes: `version` in `Cargo.toml`, `novidades.json`, and the same entry with its `data` on top of `historico-de-novidades.json`. A test holds them together. Make it the last commit of the release: anyone who compiled 0.1.N will not compile another 0.1.N.

Updates come over HTTPS from this repository, with no signature of their own.

## What this example found in RavenRS

Fixed in the framework:

* Controls ignored `Align::Center` in their stack (`Align::Fit`).
* `None` and other empty views took a gap in their stack.
* `DocumentViewer` assumed it had the whole window.
* A doughnut chart's legend listed categories with no slice.
* Component text was fixed in English (`Labels`, `Localized`).
* `Stat` could not show spending going up as bad (`Stat::delta_toned`).

Still open:

* **Waking the UI from another thread.** Nothing lets a background thread ask for a frame, so `UpdatePump` comes back once a second while work runs.
* **Desktop file dialogs.** Exports go to a fixed folder because the native build has no save dialog.
* **Opening a file with the system's program.** `paths::open_with_system` repeats what `report-designer` has; it belongs in the framework.
* **Chart axes.** `AreaChart` has no axis labels or tooltips, so the dashboard writes the first and last month by hand.
* **More text to translate.** `Ribbon`, `Carousel`, `InputOtp`, `validate::email`, `validate::min_length` and a grid's yes/no values are still in English.

Missing in the app itself: transfers between accounts, so a credit card's balance has no way to be paid off, and the chosen theme is not remembered.
