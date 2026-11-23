# Build agents

A build agent runs builds away from the server: on another machine, as another user, inside a
container. The server keeps what has to be in one place (the queue, the log, the attachments, the
status of each commit) and the agent does the work. This page explains what an agent does, how it
talks to the server, what happens when it falls, and how to set one up.

## What runs where

| The server | The agent |
|---|---|
| the queue and its leases (`ravenrs-flows-runtime`), the numbers of builds | the build's folder, in the agent's own folder |
| the log: every line goes through the server's log, which hides the server's secrets | the hooks of the plugins it switches on (`workspace-cleanup`, `lock`) |
| the attachments (`Ci::attach`) | fetching the code (`git`) |
| the status of the commit and the notices: the GitHub token never leaves the server | the steps, from the checkpoint of the queue |
| the retention, cancelling, the time limit | the post steps, the publishers (it reads the files and sends the bytes), `BuildScope::at_end` |

The life of a build is the same code on both sides: `Ci::run_order` with a `BuildSite`. The
server's site runs the steps in its queue; the agent's sends everything to the server.

The agent is one program, `raven-ci-agent`. It brings compiled in the plugins of steps, of
fetching code, of hooks and the publishers (`shell`, `git`, `lock`, `workspace-cleanup`,
`artifacts`, `junit`, `cargo`, `docker`, `kubernetes`), plus the external plugins of its folder
`plugins/`, all switched off. Each build switches on only the plugins it needs, with
the settings the server has for them, and they go off again when no build needs them. Built for
release, an agent connected and idle holds 15.6 MiB, and 16.3 MiB at the highest of a build
(resident memory, measured by `tools/raven-ci/scripts/medir-memoria.sh` on 22/11/2026, macOS arm64;
see "Feito com RavenRS" in the [README](../README.md)). The agent of Jenkins holds 250 MB. Its goal
is 15 MB, and #916 looks at the difference.

## How it talks to the server

The agent connects out: nothing is open where it runs. It speaks HTTP to the routes under
`/api/ci/agentes/` and hears the server over Server-Sent Events.

| Route | What it does |
|---|---|
| `POST entrar` | the only one with no key: spends a token used once and gives a key `rvk_…` |
| `GET /api/live/agente-<name>` | the doorbell: `trabalho` (there may be a build for it) and `cancelar`; no data of a build goes here |
| `POST oi` | every 5 s: labels, executors free, the builds it runs, its memory and limits. The server renews their leases and answers which to cancel and which to forget |
| `POST pegar` | the next build for it: one of its own it lost when it fell, else the one due first its labels allow. It comes with the job's settings, the plugins it needs and the run's `Lease` (the checkpoint) |
| `POST passos` | the steps it ran, in order: `Runtime::record`, which renews the lease and says whether it still holds the run |
| `POST log` | lines, with the agent's session and the place of the first one; the server drops what it has |
| `POST codigo` | the commit fetched: the commit's status goes pending, the notices hear the start |
| `POST anexo` | an attachment in parts of 2 MB, raw bytes |
| `POST desfecho` | how the steps ended: `Runtime::finish` |
| `POST fim` | post steps, publishers and hooks are done: status, notices, retention |
| `POST segredo` | a secret, only while it runs a build; every read goes to the audit |

`GET /api/ci/agents` lists the agents for anyone signed in, and the topic `agentes` of the live
channel tells how each one is as it changes: events `agent` with `{name, state, labels, busy}`,
`state` being `conectado`, `sumido` or `removido`.

### What a launcher gives the agent

The contract is `raven_ci_plugin_api::AgentArgs`, the same for every launcher (local, SSH,
container, pod): the words `--servidor URL --nome NOME --rotulos a,b --executores N --pasta DIR`
(`AgentArgs::to_args`) and the token used once in `RAVEN_CI_AGENT_TOKEN` (`AgentArgs::env`), never
on a command line. A person starting an agent by hand may say `--token T`; `agent-remote` says
`--token -`, and the agent reads it from its input. The agent's own words (its limits) come after
those of the contract, and the agent takes them apart before it reads the contract, which stays
strict: a word mistyped is refused. Limits are the agent's own business; in a container or a pod,
the container holds them. The token is needed only the first time
(`AgentArgs::token` is optional): an agent that keeps its key comes back with it, and one with
neither a token nor a key refuses to start, saying so. It connects by itself, and never leaves by itself: the server calls
`LaunchedAgent::stop`.

## How a job picks its agent

```toml
[pipeline]
agent = { label = "linux" }   # or "any", { docker = {} }, { docker = { image = "minha-imagem" } }
```

- With a label, the build leaves the queue only for an agent with that label. The server's own
  executors never take it.
- With `"any"`, or no `agent`, any agent takes it, or an executor of the server. `raven-ci servir
  --executores 0` leaves every build to the agents.
- When no agent connected has the label, the server looks for a **template** with the label and
  asks its launcher to bring an agent up, with a token used once. An agent a template brought up
  that stays idle past the template's time is taken down.
- `{ docker = {…} }` and `{ kubernetes = {…} }` name a launcher themselves: it brings up an agent
  for that build only, with the label `build:<job>#<number>`, taken down when the build ends,
  with its key revoked and its name out of the list.
- An agent brought up for a build that dies before coming in says why in the build's log (its
  exit code and last lines): `o agente docker-3-ab12cd não subiu (tentativa 1 de 3): o contêiner
  saiu com o código 1: … GLIBC_2.44 not found`. After 3 in a row (`tentativas` in the launcher's
  settings) the build ends as failed with that reason, instead of waiting in the queue.

```text
raven-ci agentes modelo local --lancador agent-local --rotulos local,linux --executores 2 --ocioso 10
raven-ci agentes modelos
```

## How an agent gets in

1. A **token used once**: 32 random bytes, kept only as their SHA-256, for 10 minutes, for one
   name. A launcher gets one in `LaunchRequest::token`; for an agent installed by hand,
   `raven-ci agentes token linux-1 --rotulos linux` prints one, and so does "Adicionar agente" on the
   page Fila e agentes of the panel (`POST /api/ci/agents/token`, for administrators), with the
   command to run on the agent's machine.
2. `POST entrar` spends it, whatever comes of the try, and makes the agent a key of
   `ravenrs-app-server` for the user `agente:linux-1`, whose only role, `Agentes`
   (`raven_ci_plugin_api::ROLE_AGENT`), opens the routes of agents and nothing else (not the
   credentials, not the jobs to change).
3. The agent keeps the key in the file `chave` of its folder, readable only by its owner, and comes
   back with it after it falls. `raven-ci agentes remover linux-1` revokes it.

The first person of the server has to exist before any agent gets in: the first user made is the
administrator, and an agent never is.

```text
# on the server
raven-ci agentes token linux-1 --rotulos linux,x64 --executores 2
# on the agent's machine
RAVEN_CI_AGENT_TOKEN=rva_… raven-ci-agent --servidor https://ci.local \
    --nome linux-1 --rotulos linux,x64 --executores 2 --pasta /var/lib/raven-ci-agente
```

## When an agent falls

The builds of an agent are held in the queue by the worker `agente/<name>`. While it says hello,
the server renews their leases. When it goes quiet, the server goes on renewing them for the
**grace time** (`raven-ci servir --tolerancia-agente 300`, 5 minutes unless said), so no one else
takes them.

- **The network fell, the agent lives.** Its steps go on. What has to reach the server (steps,
  lines, the commit) waits in the build's outbox and goes again, in order, after a wait that grows
  from half a second to ten. The server drops the lines it has by the agent's session and the
  place of each one, so the log has every line once.
- **The process died and came back.** It comes back with its key, in a new session. `pegar` gives
  it the builds it held, with the checkpoint of the queue. It resumes each one with `Lease::run`:
  the steps that ended do not run again, and the step that was running is asked
  (`NodeKind::reconcile`) whether its call happened. A command (`sh`) says it did not, and runs
  again **from its start**, in the folder that kept what it wrote: a script has to bear running
  twice, as a stage that Jenkins resumes does. The note beside the folder (`<number>.estado`) says the code
  was fetched, so it is not fetched again. Lines the dead process had not sent are lost, and the
  log says the build was resumed.
- **It did not come back.** Past the grace time the server ends the build as interrupted
  (`BuildStatus::Interrupted`, not cancelled: no one cancelled it, and no step failed), with the
  status `error` on its commit and the notices, and writes in the log which agent went quiet and how long
  ago. When the agent comes back later, its hello is answered with the build to forget: it stops
  what it runs of it and removes the folder.

## The launchers

### On the same machine: `agent-local`

The plugin `agent-local` brings the agent up on the server's machine, as another user when the
template says one. The server can then run with `--executores 0`, and a build reads none of the
server's files.

```text
raven-ci agentes modelo local --lancador agent-local --rotulos local --executores 2 \
    --config '{"usuario": "ci", "nucleos": 4, "memoria": "8G", "disco": "50G"}'
raven-ci servir --executores 0
```

- `programa`: the agent's program; by default `raven-ci-agent` beside `raven-ci`.
- `pasta`: where each agent gets a folder of its own, `<pasta>/<name>`; by default
  `agentes/<name>` in the server's folder of data. With `usuario` it has to be said, a folder that
  user writes in (`/var/lib/raven-ci-agente`, say): the agent makes its folder there and keeps its
  key in it, and the server's folder of data is not that user's to write in.
- `usuario`: the user it runs as. As root, the server switches the user of the child itself. As
  anyone else, it runs `sudo -n -u <user> --`, which fails at once, saying so, when sudo has no
  rule for it. The token goes on the agent's input (`--token -`), not in its environment, which
  `sudo` would write to the journal of the system. A rule that lets the server's user run the
  agent as `ci`:

  ```text
  raven-ci ALL=(ci) NOPASSWD: /usr/bin/raven-ci-agent
  ```

  The service of the RPM runs with `NoNewPrivileges=true`, and with it `sudo` cannot switch the
  user: the launch is refused saying so. To have agents of another user, the RPM brings a drop-in
  that switches it off, and the private `/tmp` with it:

  ```bash
  sudo mkdir -p /etc/systemd/system/raven-ci.service.d
  sudo cp /usr/share/raven-ci/agentes-de-outro-usuario.conf /etc/systemd/system/raven-ci.service.d/
  sudo systemctl daemon-reload && sudo systemctl restart raven-ci
  ```

  ```ini
  [Service]
  NoNewPrivileges=false
  PrivateTmp=false
  ```

  Why `PrivateTmp=false`: see [What a build leaves behind](#what-a-build-leaves-behind).

  On Windows a user of its own is not done yet, and the launch is refused saying so.

### In a container: `docker`

`agent = { docker = {} }` runs the build in a container of its own, with the agent in it.

- **The image** is `raven-ci-agente:latest`, the agent's own: Fedora with the RPM of Raven CI, `git`
  (the agent fetches the code) and the user `agente`, not root. Make it on the machine of the
  docker the server uses, after `pacote/gerar-rpm.sh`:

  ```bash
  tools/raven-ci/pacote/docker/gerar-imagem-agente.sh
  ```

  A build that needs more tools makes an image `FROM raven-ci-agente:latest` and names it:
  `{ docker = { image = "raven-ci-agente-node:24" } }`. Any other image needs `agent_binary`, a
  `raven-ci-agent` of Linux on the engine's machine built for the C library of that image: the
  agent of the RPM is built on Fedora 45 (glibc 2.44) and does not run on Debian 12 (2.36).
- **The way to the server.** The RPM's server listens on `127.0.0.1:8090`, which a container on
  docker's own network does not reach. On Linux the launcher then gives the container the host's
  network (`--network host`); on Docker Desktop it tells the agent `host.docker.internal` in place
  of `127.0.0.1`. A server that listens on another address keeps docker's network; `network = "…"`
  or `--network` in `args` choose one.
- **The access to the engine.** The user of the service (`raven-ci`) has to reach the engine.
  With Docker, that is the group `docker` (`sudo usermod -aG docker raven-ci`, then restart the
  service), **and the group `docker` is worth root on the machine**: whoever runs builds may then
  mount `/` in a container. Podman with no root, as the user `raven-ci`, does not give that: install
  `podman` and the plugin finds it when there is no `docker`.
- **Settings:** `image`, `agent_binary`, `network`, `args` (more words for `docker run`),
  `executors`, `timeout_s` (taken down past it) and `tentativas`.
- **When it dies**, the container is still there for its last words (it does not run with
  `--rm`), and it is removed right after. No container with the label `raven-ci.agent` outlives
  its build, passed, failed or cancelled.

### On another machine: `agent-remote`

The plugin `agent-remote` brings the agent up on another machine through SSH, and the agent
connects back to the server: only the SSH that is there already is used.

```text
raven-ci agentes modelo mac --lancador agent-remote --rotulos mac \
    --config '{"host": "mac-mini.local", "usuario": "ci", "programa": "/opt/raven/raven-ci-agent"}'
```

- `host`, `usuario`, `porta`: where to.
- `programa`, `pasta`: the agent's program and folder there.
- `chave`: the name of a credential `ssh-key` of the vault; without it, the server's user's own
  keys. The key goes to a file readable only by its owner and is removed when the agent stops.
- `ssh`: the program; `ssh` by default, which Linux, macOS and Windows 10+ have.

## What a build leaves behind

Nothing a build starts outlives it, on the server or on an agent:

- **The processes of a step.** Each `sh` runs in a process group of its own. When the command
  ends, what it left running in the background (`sleep 600 &`, the server of `sccache` a
  `cargo build` started) is stopped at once, and the log names it: `o comando deixou 1
  processo(s) rodando (sccache); parados`. A step with `keep_daemons = true` keeps them, and the
  log says so. A daemon that leaves the group itself (`setsid`) is not reached; on Linux, a
  template with limits (A-04) holds the agent in a cgroup of its own.
- **The containers and pods** of `docker` and `kubernetes`, by their label.
- **The folder** of the build and the secret files of its steps (`<pasta>.segredos`), at its end.

What runs where, on the server of the RPM:

| What | Its user | Its mount namespace and `/tmp` | Its CPU priority |
|---|---|---|---|
| the server | `raven-ci` | the service's: a private `/tmp` (`PrivateTmp=true`) | `CPUWeight=50`, `IOWeight=50`: a smaller share of the machine, with the nice of the machine (0) |
| a build on the server's own executors | `raven-ci` | the service's | the server's |
| an agent of `agent-local` with no `usuario` | `raven-ci` | the service's | the server's |
| an agent of `agent-local` with `usuario` | that user, through `sudo` | the service's, unless the drop-in above switches `PrivateTmp` off | nice 0 |
| an agent of `agent-remote`, or one started by hand | its own | its machine's | its own |

The private `/tmp` matters when a tool of the machine keeps something there for everyone: the
server of `sccache` (port 4226) is one per user, shared by Jenkins, scripts and Raven CI. Started
by a build in the service's `/tmp`, it kept that `/tmp` after the service restarted, and every
user of `sccache` failed with `Failed to create temp dir` until `sccache --stop-server` (#931).
Now the step stops the server it started; and with the drop-in, a build sees the machine's `/tmp`,
as on a node of Jenkins.

The service used to say `Nice=10`, and every build inherited it: `nice -n 10` in a step gave 19,
and a test that checks its own priority failed where Jenkins passed. The weights protect the
machine by the cgroup instead, and a step sees the nice of the machine.

After `systemctl restart raven-ci`: the server and the agents of `agent-local` it started stop
(the agents are taken down with what they started); a build that was running is interrupted and
taken up again by the next agent of its label; nothing of it stays running.

## Limits

Each agent may have a limit of cores, memory and disk: `--nucleos 4 --memoria 8G --disco 50G`, or
`nucleos`, `memoria` and `disco` in a template's settings.

- **Cores.** The agent runs no more executors than its cores. Every step gets `RAVEN_CI_NUCLEOS`,
  `CARGO_BUILD_JOBS` and `MAKEFLAGS=-jN`. On Linux the agent's cgroup gets `cpu.max`.
- **Memory.** On Linux the agent moves itself into a cgroup of its own, `raven-ci-agente-<name>`,
  under the one it is in, with `memory.max`: the system holds the limit. That needs the cgroup
  delegated to the agent's user (a systemd service with `Delegate=yes`) or root; without it the
  agent says so and goes on. On every system a watch measures the agent's tree of processes every
  2 s (`ravenrs_metrics::tree_rss_bytes`) and cancels the build that runs when it passes, saying
  "passou do limite de memória".
- **Disk.** The watch measures the folder of each build and cancels the one that passes, saying how
  much it took. An agent whose disk has less room than the minimum of `workspace-cleanup` takes no
  build.

```ini
# /etc/systemd/system/raven-ci-agent.service
[Service]
User=ci
Delegate=yes
ExecStart=/usr/local/bin/raven-ci-agent --servidor https://ci.local --nome linux-1 --memoria 8G
```
