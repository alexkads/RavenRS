# ravenrs-app-testkit

Synthetic data for testing a reusable `ravenrs-app` module, as [docs/APP-CONTRACTS.md](../../docs/APP-CONTRACTS.md) describes it.

- `Fixture`: organizations T1 and T2, each with its own records, in memory or in SQLite files. They share local keys on purpose (a person 1 in each), so mixing one with the other shows. T1 has units U1 and U2; T2 has U1.
- `Organization::space_as`: a unit of work as one person, every read and write checked by their roles. `context_of` gives the security context to share; `add_user` puts in place someone who signs in with a role kept as a record, and `turn_off` takes them out on their next reload.
- `roles`: an example policy (administrator, registrar of a unit, worker limited to some entities, and `events_worker`, which every organization keeps as a record). It belongs to the tests, not to RavenRS.
- `Clock`: time that moves only when a test says so; `date_time` gives it to `Dispatcher::run_at`.
- `FakeSink`: a destination for events that accepts, refuses, is out of reach, loses its answer or cannot tell what it received, as the test asks, keeping one effect per key and counting every send.

- `contract`: the battery every storage adapter of `ObjectSpace` runs with one call, `contract::run(&adapter)`. An `Adapter` makes fresh storage for each case and a second way into it, as another program would have (`Store::again`). The cases cover values of every kind, filters, order, pages, `only`, calculated fields, all or nothing, a record deleted under an update, states, rules and watchers, the audit trail in the same commit, permissions, and two programs at once. What an adapter does not guarantee is declared in `Capabilities`, and the case checks it fails the declared way; the table in [docs/APP-PARITY.md](../../docs/APP-PARITY.md) is checked against them. `MemoryDatabase`, `ravenrs-app-sqlite` (memory and file), `ravenrs-app-postgres` and `ravenrs-app-postgrest` run it.

Opening `Fixture::sqlite` on the same folder again finds what was kept, which is how a test restarts an application.

Not published yet (`publish = false`): it is used by `ravenrs-app-requests`, the examples in `examples/contracts` and the tests of the storage adapters, which take it by path so it stays out of what they publish.
