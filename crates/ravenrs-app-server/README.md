# ravenrs-app-server

A web API for applications made with `ravenrs-app`: the same entities, rules, permissions, states and audit trail as the screens, over HTTP, for clients that do not use RavenRS.

```rust,no_run
use ravenrs_app_server::{ApiService, serve};

# async fn run() -> std::io::Result<()> {
let listener = tokio::net::TcpListener::bind("127.0.0.1:8090").await?;
serve(listener, || {
    // Runs on each thread that serves the requests, since storage belongs to one thread.
    // `serve_with(listener, ApiOptions::new().workers(4), ..)` answers four at once.
    # let (space, roles) = todo!();
    ApiService::new(space, roles).map_err(|error| error.to_string())
})
.await
# }
```

`LiveChannels` with `ApiOptions::live` streams events at `GET /api/live/{topic}`, as Server-Sent Events. See the [application framework guide](../../docs/APP-FRAMEWORK.md) for the routes. It speaks plain HTTP: put it behind something that does TLS before leaving the machine.
