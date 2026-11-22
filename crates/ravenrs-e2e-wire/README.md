# ravenrs-e2e-wire

The test channel of RavenRS apps. A tool, such as Raven Test Studio, opens the app under test in a child process with no window and drives it, one JSON line per step. On the app's side a [`ravenrs-e2e`](../ravenrs-e2e) session runs each step, so a step behaves the same whether the app is compiled into the tool or opened apart.

This is the first part of phase F6 of [#627](https://github.com/alexkads/RavenRS/issues/627) ([#649](https://github.com/alexkads/RavenRS/issues/649)).

## How the app turns it on

The `serve` feature is on by default: every RavenRS app carries the channel, closed until someone opens the app with `--e2e-serve`. The app decides what the session sees. Raven Finanças opens its sample family in memory and never touches the database of the person using it:

```rust,ignore
if std::env::args().nth(1).as_deref() == Some(ravenrs_e2e_wire::SERVE_FLAG) {
    let session = E2e::new(my_app_with_sample_data(), Theme::raven_light());
    return ravenrs_e2e_wire::serve(ravenrs_e2e_wire::Hello::of("My App"), session);
}
```

Nothing else may write to standard output while it serves.

## With the real window

Opened with `--e2e-serve --e2e-window` (`WINDOW_FLAG`, what `--headed` sends), an app that supports it shows its real window and answers the same commands on the interface the window shows. It builds its view as usual, on its test data, and hands `ravenrs::native::run_driven` a `Headed` driver:

```rust,ignore
let args: Vec<String> = std::env::args().collect();
if args.iter().any(|a| a == ravenrs_e2e_wire::WINDOW_FLAG) {
    let driver = ravenrs_e2e_wire::Headed::new(ravenrs_e2e_wire::Hello::of("My App"));
    return ravenrs::native::run_driven(my_app_with_sample_data(), WindowOptions::new("My App — teste"), driver);
}
```

The window's platform lends its `Ui` to the driver on the main thread whenever a command arrives; standard input is read on a thread of its own, so the window keeps drawing between steps. Each command goes through the same code as with no window, so the codes and the screen in a failure are the same. The hello says `window: true`; a tool that asked for the window and hears no such hello refuses with `E2E_NO_WINDOW`. Between steps the app's clock runs at most `SETTLE` (0.6 s) and then stands still until the next one, so a pace a person can follow never changes a verdict. While driven, the person's mouse and keyboard do not reach the window; closing it answers the waiting step with `E2E_WINDOW_CLOSED`. `Command::Release` ends the test and leaves the window to the person; `Client::release` sends it and waits for the window to be closed. The test window has no tray, single copy or app id, so it never reaches a copy of the app the person has open.

## How a tool drives it

```rust,ignore
let mut process = std::process::Command::new(app_binary);
process.arg(ravenrs_e2e_wire::SERVE_FLAG);
let mut app = Client::spawn(process, Duration::from_secs(10))?;
app.send(&Command::Type { target: "#descricao".into(), text: "Conserto da pia".into() })?;
app.send(&Command::Click { target: "Salvar lançamento".into() })?;
let screen = app.send(&Command::Snapshot)?;
```

A target is `#id` for a test id, or the text shown. A failed step comes back as a `Failure` with its stable code (`E2E_NOT_FOUND`, `E2E_EXPECTATION`…), how to fix it, what came close and the screen of the app as it was. The channel adds these codes:

| Code | Means |
|---|---|
| `E2E_APP_START` | the app did not start, or ended before saying hello or during a step |
| `E2E_TIMEOUT` | a `wait` ran out of time, or the app did not answer a step in time and the client ended it |
| `E2E_WIRE` | a line that is not part of this protocol |
| `E2E_NO_FILM` | the app has no film adapter, or could not keep the film |
| `E2E_NO_FRAME` | the app has no vector-frame adapter, or could not capture the frame |

The child ends when the `Client` is dropped.

## The protocol

Standard input and output of the child, one JSON object per line. The app first says hello, then answers each command in order:

```text
app  → {"protocol":"ravenrs-e2e-wire","version":1,"app":"Raven Finanças"}
tool → {"op":"click","target":"#nova-despesa"}
app  → {"ok":true}
tool → {"op":"check","target":"#aviso","text":"Lançamento salvo"}
app  → {"ok":false,"failure":{"code":"E2E_NOT_FOUND","message":"…","screen":"…"}}
tool → {"op":"bye"}
app  → {"ok":true}
```

Commands: `click`, `type`, `fill`, `select`, `expand`, `collapse`, `set_value`, `show_menu`, `dismiss`, `scroll`, `drag`, `wait` (with an optional `text`), `check` (value, or name when there is no value, is exactly the text), `mention` (name or description contains the text), `expect` (the state: `checked`, `unchecked`, `disabled`, `enabled`, `focused`, `visible` or `count N`; an unknown one is refused when the scenario is read), `snapshot`, `frame`, `source`, `film`, `film_stop` and `bye`.

`frame` answers with `{"ok":true,"frame":[/* bytes */]}`: a single-frame `.ravenmovie` containing the app's current display list and assets. `Client::frame()` returns those bytes; decode with `ravenrs_movie::Movie::from_bytes` and show in MoviePlayer. The app supplies `ChannelFilm::default()` to `serve_filmed`, the same adapter it uses to film. There is no GPU, shared temporary file, clock advance or change to a film in progress. A `Filming` implementation that only films can leave its new `frame` method unimplemented and gets `E2E_NO_FRAME`. Plain `serve` also refuses frames. `Client::send` is for ordinary commands: it refuses to discard frame data and tells the caller to use `Client::frame`.

`Reply.frame` is optional, so replies from older peers still read. The protocol remains version 1: existing commands keep their meaning, and the additional command and field are optional. A peer from before `frame` reports an unsupported command as `E2E_WIRE`. The source build stage is in `ravenrs-e2e-runner`, which reports `E2E_APP_BUILD` with Rust's source location before opening this channel.

`Client::source("#save")` sends `{"op":"source","target":"#save"}` and reads `{"ok":true,"source":{"file":"src/app.rs","line":42,"column":7}}`. The app under test must enable `ravenrs-e2e-wire/e2e-source`; `source` works with plain `serve` too and needs no movie adapter. It reports the declaration of the test id, captured with `track_caller`, not a guessed widget implementation. Without instrumentation or a recorded declaration it returns `E2E_NO_SOURCE`; missing and ambiguous queries keep `E2E_NOT_FOUND`/`E2E_AMBIGUOUS`. `Reply.source` is optional for old replies. Use `Client::source`, not `send`, to receive this typed result.

A step does not fail for being early. It tries again for up to `PATIENCE` (2 s), with the session's clock moving, while its target is not there, covered, disabled or out of sight, or its check does not hold yet. When time runs out, it fails with its own code. An action on a part out of sight scrolls to it first, even a row a long list has not built. `wait` waits up to `WAIT` (5 s) and fails with `E2E_TIMEOUT`. In a `.raventest` file the words are `fill`/`preencher`, `select`/`escolher`, `expand`/`expandir`, `collapse`/`recolher`, `set-value`/`definir-valor`, `show-menu`/`abrir-menu`, `dismiss`/`dispensar`, `expect`/`estar` (with the states `marcado`, `desmarcado`, `desabilitado`, `habilitado`, `com-foco`, `visivel` and `quantidade N`), `scroll`/`rolar` and `wait`/`esperar`.

## Not yet

Instrumented `Button::new` and `Text::{new,bound,dynamic}` also supply their constructor's caller without a test id; an explicit tag's origin has priority. Other untagged constructors remain unavailable. Native apps generated by `cargo ravenrs new` / `create-ravenrs` now include this channel. From the rest of F6: origins for the remaining views, `attach` to a running app by local socket with permission on screen, and `cargo ravenrs e2e install`. The Studio's live preview using these vector frames and opening the source in an editor come in F9.
