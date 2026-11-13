//! How a request reaches PostgREST. The space only builds requests and reads answers; [`Http`]
//! carries them, so a test can stand in for the server and lose an answer on purpose.

use std::fmt;
use std::time::Duration;

/// A request on its way.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpRequest {
    /// `GET` or `POST`.
    pub method: &'static str,
    /// The whole address, with its query.
    pub url: String,
    /// Headers, the token among them.
    pub headers: Vec<(String, String)>,
    /// The JSON sent, if any.
    pub body: Option<String>,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The headers carry the token and the key: only their names are shown.
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &names)
            .finish_non_exhaustive()
    }
}

impl HttpRequest {
    /// The value of a header, by its name in any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(named, _)| named.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What came back.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpResponse {
    /// The status.
    pub status: u16,
    /// The body.
    pub body: String,
}

impl HttpResponse {
    /// With this status and body.
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self { status, body: body.into() }
    }
}

/// Why no answer came.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HttpError {
    /// The request never left: no name, no connection, a bad address. Nothing happened.
    NotSent(String),
    /// It left, and no answer came back. It may have happened.
    NoAnswer(String),
}

/// Carries a request and waits for the answer.
pub trait Http {
    /// Sends it.
    ///
    /// # Errors
    ///
    /// When no answer came, saying whether the request ever left.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}

/// HTTP and HTTPS through `ureq`, with rustls.
#[derive(Clone)]
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl fmt::Debug for UreqHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UreqHttp").finish_non_exhaustive()
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new(Duration::from_secs(10), Duration::from_secs(60))
    }
}

impl UreqHttp {
    /// Waiting this long to connect, and that long for the whole answer.
    pub fn new(connect: Duration, whole: Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(connect))
            .timeout_global(Some(whole))
            .build()
            .new_agent();
        Self { agent }
    }
}

/// Whether a failure came before the request left.
fn before_sending(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::Http(_)
        | ureq::Error::InvalidProxyUrl => true,
        ureq::Error::Timeout(timeout) => {
            matches!(timeout, ureq::Timeout::Resolve | ureq::Timeout::Connect)
        }
        ureq::Error::Io(io) => {
            matches!(io.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::AddrNotAvailable)
        }
        _ => false,
    }
}

impl Http for UreqHttp {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut built = ureq::http::Request::builder().method(request.method).uri(&request.url);
        for (name, value) in &request.headers {
            built = built.header(name, value);
        }
        let asked = built
            .body(request.body.clone().unwrap_or_default())
            .map_err(|error| HttpError::NotSent(error.to_string()))?;
        let mut response = self.agent.run(asked).map_err(|error| {
            if before_sending(&error) {
                HttpError::NotSent(error.to_string())
            } else {
                HttpError::NoAnswer(error.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let body =
            response.body_mut().read_to_string().map_err(|error| HttpError::NoAnswer(error.to_string()))?;
        Ok(HttpResponse::new(status, body))
    }
}
