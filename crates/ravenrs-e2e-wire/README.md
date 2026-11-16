# ravenrs-e2e-wire

The test channel of RavenRS apps. A tool, such as Raven Test Studio, opens the app under test in a child process with no window and drives it, one JSON line per step. On the app's side a [`ravenrs-e2e`](../ravenrs-e2e) session runs each step, so a step behaves the same whether the app is compiled into the tool or opened apart.

This is the first part of phase F6 of [#627](https://github.com/alexkads/RavenRS/issues/627) ([#649](https://github.com/alexkads/RavenRS/issues/649)).

## How the app turns it on

The `serve` feature is on by default: every RavenRS app carries the channel, closed until someone opens the app with `--e2e-serve`. The app decides what the session sees. Raven Finanças opens its sample family in memory and never touches the database of the person using it:

```rust,ignore
if std::env::args().nth(1).as_deref() == Some(ravenrs_e2e_wire::SERVE_FLAG) {
    let session = E2e::new(my_app_with_sample_data(), Theme::raven_light());
    return ravenrs_e2e_wire::serve("My App", session);
}
```

Nothing else may write to standard output while it serves.

## How a tool drives it

```rust,ignore
let mut process = std::process::Command::new(app_binary);
process.arg(ravenrs_e2e_wire::SERVE_FLAG);
let mut app = Client::spawn(process, Duration::from_secs(10))?;
app.send(&Command::Type { target: "#descricao".into(), text: "Conserto da pia".into() })?;
app.send(&Command::Click { target: "Salvar lançamento".into() })?;
let screen = app.send(&Command::Snapshot)?;
```

A target is `#id` for a test id, or the text shown. A failed step comes back as a `Failure` with its stable code (`E2E_NOT_FOUND`, `E2E_EXPECTATION`…), how to fix it, what came close and the screen of the app as it was. The channel adds three codes:

| Code | Means |
|---|---|
| `E2E_APP_START` | the app did not start, or ended before saying hello or during a step |
| `E2E_TIMEOUT` | a `wait` ran out of time, or the app did not answer a step in time and the client ended it |
| `E2E_WIRE` | a line that is not part of this protocol |

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

Commands: `click`, `type`, `fill`, `select`, `expand`, `collapse`, `scroll`, `wait` (with an optional `text`), `check` (value, or name when there is no value, is exactly the text), `mention` (name or description contains the text), `snapshot` and `bye`.

A step does not fail for being early. It tries again for up to `PATIENCE` (2 s), with the session's clock moving, while its target is not there, covered, disabled or out of sight, or its check does not hold yet. When time runs out, it fails with its own code. An action on a part out of sight scrolls to it first, even a row a long list has not built. `wait` waits up to `WAIT` (5 s) and fails with `E2E_TIMEOUT`. In a `.raventest` file the words are `fill`/`preencher`, `select`/`escolher`, `expand`/`expandir`, `collapse`/`recolher`, `scroll`/`rolar` and `wait`/`esperar`.

## Not yet

From the rest of F6: frames over the channel, `E2E_APP_BUILD` with file and line, `e2e_source`, `attach` to a running app by local socket with permission on screen, `cargo ravenrs e2e install`, and the `create-ravenrs` template.
