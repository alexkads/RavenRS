//! The client of the HTTP API of a server of Raven CI that the remote commands ([`crate::remote`])
//! and the MCP server ([`crate::mcp`]) speak through: where the server is, the key, and what a
//! failure of HTTP means as a [`Refused`].

use std::time::Duration;

use serde_json::Value;

use crate::saida::{Exit, Refused};

/// Where the server is unless told.
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";

/// A server of Raven CI, and how to reach it.
pub(crate) struct Server {
    pub(crate) base: String,
    token: Option<String>,
    agent: ureq::Agent,
}

/// `text` as a value of a query: what is not a letter, a digit or `-._~` goes as `%XX`.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl Server {
    pub(crate) fn new(server: Option<String>) -> Self {
        let base = server
            .or_else(|| std::env::var("RAVEN_CI_SERVIDOR").ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_SERVER.to_owned())
            .trim_end_matches('/')
            .to_owned();
        // The key: RAVEN_CI_TOKEN, else the one `ia instalar` saved (#941).
        let token = std::env::var("RAVEN_CI_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty())
            .or_else(crate::chave::saved_key);
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        Self { base, token, agent }
    }

    /// `GET` of `path` with `query`: the status, the type and the bytes of the answer.
    pub(crate) fn fetch(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<(u16, String, Vec<u8>), Refused> {
        let mut url = format!("{}{path}", self.base);
        for (i, (name, value)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(&format!("{name}={}", encode(value)));
        }
        let mut request = self.agent.get(&url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.call().map_err(|e| self.unreachable(&e))?;
        self.read(answer)
    }

    fn unreachable(&self, e: &ureq::Error) -> Refused {
        Refused::with(
            Exit::Unreachable,
            format!(
                "o servidor em {} não respondeu ({e}); ele está de pé? (raven-ci servir, ou --servidor URL)",
                self.base
            ),
        )
    }

    fn read(&self, mut answer: ureq::http::Response<ureq::Body>) -> Result<(u16, String, Vec<u8>), Refused> {
        let status = answer.status().as_u16();
        let kind = answer
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let bytes = answer
            .body_mut()
            .with_config()
            .limit(u64::MAX)
            .read_to_vec()
            .map_err(|e| Refused::new(format!("a resposta de {}: {e}", self.base)))?;
        if status == 401 {
            let why = if self.token.is_some() {
                "o servidor não aceitou o RAVEN_CI_TOKEN; confira a chave"
            } else {
                "o servidor pede login: ponha uma chave em RAVEN_CI_TOKEN (crie com POST /api/auth/keys)"
            };
            return Err(Refused::with(Exit::Denied, why.to_owned()));
        }
        if !(200..300).contains(&status) {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|value| value.get("message").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(200).collect());
            let exit = if status == 403 { Exit::Denied } else { Exit::Refused };
            return Err(Refused::with(exit, format!("o servidor respondeu {status}: {message}")));
        }
        Ok((status, kind, bytes))
    }

    pub(crate) fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Refused> {
        let (_, _, bytes) = self.fetch(path, query)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Refused::new(format!("a resposta de {path} não é JSON: {e}")))
    }

    pub(crate) fn post(&self, path: &str, body: &Value) -> Result<Value, Refused> {
        let mut request =
            self.agent.post(&format!("{}{path}", self.base)).header("Content-Type", "application/json");
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.send(body.to_string()).map_err(|e| self.unreachable(&e))?;
        let (_, _, bytes) = self.read(answer)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Refused::new(format!("a resposta de {path} não é JSON: {e}")))
    }

    pub(crate) fn delete(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Refused> {
        let mut url = format!("{}{path}", self.base);
        for (i, (name, value)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(&format!("{name}={}", encode(value)));
        }
        let mut request = self.agent.delete(&url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.call().map_err(|e| self.unreachable(&e))?;
        let (_, _, bytes) = self.read(answer)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Refused::new(format!("a resposta de {path} não é JSON: {e}")))
    }

    /// The build `number` of `job`, with its stages and attachments.
    pub(crate) fn build(&self, job: &str, number: u64) -> Result<Value, Refused> {
        self.get("/api/ci/build", &[("job", job.to_owned()), ("number", number.to_string())])
    }

    /// The lines of the log of the build after `after`.
    pub(crate) fn log(&self, job: &str, number: u64, after: u64) -> Result<Vec<Value>, Refused> {
        let lines = self.get(
            "/api/ci/log",
            &[("job", job.to_owned()), ("number", number.to_string()), ("after", after.to_string())],
        )?;
        Ok(lines.as_array().cloned().unwrap_or_default())
    }
}
