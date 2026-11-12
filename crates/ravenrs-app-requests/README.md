# ravenrs-app-requests

An optional `ravenrs-app` package of requests decided step by step.

- The application gives the steps, the role that decides each one and any further rule: an expression of `ravenrs-criteria` on the step (`Step::when("[requester] <> ?CurrentUserName")`) or a Rust `policy`. With no rule, only the role of the step decides.
- Each step opens a task with its role and, with `Step::due_in`, a deadline. The generated list of tasks shows the open ones, and `task_reader` on each role makes it that role's inbox; `tasks_for` answers the same in code.
- A member lends a role with `delegate` until a moment. `delegated_roles` gives the borrower what to read when signing in, and every decision checks the end again.
- A decision carries an id: sending it again changes nothing. A decision taken on an old version of the request is refused as a conflict.
- Each decision writes an event in the **same commit**, so a refused decision leaves no event.
- The event is the framework's (`ObjectSpaceExt::emit`), so an application adds `OutboxModule` next to this package. `Dispatcher`, also from `ravenrs-app`, hands events to an `EventSink`, keyed by organization. A temporary failure is tried again up to a limit; a refusal or a missing capability fails at once. An event sent with no answer is checked with `status` before it is sent again, and stays to be reconciled when the sink cannot tell.
- `ravenrs_app_testkit::FakeSink` answers as a test says (accept, refuse, down, answer lost, no status) with no network.

Nothing here belongs to a sector: [`examples/contracts`](../../examples/contracts) installs it in an association and in a course center with different steps, roles and rules. Scenarios C06, C07 and C19 of [docs/APP-CONTRACTS.md](../../docs/APP-CONTRACTS.md).

Limits for now: the version check reads the stored request before writing, so two decisions in the very same instant are not caught (#241); people who decide also need permission to create events in `OutboxEvent`.

Not published yet (`publish = false`).
