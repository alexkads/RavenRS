# Running the server

This page is for whoever keeps a Raven CI server running: how to install it, where its data is,
who may do what, how its secrets are kept, how to watch it, how to update it, and what to look at
when something goes wrong.

## Installing

| How | When | What you get |
|---|---|---|
| **RPM** (Fedora): `sudo dnf install <url of the release>` | a Linux server | the service `raven-ci` as the user `raven-ci`, data in `/var/lib/raven-ci`, `127.0.0.1:8090`, the panel at `/painel/`, `MemoryMax=256M`. See "Install it" in the [README](../README.md) |
| **Container**: `tools/raven-ci/pacote/docker` | trying a version on a machine that already runs something else | the same RPM in a clean Fedora, its data in a volume, `127.0.0.1:8090`, 256 MB |
| **Installer**: `install.cmd`, then `raven-ci servico instalar` | macOS, Windows, or a Linux without the RPM | the binary of the user, and the server as a service of systemd, launchd or the Task Scheduler |
| **From the repository**: `cargo run -p raven-ci -- servir` | developing Raven CI | everything in `~/.raven-ci` unless `--dados` says |

The server listens on one address (`--endereco`), on the loopback unless told otherwise. To reach
it from other machines, put it behind what already publishes services there (a reverse proxy, a
`tailscale serve`) rather than opening the port: the server speaks plain HTTP.

## The folder of data

Everything of a server is in one folder: `--dados`, else `RAVEN_CI_HOME`, else the user's home when
it is itself a folder of data (it has a `raven-ci.sqlite`, as `/var/lib/raven-ci` for the user
`raven-ci` of the RPM), else `~/.raven-ci`. A command that makes a folder of data that was not there
says so on its error output, `pasta de dados nova em …`, so a command pointed at the wrong folder
shows at once (#952):

| Path | What it holds | Kept? |
|---|---|---|
| `raven-ci.sqlite` | jobs, builds, logs, the queue, the bytes of the attachments, the audit | yes |
| `usuarios.sqlite` | people, their roles, their keys | yes |
| `cofre/`, `cofre.chave` | the sealed credentials and the key that opens them | yes, the key apart (see below) |
| `dados-dos-plugins/<plugin>/` | each plugin's own files and its `plugin.sqlite` | yes, except `cargo/targets`, which compiles again |
| `plugins/` | the external plugins installed | yes |
| `espacos/` | the folder of each build running | no: each one is removed at the end of its build |
| `logs/` | the output of the service, when launchd or the installer keeps it | as long as you want |

### Backup and restore

```bash
raven-ci --dados /var/lib/raven-ci backup criar                 # with the server running
raven-ci --dados /var/lib/raven-ci backup ver PASTA              # what it holds, checked
sudo systemctl stop raven-ci
sudo -u raven-ci raven-ci --dados /var/lib/raven-ci backup restaurar PASTA
sudo systemctl start raven-ci
```

- **A backup** (`backup criar [PASTA]`, in `backups/raven-ci-<data>` of the folder of data unless
  said) copies each database with `VACUUM INTO`, consistent even with a build writing, checked and
  stamped with the version; the vault, the people and the data of the plugins as files. It leaves
  out what comes back by itself: `espacos/`, the repositories of `git`, the locks of `lock`, the
  journals of SQLite. `raven-ci-backup.json` lists each file with its size.
- **The key of the vault** goes in only with `--com-chave`: a backup with it opens every
  credential. Without it, keep `cofre.chave` apart (a password manager); restoring a backup with no
  key on a new machine needs that file, or `RAVEN_CI_CHAVE`.
- **Restoring** checks the backup first, is refused while a server uses the folder, moves what is
  there to `antes-da-restauracao-<quando>/`, and only then puts the backup in place. A backup with
  no key leaves the key that is there.
- **What stays out** however it is asked: a `cofre.chave` deeper than the root (it opens another
  folder of data), and a folder `.raven-ci` inside the folder of data, which a command run with no
  `--dados` once made (#952). The key of the root goes only with `--com-chave`.

## Who may do what

| Role | May |
|---|---|
| `Administradores` | everything: jobs, configuration, plugins, credentials, people |
| `Executores` | read, and ask for builds, cancel and repeat them |
| `Leitores` | read jobs, builds, logs, tests, artifacts and metrics |
| `Agentes` | only the routes of agents; given to the keys of agents, never to people |

- The first person made is an administrator, whatever role is asked:
  `echo 'senha' | raven-ci --dados PASTA usuario criar NOME [--papel Executores]`.
- A person signs in with `POST /api/auth/token` and gets a token for a time. A program (a script,
  `raven-ci rodar` in another machine) uses a key made with `POST /api/auth/keys`, kept in
  `RAVEN_CI_TOKEN`.
- An agent has a key of its own, made by `raven-ci agentes token NOME --rotulos …`, and nothing
  else.
- Every route under `/api/` asks for a token, except `/health`. Only paths under `/hooks/` are
  reached with no sign-in, and only the plugins that need it put routes there: the webhook of
  GitHub (which checks its signature before reading anything) and `/hooks/metrics` when its
  settings open it.

## Secrets

Credentials are kept in the vault of the core (`ravenrs-secrets`): each one sealed by name, its
fields never written in clear to the disk, never shown by the API or the panel (they list the
name, the kind, what it is for and which jobs and plugins name it), and hidden in every log once
a plugin or a step reads them.

| Kind | Fields | For |
|---|---|---|
| `token` | `token` | a token of GitHub, a PAT of Azure DevOps, a feed |
| `user-password` | `user`, `password` | a login; a step takes each field apart (`nome:user`) |
| `ssh-key` | `private_key`, `passphrase` | a deploy key |
| `arquivo` | `content` | Jenkins's "Secret file": a kubeconfig, an `.npmrc`, a certificate |

Only administrators keep them, from any of three places, and each change goes to the audit
(`pessoa:<nome>`) with who made it:

- **The panel**: the page Credenciais, with the form that makes one, rotates one (the same name
  again) or revokes one.
- **The API**: `POST /api/ci/credentials` with `{"name", "kind", "description", "fields"}` (the
  same name again rotates its value), `DELETE /api/ci/credentials?name=` and
  `GET /api/ci/credentials`.
- **The command line**, here or on another machine: the fields come on the standard input, never
  in the words of the command.

  ```bash
  echo '{"token": "…"}' | raven-ci credencial criar npm-appogi --tipo token --descricao "feed npm" --servidor http://ci:8090
  raven-ci credencial listar --servidor http://ci:8090
  raven-ci credencial apagar npm-appogi --servidor http://ci:8090
  ```

  With `--servidor` (or `RAVEN_CI_SERVIDOR`) they use the API with the key in `RAVEN_CI_TOKEN`;
  without, they open the folder of data of this machine, as before.

A pipeline uses one by name: a plugin through its settings (`"credential": "github"`), and a
command through `credentials` ([the pipeline](pipeline.md#credentials-in-a-step)).

The key that opens the vault is, in this order:

1. the variable `RAVEN_CI_CHAVE` (64 hexadecimal digits), when it is set;
2. the file `cofre.chave` in the folder of data, made the first time with a random key, readable
   only by its owner. A key file that others may read is refused on Linux and macOS.

Keep the key apart from a copy of the data: whoever has both has every secret. On another machine,
the vault opens only with the same key.

## From the terminal, on a server that runs

With `--servidor URL` (or `RAVEN_CI_SERVIDOR`) and an administrator's key in `RAVEN_CI_TOKEN`,
what is administered goes to the server running, through its API, and nothing opens a folder of
data on this machine (#929):

| Command | What it does there |
|---|---|
| `raven-ci agentes listar` | the agents, with the state of each connection the server knows |
| `raven-ci agentes token NOME [--rotulos a,b] [--executores N]` | a token used once for a new agent |
| `raven-ci agentes remover NOME` | the agent's key stops working |
| `raven-ci agentes modelos`, `modelo NOME --lancador ID …`, `modelo-apagar NOME` | the templates that bring agents up |
| `raven-ci plugins listar`, `ligar NOME`, `desligar NOME` | switched at once, with no restart |
| `echo SENHA \| raven-ci usuario criar NOME [--papel P]` | a person, the password from the input |
| `raven-ci credencial criar\|listar\|apagar` | the vault ([Secrets](#secrets)) |

Without a server named, `agentes` reads the folder of data, and says that only the server running
knows whether an agent is connected.

## Configuration as code

The jobs, folders, plugins and their settings of a server can live in a `raven-ci.toml` kept in a
repository, applied with `raven-ci aplicar` or `servir --config` (see the [README](../README.md)).
The settings of a plugin go under its name:

```toml
url = "https://ci.empresa.dev"

[plugins.email]
settings = { host = "smtp.empresa.dev", port = 587, from = "CI <ci@empresa.dev>", credential = "smtp" }

[plugins.metrics]
settings = { open = true }

[plugins.workspace-cleanup]
settings = { min_free_gb = 10 }
```

`url` is the address people reach the server at. With it, a notice (e-mail, `webhook-out`) and
the status of a commit on GitHub or Azure DevOps link to the page of the build on the panel:
`https://ci.empresa.dev/painel/#/build/apps%2Fsite/42`. The panel opens on that page, after
signing in when no one is; a build the retention took lands on the jobs, saying how many builds the
job keeps. On the desktop, `raven-ci-painel --abrir build/apps%2Fsite/42` or the whole address
pasted as an argument does the same (#969). Without `url`, there is no link.

## Agents in containers

`agent = { docker = {} }` needs the image of the agent (`pacote/docker/gerar-imagem-agente.sh`, on
the machine of the engine) and the user `raven-ci` able to reach the engine: the group `docker`,
which is worth root on the machine, or Podman with no root. See
[In a container](agents.md#in-a-container-docker).

## Explaining failures with local AI

The subpackage `raven-ci-explicador` brings the program and `raven-ci-explicador.socket`, which
starts one instance per request with `MemoryMax=6G` and no network, out of the server's cgroup.
Download a model once (`sudo -u raven-ci raven-ci-explicador baixar --pasta
/var/lib/raven-ci-explicador/modelos`) and switch the plugin `explain` on. See
[Explaining a failure with local AI](ai.md#explaining-a-failure-with-local-ai).

## What runs on the machine

The service of the RPM runs as `raven-ci`, with a private `/tmp` and `CPUWeight=50`/`IOWeight=50`
instead of a `Nice`, so the builds see the machine's priority. Agents of another user need the
drop-in `/usr/share/raven-ci/agentes-de-outro-usuario.conf` (`NoNewPrivileges=false`,
`PrivateTmp=false`). Nothing a build starts outlives it: see
[What a build leaves behind](agents.md#what-a-build-leaves-behind), with what runs in which user,
`/tmp` and priority, and what stays after a restart.

## Watching it

- **Health**: `GET /health` answers `{"status":"ok"}` with no sign-in. The container's healthcheck
  and a service manager can use it.
- **Metrics**: the plugin `metrics` gives the queue, the builds by result, the last build of each
  job, the agents, the plugins and the memory, in the Prometheus format, at `/api/ci/metrics`, or
  at `/hooks/metrics` with `open = true` for a Prometheus with no token.
  `raven_ci_builds` counts the builds still kept, so it goes down when the retention lets some
  go; for how many builds ran in a day use the counters, which never go down, not even after a
  restart: `increase(raven_ci_builds_total{status="failed"}[1d])`, or the mean time of a job's
  builds with `rate(raven_ci_build_seconds_total[1h]) / rate(raven_ci_builds_total[1h])`. They
  count from `raven_ci_totals_since_seconds`, the first start of a server with them (#970).
- **Notices**: the plugins `email` and `webhook-out` tell of a build that failed, of one that passed
  again, or of every build, by what each job says in `notify`.
- **The panel**: Jobs says what waits and why, Fila e agentes shows the last two hours of each
  executor and agent, a build that waits for a plugin switched off offers "Ligar o plugin", and
  Histórico lists every build (`GET /api/ci/history`).
- **Logs**: the server writes to its output (an agent that would not come up, a plugin that did
  not start, a notice that failed, each line of the agents it brought up); with the RPM,
  `journalctl -u raven-ci`; in the container, `docker logs raven-ci-teste`. The log of each build is in the panel and at
  `/api/ci/log`.
- **Memory**: about 14 MiB idle, 36 MiB with an agent and a build (see "Feito com RavenRS" in the
  README); `tools/raven-ci/scripts/medir-memoria.sh` measures it again on any machine.

## Updating

- **RPM**: `sudo dnf install <url of the new release>`. The service starts again with the new
  binary, and the data stays.
- **Service of the installer**: the installer with `RCI_SERVICO=1` replaces the binary and starts
  the service again.
- **Container**: build the image again with the new RPM and `docker compose up -d --build`; the
  volume keeps the data.

An agent started by hand (a service of a user, as `raven-ci-agente-oficina`) keeps the old
binary loaded after the package is replaced: restart it too (`systemctl --user restart
raven-ci-agente-oficina`), or what runs on it (fetching artifacts, the steps) stays as it was.

The database moves to a new version by itself when the server opens it. Going back to an older
version after that is not guaranteed: `raven-ci backup criar` before updating.

## When something goes wrong

| What you see | What it means | What to do |
|---|---|---|
| a build refused with `falta o plugin X` | the pipeline names a step, a scm or a publisher of a plugin switched off or missing | `raven-ci plugins listar`; switch it on (`plugins ligar X`), or install the external one in `plugins/` |
| a plugin `recusado: …` in `plugins listar` | it does not fit: a version it does not accept, a dependency missing or switched off | the reason is in the state; `raven-ci plugins info X` shows its dependencies and permissions |
| a build waits in the queue for ever | it asks for `agent = { label = … }` and no agent with that label is connected, nor a template that brings one | `raven-ci agentes listar` lists the agents and their labels; start one, or add a template ([Build agents](agents.md)) |
| a build refused for room on the disk | `workspace-cleanup` found less free space than `min_free_gb` | free the disk, or lower `min_free_gb` |
| `o job X não aceita pedidos pela API` | the job has no `{"id": "api"}` in its `triggers` | add it to the job |
| the GitHub webhook answers 401 | the signature does not match the secret of any job watching it | the secret of the webhook at GitHub and the credential the job names must be the same text |
| a notice that never arrives | `notify` names a notifier switched off, or its settings are wrong | the line `aviso <id>: …` in the log of the build says which |
| `nem docker nem podman no PATH` | a step of the plugin `docker` on a machine without an engine | install one, or say the engine in the plugin's settings |
| `falta o cargo neste agente` | a step `cargo` where `cargo` is not in the `PATH` | install Rust for the user that runs the agent, or say `program` in the settings of `cargo` |
| the server does not start: the address is in use | another program listens there | `--endereco` with another port, or stop the other one |
| the vault does not open | the key is not the one the vault was sealed with, or its file can be read by others | the variable `RAVEN_CI_CHAVE` or the file `cofre.chave` of the machine that made it, mode `600` |

For a bug report: `raven-ci --versao`, the output of `raven-ci plugins listar --format json`, the
log of the build and of the server around the time, and what the pipeline says.
