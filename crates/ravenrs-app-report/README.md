# ravenrs-app-report

Reports of the lists of a RavenRS business application. Add `ReportModule` and every list gets an
action that lays out its records as a report, through the same engine as `ravenrs-report`, and shows
it in the document viewer, from where it prints and exports.

```rust,ignore
let application = Application::builder("Contas")
    .module(SystemModule::new())
    .module(ReportModule::new("Relatório"))
    .entity(account)
    .build()?;
```

See [`docs/APP-FRAMEWORK.md`](../../docs/APP-FRAMEWORK.md).
