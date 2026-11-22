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

`LiveChannels` with `ApiOptions::live` streams events at `GET /api/live/{topic}`, as Server-Sent Events.

An application whose routes change while it runs, as the plugins of a CI switched on and off, answers them with `ApiService::fallback`: it is asked first, with the request and a way to know who sent it (`Caller`: name, roles, whether an administrator), checked as every route checks a token, and it answers or leaves the request to the API. `ApiResponse::raw` answers a body that is not JSON with its own `Content-Type`, as the text of `/metrics`. A path a service of another company calls with no token, as a webhook, is opened with `ApiOptions::open("/hooks/")`, and its headers come with `ApiOptions::pass_header("x-hub-")`; everything else with no token is still turned away before its body is read. See the [application framework guide](../../docs/APP-FRAMEWORK.md) for the routes. It speaks plain HTTP: put it behind something that does TLS before leaving the machine.
