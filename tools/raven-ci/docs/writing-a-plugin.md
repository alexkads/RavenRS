# Writing a plugin for Raven CI

Raven CI has a core that knows a queue, jobs and builds, and nothing else. Git, GitHub, the shell,
schedules, test reports, Docker and every pipeline format come from plugins, and a plugin you write
uses the same API as the ones that ship with the server. This page explains what a plugin is, how
the server treats it, and how to write one in Rust or in any other language.

## What a plugin is

A plugin is a [`ravenrs_app::Module`](../../../crates/ravenrs-app/src/module_api.rs) with the
extension points of the CI on top, the trait `CiPlugin` of
[`raven-ci-plugin-api`](../crates/raven-ci-plugin-api/src/lib.rs). From the module it gets its name,
its manifest (version, the versions it accepts of other plugins, the permissions it needs), its
entities and its screens. From `CiPlugin` it gets one method per extension point:

| Point | What it offers | Example |
|---|---|---|
| Step | a kind of step of a pipeline, a `NodeKind` of `ravenrs-flows` | `sh`, `cargo`, `kubectl` |
| Scm | fetching code into the workspace | git |
| Branch source | finding branches and pull requests | GitHub multibranch |
| Trigger | when a build is asked for | cron, webhook, polling |
| Publisher | what a build leaves, as attachments | artifacts, JUnit |
| Status reporter | the status of a commit at its provider | GitHub checks |
| Notifier | telling people | e-mail |
| Credential kind | a kind of credential and what a log hides of it | token, SSH key |
| Agent launcher | how an agent comes up | container, pod |
| Pipeline format | reading a pipeline file into a flow | `.raven-ci.toml` |
| Routes | addresses of the HTTP API | `/api/github/hook` |
| CLI command | subcommands of `raven-ci` | `raven-ci converter` |
| Screens | pages and tabs of the panel, through the module | the JUnit tab of a build |

Each trait's documentation says what it is for, when the core calls it and how to write one.

## How the server treats a plugin

1. **Found.** Compiled-in plugins are added by the server; external ones are found in the folders
   of plugins, one folder per plugin, each with a `plugin.toml`.
2. **Planned.** The server reads every manifest and puts the plugins in an order where each comes
   after what it requires, with `ravenrs_app::plan_modules`. A plugin that does not fit stays out
   with the reason, and so do the plugins that require it; the others go on. `raven-ci plugins
   listar` shows the reason, such as *precisa de git a partir da 2.0.0 e abaixo da 3.0.0, e o
   instalado é 1.4.0*.
3. **Started.** The plugins switched on start in that order. Before `start`, the server checks the
   plugin's configuration against the entity `config()` describes and applies its migrations to its
   own SQLite database, each once. `start` gets a `Host`.
4. **Asked.** Only started plugins are asked for their points.
5. **Stopped.** Switching a plugin off stops the plugins that require it first, then it. Nothing
   restarts, and switching it back on brings them back.

A plugin switched off costs nothing but its struct: its constructor does no work, `start` does.

## Permissions

A plugin declares what it asks the host for. A compiled-in plugin declares it as capabilities of its
manifest, and an external one in its `plugin.toml`:

| Permission | Capability | Lets it |
|---|---|---|
| `network` | `ci.network` | talk to other machines |
| `process` | `ci.process` | start programs |
| `secrets` | `ci.secrets` | read credentials |
| `disk` | `ci.disk` | have a folder and a database of its own |

The server grants what was declared, unless the administrator took it away. Every service of the
`Host` checks its permission, and every answer, yes or no, goes to the audit with the plugin and
what it wanted. A plugin compiled in runs in the server's process, so for it a permission is a
contract checked at every service and a list a reviewer reads, not a sandbox. An external plugin
runs in a process of its own, started with only the basic variables of the environment, and what it
asks the host for is checked the same way.

## A plugin in Rust

Add a crate `raven-ci-plugin-<name>` that depends on `raven-ci-plugin-api`, `ravenrs-app` and, for
steps, `ravenrs-flows`:

```rust
use std::sync::{Arc, Mutex};

use raven_ci_plugin_api::{CiError, CiPlugin, Host, Permission};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports};

/// A step that says hello in the items it passes on.
struct Hello;

impl NodeKind for Hello {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("hello", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Ok(Ports::from([("main".to_owned(), cx.input("main").to_vec())]))
    }
}

#[derive(Default)]
struct HelloPlugin {
    host: Mutex<Option<Arc<dyn Host>>>,
}

impl Module for HelloPlugin {
    fn name(&self) -> &str {
        "hello"
    }
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version("0.1.0")
            .expect("a version")
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for HelloPlugin {
    fn summary(&self) -> String {
        "Diz olá.".to_owned()
    }
    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.log("ligado");
        *self.host.lock().expect("lock") = Some(host);
        Ok(())
    }
    fn stop(&self) {
        *self.host.lock().expect("lock") = None;
    }
    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Hello);
    }
}
```

The server adds it with `Ci::add_plugin(Arc::new(HelloPlugin::default()))`.

**Testing it.** The `testing` feature of `raven-ci-plugin-api` has a `FakeHost` that grants what it
is told and keeps the audit, and a fake plugin for every point to put next to yours. The tests of
`raven-ci-core` (`tools/raven-ci/crates/raven-ci-core/tests/suite`) show a core in memory running
builds with them.

## A step and its build

A step is a `NodeKind` of `ravenrs-flows`, which knows nothing of builds. What a step of a CI needs
(the folder the code is in, the log people read while it runs, the variables of the build) the core
keeps in a `BuildScope`, one per build that runs, and a step finds the scope of its build with
`BuildScope::of(cx)`:

```rust
use raven_ci_plugin_api::BuildScope;
use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};

struct Where;

impl NodeKind for Where {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("where", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        build.log(&cx.node().id, &format!("na pasta {}", build.context().workspace.display()));
        Ok(BuildScope::pass_on(cx))
    }
}
```

- **`context()`**: the job, the number, the folder (`workspace`) and the run.
- **`log(step, line)`**: a line in the build's log, kept in the core's file as it comes, with every
  secret hidden as `****` (the secrets plugins read, and what `hide` was told).
- **`environment()`**: what the programs of the step get: `JOB_NAME`, `BUILD_NUMBER`,
  `BRANCH_NAME`, `WORKSPACE`, `GIT_COMMIT` when code was fetched, the parameters of the build and
  the `environment` of the pipeline.
- **`at_end(what, undo)`**: something to undo when the build ends, however it ends, after its post
  steps and publishers: a lock to let go, a container or a pod to remove. The last asked runs
  first.
- **`pass_on(cx)`**: the items that reached the step, for the next one. A step that changes
  nothing in them gives them on, so every step finds the build.

Around the steps the core does the rest of a build's life, in this order: it makes the folder
(`espacos/<job>/<number>`), fetches the code with the `scm` the job's settings name, sends
`pending` to each `status` and tells each `notify` that the build started; then the steps run,
then the `post` steps (`always`, then `success` or `failure`), then the publishers of the pipeline;
then what `at_end` was asked, the final status, the notices, and the folder is removed (unless
the job's settings say `workspace.keep`). What fails after the steps goes to the log and does not
change how the build ended.

A format marks each node with the plugin its kind comes from (`PLUGIN_KEY`), so a pipeline that
needs a plugin switched off is refused saying "falta o plugin shell" before anything goes to the
queue. The plugins `shell` (`crates/raven-ci-plugin-shell`) and `format-toml`
(`crates/raven-ci-plugin-format-toml`) are the first real ones, and the place to look when writing a
step or a format.

## A plugin in any language

An external plugin is a program that speaks JSON-RPC 2.0 on its standard input and output, one
message per line, as MCP servers do. The example is
[`tools/raven-ci/plugins/hello`](../plugins/hello), in Python with only its standard library.

### Its folder

```
plugins/
  hello/
    plugin.toml
    hello.py
```

### `plugin.toml`

```toml
name = "hello"                # lower-case letters, numbers and '-'
version = "0.1.0"
summary = "Diz olá."
command = "python3"           # a name in PATH, or a path inside the folder
windows_command = "python"    # used on Windows instead, when given
args = ["hello.py"]
permissions = ["secrets"]     # network, process, secrets, disk
timeout_ms = 10000            # the longest one call may take (30 s by default)
idle_ms = 60000               # stopped after this long with no call (60 s by default)
after = []                    # plugins that come before it, with no version asked

[requires]                    # the versions it accepts of other plugins
# git = "^2"

[[steps]]                     # kinds of step it brings
kind = "hello"
title = "Olá"
# version = 1
# required = ["who"]

[[commands]]                  # subcommands of raven-ci it brings
name = "hello"
summary = "Diz olá no terminal: raven-ci hello Ana"
```

The server reads the file without starting the program, so a plugin switched off never runs. A
mistake in it is said with every problem at once, and the other plugins go on.

### The conversation

The program starts at the first call, in its folder. Each message is one JSON object on one line;
flush after each. Anything written to the error output goes to the server's log.

**`initialize`**, always first. Answer the name and the version of the `plugin.toml`; anything else
stops the program.

```json
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"server":"raven-ci","plugin":"hello","protocol":1}}
← {"jsonrpc":"2.0","id":1,"result":{"name":"hello","version":"0.1.0"}}
```

**`step/execute`**, for each node of a build whose kind is one of its steps. `inputs` are the items
that came in, by port; answer the items of each output port.

```json
→ {"jsonrpc":"2.0","id":2,"method":"step/execute","params":{"kind":"hello","node":"ola","parameters":{"who":"Ana"},"inputs":{"main":[{"n":1}]}}}
← {"jsonrpc":"2.0","id":2,"result":{"outputs":{"main":[{"n":1,"hello":"Olá, Ana!"}]}}}
```

**`cli/run`**, for `raven-ci <command> <args…>`. Answer the exit code and what to write.

```json
→ {"jsonrpc":"2.0","id":3,"method":"cli/run","params":{"command":"hello","args":["Ana"]}}
← {"jsonrpc":"2.0","id":3,"result":{"code":0,"output":"Olá, Ana!\n"}}
```

**`shutdown`**, a notification with no `id`: end now. The server stops the program shortly after.

A step or a command that fails answers an `error`; the server shows its `message` and the program
stays up:

```json
← {"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"o repositório não existe"}}
```

### Asking the host

While it answers a call, the program may ask the server for a service: send a request with an `id`
of its own and read lines until the answer with that `id` comes. Each service checks its
permission, and the answer goes to the audit.

| Method | Params | Answers | Needs |
|---|---|---|---|
| `host/log` | `{"line": "…"}`, as a notification with no `id` | nothing | nothing |
| `host/settings` | none | the configuration of the plugin | nothing |
| `host/secret` | `{"name": "token"}` | the secret, which every log then hides | `secrets` |
| `host/data_dir` | none | the path of its own folder | `disk` |

```json
← {"jsonrpc":"2.0","id":"h1","method":"host/secret","params":{"name":"token"}}
→ {"jsonrpc":"2.0","id":"h1","error":{"code":-32000,"message":"o plugin hello não tem a permissão ci.secrets (ler segredos), e por isso não pôde ler o segredo token"}}
```

### When it misbehaves

| What happens | What the server does |
|---|---|
| It does not answer within `timeout_ms` | stops the program; the call fails with *não respondeu a step/execute em N ms*; the next call starts it again |
| It dies in the middle of a call | the call fails with *terminou no meio de step/execute, com o código N*; the next call starts it again |
| It writes a line that is not JSON | the line goes to the log; the call goes on |
| It sits with no call for `idle_ms` | sends `shutdown` and stops it |
| The node is cancelled | stops the program; the node ends as cancelled |

The server never goes down with a plugin.

### Trying it

```bash
raven-ci plugins listar
raven-ci plugins info hello
raven-ci hello Ana
```

`raven-ci plugins desligar hello` switches it off without touching the folder, and `ligar` brings it
back.

## Um plugin Rust compilado pelo sistema

A plugin compiled into `raven-ci` only arrives with a new `raven-ci`. The same plugin, unchanged,
can instead be a program of its own that the server builds and installs:

```bash
raven-ci plugins compilar caminho/do/meu-plugin
raven-ci plugins listar
```

It is still written with the standard traits (`CiPlugin` and its points, as in
[A plugin in Rust](#a-plugin-in-rust)); only its `main` changes. The server reaches it as an
external plugin, over the protocol below, so it may crash, hang or leak without taking the server
down, and it can be added, updated or removed while the server runs. The example is
[`plugins/exemplo-rust`](../plugins/exemplo-rust): a step, a publisher, a trigger and a pipeline
format, which the tests of the tool compile with the command.

### The crate

```toml
[package]
name = "meu-plugin"
version = "0.1.0"
edition = "2024"

[dependencies]
# The version is what `plugins compilar` checks before it builds; `serve` is the main of a plugin.
raven-ci-plugin-api = { path = "…/RavenRS/tools/raven-ci/crates/raven-ci-plugin-api", version = "0.1", features = ["serve"] }
ravenrs-app = { path = "…/RavenRS/crates/ravenrs-app" }
ravenrs-flows = { path = "…/RavenRS/crates/ravenrs-flows" }
```

```rust
fn main() {
    raven_ci_plugin_api::serve(MeuPlugin::default());
}
```

`raven-ci-plugin-api` is not on crates.io, so it comes by `path` (a checkout of RavenRS) or by
`git`. A crate inside the RavenRS checkout is a member of its workspace, as the example is.

The example inherits nothing from the workspace (no `edition.workspace`, no `[lints] workspace`,
no `workspace = true` dependency): it is a plugin of someone else, and copied out of the repository
it compiles as it is, once its `path`s point to a checkout of RavenRS or become `git`. It is a
member of the workspace only so that, inside the repository, its build reuses what the workspace
already compiled.

### What `plugins compilar` does

1. **Checks the API before building.** It reads the `Cargo.toml`: a crate that asks for a
   `raven-ci-plugin-api` this server does not accept is refused with both versions, *o plugin pede a
   raven-ci-plugin-api ^2 (a partir da 2.0.0 e abaixo da 3.0.0), e este Raven CI tem a 0.1.0*, and
   nothing is built.
2. **Builds** with `cargo build --release`, with a `CARGO_TARGET_DIR` of its own under the folder of
   data, removed at the end whether it passed or not. With no `cargo` on the machine, it says so.
3. **Asks the binary for its `plugin.toml`** with `--manifesto`. `serve` writes it from the
   `ModuleManifest` (name, version, permissions, requirements) and from the points the plugin
   offers, with their ids, plus `api`, the version of `raven-ci-plugin-api` it was built with. The
   points are asked **without `start`**, so a plugin returns them from its constructor, as every
   plugin of the house does.
4. **Installs** the binary and the `plugin.toml` in `plugins/<name>/` of the folder of data, each
   written beside and renamed over the old one. The server finds it there the next time it lists
   or applies plugins.

When the server starts the program it checks again: the name and version `initialize` answers have
to be those of the `plugin.toml`, and an `api` it does not accept stops the program.

### What `serve` does

- A thread reads the standard input. Each call of the server runs on a thread of its own, so a
  call that asks the host, and a host that asks the plugin back, never waits on itself.
- `initialize` starts the plugin with a `Host` that asks the server (`host/*`): the server checks
  each permission and keeps the audit, exactly as for a plugin compiled in. A secret read this way
  is hidden in every log.
- `step/execute` runs the node with a `BuildScope` of the build it runs in (job, number, folder,
  variables), so `BuildScope::of(cx)` works as it does in the server; what the step writes to it
  goes to the log of the build, hidden. `at_end` does not cross the protocol: undo in the step.
- `trigger/watch` gives the trigger a `Builds` whose `request` and `scan` are `host/request_build`
  and `host/scan`, which the server answers at any time, not only during a call.

### What the server does with an external plugin that watches jobs

A plugin whose trigger watches a job is never stopped for being idle. When its program falls, the
server starts it again (at most three times a minute) and sends `trigger/watch` again for every job
it watched, so a schedule or a poll goes on.

### Every point, over the protocol

Each point is declared in the `plugin.toml` with its ids, so the server knows what a plugin offers
without starting it; what it has to know before asking (the form of a credential, the method and
path of a route) is declared there too. A plugin in any language can declare and answer them; `serve`
does it for a plugin in Rust.

```toml
[[scms]]
id = "svn"

[[publishers]]
id = "relatorio"

[[pipeline_formats]]
id = "linhas"
file_names = [".linhas"]

[[routes]]
method = "POST"
path = "/hooks/meu"
public = true

[[credential_kinds]]
id = "token-meu"
caption = "Token do Meu"
fields = [
  { name = "usuario", kind = "text" },
  { name = "token", kind = "text", secret = true, required = true },
]

[[config]]                    # the configuration of the plugin, the same vocabulary
name = "url"
kind = "text"
required = true
```

The tables of ids alone are `scms`, `branch_sources`, `triggers`, `publishers`, `status_reporters`,
`notifiers`, `agent_launchers` and `build_hooks`. A field has a `kind` (`text`, `long_text`, `integer`,
`decimal`, `money`, `boolean`, `date`, `date_time`, `color`, `criteria`, or `enum` with `options =
[{ code = "a", caption = "A" }]`), and may be `secret`, `required`, have `min`, `max` and a `pattern`:
the vocabulary `check_settings` understands. A form of `ravenrs-app` with a kind or rule outside it
(an image, a reference, a rule of criteria) cannot be served, and `--manifesto` says which field.

| Method | Params | Answers |
|---|---|---|
| `scm/checkout` | `id`, `request` (url, reference, workspace, depth, credential, merge_onto_target) | the `Checkout`; lines for the build go as `host/build_log` |
| `branch_source/discover` | `id`, `settings` | the heads: `[{name, kind, commit}]` |
| `branch_source/file` | `id`, `settings`, `head`, `path` | the text, or `null` |
| `publisher/publish` | `id`, `build`, `settings`, `workspace` | `[{kind, name, mime, summary, base64}]`, or `file` (a path in `.raven-ci-anexos/` of the workspace, which the server reads and removes) instead of `base64` for big ones |
| `status/report` | `id`, `status`, `settings` | nothing |
| `notifier/notify` | `id`, `event`, `settings` | nothing |
| `credential/hidden` | `id`, `credential` | the values a log hides; the secret fields are hidden as well |
| `credential/secret` | `id`, `credential` | what a plugin that reads the credential gets |
| `hook/before` | `id`, `build`, `settings` | nothing, or an error that refuses the build |
| `hook/after` | `id`, `build`, `status`, `settings` | nothing |
| `route/handle` | `route` (the path declared), `request` (method, path, query, headers, `body` in base64, actor, roles) | `{status, content_type, body}`, the body in base64 |
| `trigger/watch` | `id`, `job`, `settings` | nothing, or a settings error the job shows |
| `trigger/unwatch` | `id`, `job` | nothing |
| `format/read` | `id`, `job`, `text` | `{pipeline}` (the `FlowDefinition` in JSON, `start`, `parameters` as fields, `environment`, `post`, `publishers`, `timeout_ms`, `keep_builds`, `agent`) or `{errors: [{line, message}]}` |
| `agent/launch` | `id`, `request` | `{agent, name}`: `agent` is the handle of the two below |
| `agent/alive` | `id`, `agent` | `true` or `false` |
| `agent/stop` | `id`, `agent` | nothing |
| `step/execute` | `kind`, `version`, `node`, `parameters`, `inputs`, `build` (`context` and `environment`, inside a build) | `{outputs}` |
| `cli/run` | `command`, `args` | `{code, output}` |

What the plugin asks the server, besides `host/log`, `host/settings`, `host/secret` and
`host/data_dir`:

| Method | Params | Answers | Needs |
|---|---|---|---|
| `host/database` | none | the path of its SQLite file | `disk` |
| `host/granted` | none | the permissions granted, by name | nothing |
| `host/require` | `permission`, `what` | nothing, or the refusal, which goes to the audit | the permission |
| `host/build_log` | `call` (the `id` of the call in progress), `step`, `line`; a notification | nothing | nothing |
| `host/hide` | `call`, `value`; a notification | nothing | nothing |
| `host/request_build` | `trigger`, `watched` (the job it watches), `request` (a `BuildRequest`) | the number of the build | a job its trigger watches |
| `host/scan` | `trigger`, `watched`, `job` | what changed, in words | a job its trigger watches |
| `host/attachments` | `job`, `number` (or `null` for every build kept), `kind` | what the builds left, without the bytes: `[{id, job, number, kind, name, mime, size, summary}]` | nothing |
| `host/attachment` | `id` | `{info, base64}`: the attachment and its bytes | nothing |

An error carries `data` that says what it is, so it crosses whole: a refusal of the host is still
*não tem a permissão* on the other side, and a settings error is still the field and the reason.

```json
{"code":-32000,"message":"a configuração porteiro: sem espaço","data":{"kind":"settings","field":"porteiro","reason":"sem espaço"}}
```

`kind` is `denied` (with `plugin`, `permission`, `what`), `settings` (`field`, `reason`),
`unavailable` or `failed` (`what`).

### Compiling faster, and on a busy machine

`raven-ci plugins compilar PASTA --debug` builds in the profile dev: quicker to compile, slower to
run, for trying a plugin out. The whole environment goes to `cargo`, so `CARGO_BUILD_JOBS=3 raven-ci
plugins compilar …` keeps a build from taking every core. With `RAVEN_CI_CARGO_TARGET=PASTA`, the
build uses that folder and keeps it, so the next build of the same plugin only compiles what
changed; without it, each build has a folder of its own, removed at the end. The tests of the tool
compile the example with `--debug`, three jobs and a folder kept in the target of the tests.

The API depends on `ravenrs-flows` without its feature `canvas`, and so should a plugin: then no
drawing (`ravenrs-graphics`) goes into its tree.
