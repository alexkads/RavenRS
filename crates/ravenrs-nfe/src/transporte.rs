//! The only way out of the crate to the network: [`Transport`].

use std::fmt;

/// An HTTP request, ready to go. The method is always POST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    /// The full address.
    pub url: String,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// The value of a header, ignoring case in its name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

/// An HTTP response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    /// The status code.
    pub status: u16,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// Why a request got no HTTP response.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportError {
    /// The time limit ran out.
    Timeout,
    /// The connection could not be made or was cut.
    Conexao(String),
    /// TLS failed: untrusted server, refused client certificate, or similar.
    Tls(String),
    /// The server answered with a redirect, which is never followed.
    Redirecionamento(String),
    /// The sandbox got a request no handler answers, with `on_unhandled(Error)`.
    NaoTratado {
        /// The address.
        url: String,
        /// The action found in the `Content-Type`, if any.
        acao: Option<String>,
    },
    /// Anything else.
    Outro(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "tempo de resposta esgotado"),
            Self::Conexao(motivo) => write!(f, "falha de conexão: {motivo}"),
            Self::Tls(motivo) => write!(f, "falha de TLS: {motivo}"),
            Self::Redirecionamento(destino) => write!(f, "redirecionamento recusado: {destino}"),
            Self::NaoTratado { url, acao } => {
                write!(f, "sandbox sem handler para {url} (ação: {})", acao.as_deref().unwrap_or("-"))
            }
            Self::Outro(motivo) => write!(f, "{motivo}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// The only exit of the crate to the network.
///
/// The emitter builds the same bytes and calls the same addresses whatever the transport is: the
/// sandbox replaces this and nothing else. A send can take seconds, so the application calls the
/// emitter from a separate thread.
pub trait Transport: Send + Sync {
    /// Sends a request and waits for the response.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;

    /// Whether this transport keeps everything local. `Ambiente::Sandbox` only accepts one that does.
    fn intercepta(&self) -> bool {
        false
    }
}
