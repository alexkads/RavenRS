//! How a request gets to the server.

/// A request on its way.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RemoteRequest {
    /// `GET`, `POST` and so on.
    pub method: String,
    /// The whole address.
    pub url: String,
    /// The token to present.
    pub bearer: Option<String>,
    /// The JSON to send.
    pub body: Option<String>,
}

/// What came back.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RemoteResponse {
    /// The status.
    pub status: u16,
    /// The body.
    pub body: String,
}

impl RemoteResponse {
    /// With this status and body.
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self { status, body: body.into() }
    }
}

/// Carries requests. Calling `done` before returning makes commits wait for the server; calling it
/// later makes them go on at once, as they have to in a browser. Failing means nothing came back.
pub trait Transport {
    /// Sends it, and calls `done` with the answer.
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>);
}

/// HTTP: on the desktop it waits for the answer, and in the browser it does not.
#[derive(Clone, Copy, Debug, Default)]
pub struct HttpTransport;

#[cfg(not(target_arch = "wasm32"))]
impl Transport for HttpTransport {
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>) {
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
        let mut built = ureq::http::Request::builder().method(request.method.as_str()).uri(&request.url);
        if let Some(token) = &request.bearer {
            built = built.header("Authorization", format!("Bearer {token}"));
        }
        built = built.header("Content-Type", "application/json");
        let answered = built
            .body(request.body.unwrap_or_default())
            .map_err(|error| error.to_string())
            .and_then(|asked| agent.run(asked).map_err(|error| error.to_string()))
            .and_then(|mut response| {
                let status = response.status().as_u16();
                let body = response.body_mut().read_to_string().map_err(|error| error.to_string())?;
                Ok(RemoteResponse::new(status, body))
            });
        done(answered);
    }
}

#[cfg(target_arch = "wasm32")]
impl Transport for HttpTransport {
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>) {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        async fn fetch(request: RemoteRequest) -> Result<RemoteResponse, JsValue> {
            let init = web_sys::RequestInit::new();
            init.set_method(&request.method);
            if let Some(body) = &request.body {
                init.set_body(&JsValue::from_str(body));
            }
            let asked = web_sys::Request::new_with_str_and_init(&request.url, &init)?;
            asked.headers().set("Content-Type", "application/json")?;
            if let Some(token) = &request.bearer {
                asked.headers().set("Authorization", &format!("Bearer {token}"))?;
            }
            let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
            let response: web_sys::Response =
                JsFuture::from(window.fetch_with_request(&asked)).await?.dyn_into()?;
            let body = JsFuture::from(response.text()?).await?.as_string().unwrap_or_default();
            Ok(RemoteResponse::new(response.status(), body))
        }

        wasm_bindgen_futures::spawn_local(async move {
            done(fetch(request).await.map_err(|error| format!("{error:?}")));
        });
    }
}
