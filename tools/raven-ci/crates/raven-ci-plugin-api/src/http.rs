//! Addresses a plugin adds to the HTTP API: the [`Route`] point.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A request that came to a route of a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    /// `GET`, `POST`.
    pub method: String,
    /// The path, from `/`.
    pub path: String,
    /// What came after `?`, by name.
    pub query: BTreeMap<String, String>,
    /// The headers, by name in lower case.
    pub headers: BTreeMap<String, String>,
    /// The body.
    pub body: Vec<u8>,
    /// Who it came from, when it was signed in or carried a key.
    pub actor: Option<String>,
}

/// What a route answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    /// `200`, `404`.
    pub status: u16,
    /// The type of the body.
    pub content_type: String,
    /// The body.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// `value` as JSON, with `status`.
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self { status, content_type: "application/json".to_owned(), body: value.to_string().into_bytes() }
    }

    /// `text`, with `status`.
    pub fn text(status: u16, text: impl Into<String>) -> Self {
        Self { status, content_type: "text/plain; charset=utf-8".to_owned(), body: text.into().into_bytes() }
    }
}

/// What answers a route.
pub type Handler = Arc<dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync>;

/// An address a plugin adds to the HTTP API: a webhook, an endpoint to ask for builds.
///
/// **Its role.** The API of the server has only what the core needs (health, sign in); the rest
/// comes from plugins.
///
/// **How it works.** While the plugin is switched on, the server answers `method` on `path` with
/// the handler; switched off, the address answers 404 again. The core refuses a route whose
/// method and path another plugin took, naming both. Unless [`public`](Self::public), only a
/// person signed in or a key reaches it, and [`HttpRequest::actor`] says who.
///
/// **How to write one.** Keep the path under the plugin's name (`/api/github/hook`). A webhook is
/// public and checks its own signature.
#[derive(Clone)]
pub struct Route {
    /// `GET`, `POST`.
    pub method: String,
    /// The path, from `/`; a part between braces (`{job}`) takes any value.
    pub path: String,
    /// Reached with no one signed in, as a webhook is.
    pub public: bool,
    /// What answers it.
    pub handler: Handler,
}

impl Route {
    /// `method` on `path`, answered by `handler`, for people signed in.
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        handler: impl Fn(&HttpRequest) -> HttpResponse + Send + Sync + 'static,
    ) -> Self {
        Self { method: method.into(), path: path.into(), public: false, handler: Arc::new(handler) }
    }

    /// Reached by anyone, as a webhook is.
    pub fn public(mut self) -> Self {
        self.public = true;
        self
    }

    /// Whether it answers `method` on `path`: the same method, and the same parts, where a part
    /// between braces takes any value.
    pub fn matches(&self, method: &str, path: &str) -> bool {
        if !self.method.eq_ignore_ascii_case(method) {
            return false;
        }
        let ours: Vec<&str> = self.path.trim_matches('/').split('/').collect();
        let theirs: Vec<&str> = path.trim_matches('/').split('/').collect();
        ours.len() == theirs.len()
            && ours.iter().zip(&theirs).all(|(a, b)| (a.starts_with('{') && a.ends_with('}')) || a == b)
    }
}

impl fmt::Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_between_braces_takes_any_value() {
        let route = Route::new("POST", "/api/jobs/{job}/builds", |_| HttpResponse::text(200, "ok"));
        assert!(route.matches("post", "/api/jobs/site/builds"));
        assert!(!route.matches("GET", "/api/jobs/site/builds"));
        assert!(!route.matches("POST", "/api/jobs/site"));
        assert_eq!((route.handler)(&HttpRequest::default()).body, b"ok");
    }
}
