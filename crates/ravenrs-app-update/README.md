# ravenrs-app-update

Installing, updating and versioning a [RavenRS](https://github.com/alexkads/RavenRS) app by compiling it on the user's machine. The app is not packaged: one file, `install.cmd`, runs on Windows, macOS and Linux, gets Rust and a C compiler when they are missing, downloads the code and compiles it. The same file is what the app runs to update itself.

The crate draws nothing and knows no app. Who the app is, its name, folders and addresses, is one `InstalledApp`; the app shows what the crate says with its own components.

## What it does

When it opens and every hour after that, the app reads `novidades.json` from the main branch, silently. If that version is newer than the running one:

1. the status bar says so, and an installed copy starts compiling at low priority (`nice -n 10` on Linux and macOS), telling each step the installer announces in its log (`> baixando o codigo`, `> compilando`);
2. the installer downloads to a file, never into a pipe, and is refused unless it is whole: an error page or a file cut in the middle touches nothing;
3. it compiles in a folder of its own and runs the new binary with `--versao` before swapping;
4. the installed app becomes the previous version, kept on macOS in `~/<home_dir>/anterior` (where it does not show as a second app) and elsewhere beside it as `.anterior`, and the new one moves in;
5. the app asks the installed binary which version it is; only then does the status bar offer **Reabrir agora**.

With no window, a command line or another program asking the app (a pane beside a Claude conversation asks Raven Test Studio) uses `check_now`, which reads the same news and decides by the same rule, and `install_now`, which runs the same installer with the same lock and memory, on the calling thread, telling each stage.

A version that failed is not tried again by itself for a day (`RETRY_AFTER`); clicking **Tentar de novo** tries at once. Two windows never compile together: the second follows the first one's steps through a lock in `~/<home_dir>`, which counts as stale after three hours or twenty minutes of a silent log. A copy built with `cargo run` only mentions the new version.

| In `~/<home_dir>` | What it is |
|---|---|
| `install.cmd` | the installer as downloaded, which is what ran |
| `atualizacao.log` | what the installer wrote; the details of a failure show its end |
| `atualizacao.trava` | the time an installer began, while it runs |
| `atualizacao.json` | the version that failed last, and when |
| `fonte/`, `target/` | the code and the build, which updates reuse; deleting them is safe |

## Adopting it in a new app

**1. Say who the app is.** Add the dependency (`ravenrs-app-update.workspace = true` inside this workspace) and one `static`:

```rust
use ravenrs_app_update::{InstalledApp, Updater};

pub static APP: InstalledApp = InstalledApp {
    name: "Raven Flows",
    package: "raven-flows",
    home_dir: ".raven-flows",
    bundle_id: "dev.ravenrs.raven-flows",
    icons: "examples/raven-flows/assets",
    mac_category: "public.app-category.productivity",
    linux_categories: "Office;Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RF",
    installed_env: "RAVEN_FLOWS_INSTALLED",
    news_urls: &["https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/novidades.json"],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    installed: option_env!("RAVEN_FLOWS_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};
```

- `package` is the Cargo package and the binary. On Linux it also names the `.desktop` file and the icon, so open the window with it as its id (`ravenrs::prelude::AppIdentity::new(APP.package)`), or GNOME shows no icon.
- `installed` must read the same variable `installed_env` names: the installer sets it while it compiles.
- `icons` is a folder with `icon.ico`, `icon.icns` and `icon.png` (256 px).
- An installed copy only knows the addresses it was compiled with. Add addresses; never move one.

**2. Answer `--versao`.** The installer and the updater both ask the new binary which version it is, before anything is swapped:

```rust
if std::env::args().nth(1).as_deref() == Some("--versao") {
    println!("{}", APP.version);
    return;
}
```

**3. Keep the three release files.** Give the app its own `version` in `Cargo.toml` (not `version.workspace`), and beside it `novidades.json` and `historico-de-novidades.json`, as in `examples/family-finance`. Then one test holds them together:

```rust
#[test]
fn the_three_release_files_agree() {
    ravenrs_app_update::assert_release_files_agree(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
}
```

It fails when the three versions differ, when the history's first entry is not the same one with its `data`, or when `contribuidores` or `issues` is empty. Releasing is one commit with the three changes, the last of the release: anyone who compiled 0.1.N will not compile another 0.1.N.

**4. Make its installer.** Create an empty `install.cmd` beside `Cargo.toml` and this test, then run it once with `RAVENRS_WRITE_INSTALLER=1`:

```rust
#[test]
fn the_installer_is_the_one_the_template_makes() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("install.cmd");
    ravenrs_app_update::assert_installer_is_current(&APP, &file);
}
```

```bash
RAVENRS_WRITE_INSTALLER=1 cargo test --manifest-path examples/raven-flows/Cargo.toml the_installer_is_the_one
```

From then on the test fails if the copy and [`install.cmd.in`](install.cmd.in) part ways. Change the template, never the copy, and write the copies again. A field that would change what the script does (a quote, a `$`, a `%`) is refused before anything is written.

**5. Put it on screen.** Turn on the feature `ui` (`ravenrs-app-update = { workspace = true, features = ["ui"] }`) and every app shows the news and the notice the same way, with nothing to draw:

```rust,ignore
use ravenrs_app_update::ui::{NewsDialog, version_button, with_notice};

let news = NewsDialog::new().on_open_file(|log| { let _ = ravenrs::native::open_with_system(log); });
// In the status bar: the version opens the news of the version running; the notice, in the middle.
let bar = StatusBar::new().left((version_button(format!("Raven Exemplo · versão {}", APP.version), &updater, &news),));
let bar = with_notice(bar, &updater, &news);
// Somewhere in the tree, over the screen: "Novidades", or "Detalhes da atualização" when one failed.
let dialog = news.view(&updater);
```

`ui::release_note` draws one version (what changed, why update or why it mattered, **Quem contribuiu**, **Issues resolvidas** opening through the app's links, the date, "Nova" with **Como atualizar** and the installer's command, or "Instalada"), and `ui::release_notes` every version, for a `ReleaseNotes` placed elsewhere, as Raven Flows does in its settings. Raven Finanças, Raven Flows and Raven Test Studio all use it; a new app should too, instead of drawing its own.

What it is made of, for an app that needs something else:

- create it with `Updater::idle(&APP)`, and call `watch()` once the window is open;
- in a view that builds every frame, call `connect(move || waker.wake())` with `cx.ui_waker()`, then `poll()`;
- `notice()` is the status bar's sentence and its buttons, each `NoticeAction` with its `label()` and `is_primary()`; the buttons call `install`, `reopen`, `ask` and `dismiss`;
- `timeline()` is every version known, newest first, for `ReleaseNotes`, with `contribuidores` under **Quem contribuiu** and `issues` linked by `APP.issue_url(n)` under **Issues resolvidas**;
- a failure's details are its reason, `stage()` and `APP.installer_log_tail(n)`, with `APP.install_command()` for running the installer by hand.

`ui` does all of this with `StatusBar`, `ReleaseNotes`, `Dialog` and `Button`, and **Procurar atualização** in the Ajustes of Raven Finanças is `ask()`.

**6. Show the commands in the README.** `APP.install_command_on(true)` (PowerShell) and `APP.install_command_on(false)` (a terminal) are the commands to paste; the app shows the one for its system when updating from inside fails.

## An app that is a server: a service of the system

A server (Raven CI) runs with no window and no one watching it, so the system's own manager of
services starts it with the machine and again when it falls. `ServiceSpec` says what to run
(binary, arguments, environment, `memory_max`, folder of logs, user or whole system) and
`ServiceManager` writes the file each manager reads and asks it to start, stop or say how it is:

| System | File | What it holds |
|---|---|---|
| Linux, systemd | `~/.config/systemd/user/<pacote>.service`, or `/etc/systemd/system` | `Restart=on-failure`, `MemoryMax=`, `KillSignal=SIGTERM`, `TimeoutStopSec`; `systemctl [--user] enable --now`, then `restart` |
| macOS, launchd | `~/Library/LaunchAgents/<bundle_id>.plist`, or `/Library/LaunchDaemons` | `RunAtLoad`, `KeepAlive` when it falls, output in the folder of logs; `launchctl bootout` and `bootstrap`. No memory limit: macOS applies none to a service, and the file says so |
| Windows, Task Scheduler | `<pasta>/<pacote>-tarefa.xml`, in UTF-16 | at logon (or at boot as SYSTEM), again a minute after it falls, no time limit; `schtasks /Create /XML` and `/Run`. No memory limit. A service of the Service Control Manager needs the binary to speak to it, and is left for when it can be tested on Windows |

```rust
use ravenrs_app_update::{ServiceManager, ServiceSpec};

let spec = ServiceSpec::for_app(&APP, std::env::current_exe()?)
    .args(["servir", "--endereco", "0.0.0.0:8080"])
    .memory_max("1G");
if let Some(manager) = ServiceManager::current() {
    let done = manager.install(&spec)?; // the file written, the commands run, what to know
}
```

`ServiceManager::dry_run(folder)` writes the files in `folder` and runs no command: the three
systems are tested on any machine. The installer installs the service at the end when
`<PREFIXO>_SERVICO=1` (`RCI_SERVICO=1 sh install.cmd`), by running the app's own
`servico instalar`; an app with no such command is never asked.

## Why each app keeps a whole `install.cmd`

The template is one, here; each app commits the copy made from it. A one-line `install.cmd` pointing at the template would not work:

- installed copies download the address they were compiled with and refuse a file that is not the whole installer, so that address must keep serving the whole thing;
- on Windows people double-click the file, and `cmd` has no way to include another one;
- a stub would download a second file at install time, which is a second way to get half a script.

The test of step 4 keeps the copies from drifting, so the 300 lines are written once and checked everywhere.

## What is tested where

`cargo test --manifest-path camadas/telas/Cargo.toml -p ravenrs-app-update` runs the rules with no network: the decision, the hour between looks, the day after a failure by a given clock, the status bar's sentences, the lock, the steps, the template for two apps that must not touch each other's folders, the release files, and, on Linux and macOS, an installer that does not compile, which swaps only when the new binary answers, keeps the previous one, and is refused when cut in half. Windows runs the same Rust, but its PowerShell part is only tested by hand.
