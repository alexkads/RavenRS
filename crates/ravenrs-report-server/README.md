# ravenrs-report-server

Report server for [RavenRS Reporting](https://github.com/alexkads/RavenRS): stores report layouts with versions, renders them over HTTP and delivers scheduled reports by email or to a folder as PDF, XLSX or CSV.

```bash
export RAVENRS_SERVER_TOKEN="$(openssl rand -hex 32)"
cargo install ravenrs-report-server
ravenrs-report-server --config server.json
```

See [docs/REPORT-SERVER.md](https://github.com/alexkads/RavenRS/blob/master/docs/REPORT-SERVER.md) for the configuration, the API and the job format.

Licensed under MIT or Apache-2.0.
