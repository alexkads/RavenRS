# Report server

`ravenrs-report-server` runs reports without a UI. It stores layouts with versions, renders them over HTTP, and sends scheduled reports by email or to a folder as PDF, XLSX or CSV. It uses the same engine and exporters as the designer, so a report comes out the same everywhere.

## Run it

Put the configuration in a file:

```json
{
  "listen": "127.0.0.1:8090",
  "dataDir": "/var/lib/ravenrs-reports",
  "outputDir": "/srv/reports",
  "smtp": {
    "host": "smtp.example.com",
    "from": "Reports <reports@example.com>",
    "usernameEnv": "SMTP_USER",
    "passwordEnv": "SMTP_PASSWORD"
  },
  "dataHosts": [
    { "host": "api.example.com", "headersEnv": { "Authorization": "ERP_API_KEY" } }
  ]
}
```

Secrets stay in the environment: the file holds only the names of the variables.

```bash
export RAVENRS_SERVER_TOKEN="$(openssl rand -hex 32)"
ravenrs-report-server --config server.json
```

Try it with a sample layout:

```bash
cargo run -p ravenrs-report-server --example sample_layout > invoice.json
```

```bash
curl -X PUT -H "Authorization: Bearer $RAVENRS_SERVER_TOKEN" --data-binary @invoice.json http://127.0.0.1:8090/layouts/invoice
```

```bash
curl -X POST -H "Authorization: Bearer $RAVENRS_SERVER_TOKEN" -d '{"name":"invoice","format":"pdf"}' -o invoice.pdf http://127.0.0.1:8090/render
```

| Setting | Default | What it does |
|---|---|---|
| `listen` | `127.0.0.1:8090` | Address and port |
| `dataDir` | required | Where layouts, jobs, history and kept files live |
| `apiTokenEnv` | `RAVENRS_SERVER_TOKEN` | The variable with the API token, of at least 16 characters; the server does not start without it |
| `outputDir` | none | The folder under which `folder` deliveries write |
| `smtp` | none | The mail server: `host`, `port`, `security` (`starttls`, `tls` or `none`), `from`, `usernameEnv`, `passwordEnv` |
| `dataHosts` | none | The hosts `url` data sources may read, with headers taken from the environment |
| `maxConcurrentRenders` | 4 | Reports rendered at the same time |
| `retryDelaySeconds` | 60 | Wait before the first retry; it doubles on each one |
| `misfireGraceMinutes` | 60 | How late a run may still happen after the server was down |
| `historyLimit` | 1000 | Runs kept per job |
| `maxBodyMegabytes` | 32 | Largest request body |

## Jobs

`PUT /jobs/{id}` with:

```json
{
  "layout": "sales",
  "schedule": "0 8 * * MON",
  "timezone": "America/Recife",
  "parameters": {
    "Region": "Sul",
    "From": { "expression": "AddMonths(Today(), -1)" }
  },
  "formats": ["pdf", "xlsx"],
  "deliveries": [
    {
      "type": "email",
      "to": ["Ana <ana@example.com>", "bruno@example.com"],
      "separately": true,
      "subject": "{report} - {date}",
      "body": "Attached is the report for {date}."
    },
    { "type": "folder", "path": "sales/weekly" }
  ],
  "skipIfEmpty": true,
  "retries": 2,
  "retentionDays": 30,
  "enabled": true
}
```

* **`schedule`** is five cron fields: minute, hour, day of month, month, day of week. It takes `*`, lists, ranges, steps and names (`MON`, `JAN`). When both day fields are restricted, a day matches if either does.
* **`timezone`** applies to the schedule and to `Today()`. On a daylight saving change, a local time that clocks skip runs shifted forward, and one that clocks repeat runs once.
* **`parameters`** are fixed values, or expressions in the [criteria language](../crates/ravenrs-criteria) worked out at each run.
* **`separately`** sends one message per recipient, so they do not see each other.
* **`retries`**: a retry renders nothing again and sends only to who did not receive the files.
* **`retentionDays`** keeps each run's files on the server for that long; `0` keeps none. The history stays.

A job always renders the latest version of its layout. The history records which version it was.

| Run status | Meaning |
|---|---|
| `success` | Everything was delivered |
| `skippedEmpty` | The report had no rows and `skipIfEmpty` is on |
| `partial` | Some deliveries worked and others did not, after the retries |
| `failed` | Nothing was delivered |
| `missed` | Too late to run after the server was down, or the previous run was still going |

## API

Every route but `/health` needs `Authorization: Bearer <token>`.

| Route | What it does |
|---|---|
| `GET /health` | Whether the server is up |
| `POST /render` | Renders now. Body: `name` (with an optional `version`) or `layout`; `format`; optional `parameters`, `data`, `timezone`. Answers with the file; `x-report-warnings` counts problems in the report |
| `GET /layouts` | Layout names |
| `PUT /layouts/{name}` | Saves a layout; identical content adds no version. Answers with the version |
| `GET /layouts/{name}` | The latest version, or `?version=n` |
| `GET /layouts/{name}/versions` | Versions and sizes |
| `DELETE /layouts/{name}` | Deletes all versions; refused while a job uses it |
| `GET /jobs`, `GET /jobs/{id}` | Jobs with their next run |
| `PUT /jobs/{id}` | Saves a job after checking it. Answers with its next run |
| `DELETE /jobs/{id}` | Deletes a job with its history and files |
| `POST /jobs/{id}/run` | Runs it now, also when disabled |
| `GET /jobs/{id}/runs` | History, newest first; `?limit=n`, at most 500 |
| `GET /jobs/{id}/runs/{run}` | One run: who received what, errors, report warnings |
| `GET /jobs/{id}/runs/{run}/documents/{file}` | A kept file |

## Data

* **`json`** sources come from the layout, and `POST /render` can send `data` instead.
* **`url`** sources are fetched at each run. `{?Name}` inserts a parameter value, encoded. Only hosts in `dataHosts` can be reached and redirects are not followed, so a layout cannot make the server read its internal network.
* **`sql`** sources are not supported yet, and a run that uses one fails saying so.

## Operating it

* **One identity.** The API token gives full access: there are no users or per-report permissions yet. Anyone with it can read every layout and history, and send email through your server to any address.
* **Keep it private.** It listens on the loopback interface by default and speaks plain HTTP. To reach it from elsewhere, put it behind a proxy with TLS.
* **One instance.** Runs are not coordinated across processes, so two servers on one `dataDir` would send everything twice.
* **At most once.** The next run is saved before a run starts, so a crash cannot repeat a delivery. The run under way when the server stops is not resumed and leaves no history entry.
* **Back up `dataDir`.** It holds everything, as plain files.

## Not built yet

SQL data sources, email with the report inline as HTML, delivery of a link instead of the file, opening and saving layouts on the server from the designer, and users with permissions.
