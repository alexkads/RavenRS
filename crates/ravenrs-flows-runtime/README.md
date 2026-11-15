# ravenrs-flows-runtime

Durable runs of [`ravenrs-flows`](../ravenrs-flows) flows, kept in SQLite.

`ravenrs-flows` runs a flow in memory, so a run that is cut short is lost. This crate keeps each run in a SQLite file that several processes may share. A run survives its worker dying and the app closing, picks up where it stopped without repeating what was done, and can wait hours for a time or an event.

## How a run goes

1. **Asked for.** `Runtime::enqueue(Request)` keeps the flow as it is now (later revisions do not change the run), the items it starts with, who asked and the mode, and puts the run in the queue.
2. **Taken.** A worker calls `Runtime::claim` (or `work`, which claims and runs). The run due first is given to it with a lease, in one `UPDATE … RETURNING` inside a write transaction. Two workers, in one process or several, never take the same run.
3. **Run, step by step.** Before each node runs, the store writes that it started. When its part is over, the store writes the part (status, inputs, outputs, error) as JSON and renews the lease.
4. **Left behind.** If the worker dies, its lease runs out and the run is due again. The next worker goes on from the checkpoint:
   - the nodes that were over keep their part and do not run again;
   - a node left *started* was running when the worker died. It is asked whether its call happened (`NodeKind::reconcile`). It runs again only if the answer is *no*. If no one can tell, it stays *uncertain*, so a payment is not made twice.
5. **Waiting.** A node may wait for a time (`delay`) or an event (`wait_event`). The run stops there, the worker lets it go, and it is due again when the time comes or `Runtime::signal(run, event, items)` says the event came.
6. **Ended.** The run's status (`Finished`, `Stopped`, `Cancelled` or `TimedOut`) and how it ended stay with it. `Runtime::get`, `runs_of` and `parts` read them back for the screens.

## The rest

- `Runtime::rerun(id, Revision::Original | Revision::Current(flow))` runs the same input again. The new run keeps where it came from.
- `Runtime::prune(&Retention { older_than_ms, statuses, keep_last })` forgets ended runs by age, status and how many of each flow stay, with their steps, events and files.
- `Runtime::put_blob` keeps a file a run's items carry, apart from their JSON, and `Runtime::blob` reads it back.
- `Runtime::clock(ManualClock)` puts the queue, the leases and the waits on a test clock.

## Tables

`runs` (one row per run), `steps` (one row per node that started), `loops_done`, `events` and `blobs`, made by `Runtime::open` when the file has none. The file uses the write-ahead log and waits up to 10 seconds for a lock.

## Limits

- A loop is one step: a run stopped inside a loop goes round it again from the start, so a node with an effect inside a loop's body should reconcile.
- A worker whose lease was taken by another stops writing at its next step, but the node it is running is not stopped.
- SQLite only. A queue shared by several machines needs another store.
- `rusqlite` needs Rust 1.89, one above the workspace.

License: MIT OR Apache-2.0.
