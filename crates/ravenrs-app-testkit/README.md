# ravenrs-app-testkit

Synthetic data for testing a reusable `ravenrs-app` module, as [docs/APP-CONTRACTS.md](../../docs/APP-CONTRACTS.md) describes it.

- `Fixture`: organizations T1 and T2, each with its own records, in memory or in SQLite files. They share local keys on purpose (a person 1 in each), so mixing one with the other shows. T1 has units U1 and U2; T2 has U1.
- `Organization::space_as`: a unit of work as one person, every read and write checked by their roles. `context_of` gives the security context to share; `add_user` puts in place someone who signs in with a role kept as a record, and `turn_off` takes them out on their next reload.
- `roles`: an example policy (administrator, registrar of a unit, worker limited to some entities, and `events_worker`, which every organization keeps as a record). It belongs to the tests, not to RavenRS.
- `Clock`: time that moves only when a test says so; `date_time` gives it to `Dispatcher::run_at`.
- `FakeSink`: a destination for events that accepts, refuses, is out of reach, loses its answer or cannot tell what it received, as the test asks, keeping one effect per key and counting every send.

Opening `Fixture::sqlite` on the same folder again finds what was kept, which is how a test restarts an application.

Not published yet (`publish = false`): it is used by `ravenrs-app-requests` and the examples in `examples/contracts`.
