//! HTTP around the service. Storage and permissions belong to one thread, so that thread does all
//! the work, one request at a time, and the rest here only carries requests to it and answers back.

use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value as Json, json};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use crate::{ApiRequest, ApiResponse, ApiService};

/// The largest request body.
const BODY_LIMIT: usize = 4 * 1024 * 1024;
/// The largest body of someone who has not signed in yet.
const SIGN_IN_LIMIT: usize = 64 * 1024;
/// How long a body has to arrive.
const BODY_WITHIN: Duration = Duration::from_secs(30);
/// What the thread that does the work has for a stack. An expression as long as a filter can be
/// nests its parts as deep as it is long, and running out of stack would end the whole process.
const WORKER_STACK: usize = 32 * 1024 * 1024;
/// How many requests wait for their turn before more are turned away.
const WAITING: usize = 256;
/// How long a request waits for its answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);

type Asked = (ApiRequest, oneshot::Sender<ApiResponse>);

#[derive(Clone)]
struct Worker {
    asking: mpsc::Sender<Asked>,
    origins: Arc<Vec<String>>,
}

/// How the service meets the outside.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ApiOptions {
    origins: Vec<String>,
}

impl ApiOptions {
    /// Nothing but what comes from the same place.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lets pages from this origin, such as `https://example.com`, call the service from a browser.
    /// There is no way to allow every origin: each one that may call is named.
    pub fn allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.origins.push(origin.into());
        self
    }
}

fn answer(status: u16, body: &Json) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response =
        (status, [(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(header::WWW_AUTHENTICATE, header::HeaderValue::from_static("Bearer"));
    }
    response
}

fn busy() -> Response {
    answer(503, &json!({ "error": "busy", "message": "The service cannot answer now. Try again soon." }))
}

fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = |index: usize| bytes.get(index).and_then(|b| (*b as char).to_digit(16));
        match (byte, hex(at + 1), hex(at + 2)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                at += 3;
            }
            (b'+', ..) => {
                out.push(b' ');
                at += 1;
            }
            _ => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_of(text: &str) -> BTreeMap<String, String> {
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decoded(name), decoded(value))
        })
        .collect()
}

/// Handles a request, saying to a browser whether the page that asked may read the answer.
async fn handle(State(worker): State<Worker>, request: Request<Body>) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .filter(|origin| {
            origin.to_str().is_ok_and(|origin| worker.origins.iter().any(|known| known == origin))
        })
        .cloned();
    let preflight = request.method() == Method::OPTIONS;
    let mut response =
        if preflight { StatusCode::NO_CONTENT.into_response() } else { answered(&worker, request).await };
    if let Some(origin) = origin {
        let headers = response.headers_mut();
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        if preflight {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE"),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("Authorization, Content-Type"),
            );
        }
    }
    response
}

async fn answered(worker: &Worker, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    if parts.uri.path() == "/health" {
        return if worker.asking.is_closed() { busy() } else { answer(200, &json!({ "status": "ok" })) };
    }
    let bearer = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned);
    // Nobody who has not signed in gets to make it read a large body.
    let signing_in = parts.uri.path() == "/api/auth/token";
    if bearer.is_none() && !signing_in {
        return answer(401, &json!({ "error": "unauthorized", "message": "A valid token is required." }));
    }
    let limit = if signing_in { SIGN_IN_LIMIT } else { BODY_LIMIT };
    let Ok(Ok(body)) = tokio::time::timeout(BODY_WITHIN, to_bytes(body, limit)).await else {
        return answer(413, &json!({ "error": "too_large", "message": "The request is too large." }));
    };
    let client = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0.ip().to_string());
    let path: Vec<String> =
        parts.uri.path().split('/').filter(|part| !part.is_empty()).map(decoded).collect();
    let asked = ApiRequest {
        method: parts.method.as_str().to_owned(),
        path,
        query: query_of(parts.uri.query().unwrap_or("")),
        bearer,
        client,
        body: body.to_vec(),
    };
    let (reply, answered) = oneshot::channel();
    if worker.asking.try_send((asked, reply)).is_err() {
        return busy();
    }
    match tokio::time::timeout(ANSWER_WITHIN, answered).await {
        Ok(Ok(response)) => answer(response.status, &response.body),
        _ => busy(),
    }
}

/// The routes, over a service made by `make` on a thread of its own, since what it holds belongs to
/// one thread. It fails if the service cannot be made.
pub fn router(make: impl FnOnce() -> Result<ApiService, String> + Send + 'static) -> io::Result<Router> {
    router_with(ApiOptions::new(), make)
}

/// The same, with these options.
pub fn router_with(
    options: ApiOptions,
    make: impl FnOnce() -> Result<ApiService, String> + Send + 'static,
) -> io::Result<Router> {
    let (sender, mut waiting) = mpsc::channel::<Asked>(WAITING);
    let (made, started) = std::sync::mpsc::channel::<Result<(), String>>();
    let worker = std::thread::Builder::new().name("ravenrs-app-server".to_owned()).stack_size(WORKER_STACK);
    worker.spawn(move || {
        let mut service = match make() {
            Ok(service) => {
                let _ = made.send(Ok(()));
                service
            }
            Err(why) => {
                let _ = made.send(Err(why));
                return;
            }
        };
        while let Some((request, reply)) = waiting.blocking_recv() {
            // One request that goes wrong is that request's problem, and not everyone's after it.
            let answered = catch_unwind(AssertUnwindSafe(|| service.handle(request)));
            let response = answered.unwrap_or_else(|_| {
                tracing::error!("a request panicked");
                ApiResponse {
                    status: 500,
                    body: json!({ "error": "internal", "message": "Something went wrong on the server." }),
                }
            });
            let _ = reply.send(response);
        }
    })?;
    started
        .recv()
        .map_err(|_| io::Error::other("the service stopped while starting"))?
        .map_err(io::Error::other)?;
    Ok(Router::new()
        .fallback(handle)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(Worker { asking: sender, origins: Arc::new(options.origins) }))
}

/// Serves the API on this listener until it fails.
pub async fn serve(
    listener: TcpListener,
    make: impl FnOnce() -> Result<ApiService, String> + Send + 'static,
) -> io::Result<()> {
    axum::serve(listener, router(make)?.into_make_service_with_connect_info::<SocketAddr>()).await
}

/// The same, with these options.
pub async fn serve_with(
    listener: TcpListener,
    options: ApiOptions,
    make: impl FnOnce() -> Result<ApiService, String> + Send + 'static,
) -> io::Result<()> {
    axum::serve(listener, router_with(options, make)?.into_make_service_with_connect_info::<SocketAddr>())
        .await
}
