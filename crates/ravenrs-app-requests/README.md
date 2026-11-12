# ravenrs-app-requests

An optional `ravenrs-app` package of requests decided step by step.

- The application gives the steps, the role that decides each one and any further rule (`policy`). With no rule, only the role of the step decides.
- A decision carries an id: sending it again changes nothing. A decision taken on an old version of the request is refused as a conflict.
- Each decision writes an event in the **same commit**, so a refused decision leaves no event.
- `Dispatcher` hands events to an `EventSink`, keyed by organization, with limited retries. An event sent with no answer is checked with `status` before it is sent again.
- `testing::FakeSink` answers as a test says (accept, refuse, down, answer lost) with no network.

Nothing here belongs to a sector: [`examples/contracts`](../../examples/contracts) installs it in an association and in a course center with different steps, roles and rules. Scenarios C06, C07 and C19 of [docs/APP-CONTRACTS.md](../../docs/APP-CONTRACTS.md).

Limits for now: the version check reads the stored request before writing, so two decisions in the very same instant are not caught (#241); the event lives in this package until the framework has its own (#254); people who decide also need permission to create events.

Not published yet (`publish = false`).
