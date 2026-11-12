//! The only way out of the crate to the network: [`Transport`].

use std::fmt;

/// The HTTP method. SOAP services only use POST; the REST APIs of the NFS-e also read with GET and
/// ask whether something exists with HEAD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Metodo {
    /// Sends a document or an event. The default.
    #[default]
    Post,
    /// Reads something.
    Get,
    /// Asks whether something exists, with no body in the answer.
    Head,
}

impl Metodo {
    /// The name, as it goes on the wire.
    #[must_use]
    pub const fn nome(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Get => "GET",
            Self::Head => "HEAD",
        }
    }
}

/// An HTTP request, ready to go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    /// The method.
    pub metodo: Metodo,
    /// The full address.
    pub url: String,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// A POST with these headers and body.
    #[must_use]
    pub fn post(url: impl Into<String>, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self { metodo: Metodo::Post, url: url.into(), headers, body }
    }

    /// A GET, with no body.
    #[must_use]
    pub fn get(url: impl Into<String>, headers: Vec<(String, String)>) -> Self {
        Self { metodo: Metodo::Get, url: url.into(), headers, body: Vec::new() }
    }

    /// A HEAD, with no body.
    #[must_use]
    pub fn head(url: impl Into<String>, headers: Vec<(String, String)>) -> Self {
        Self { metodo: Metodo::Head, url: url.into(), headers, body: Vec::new() }
    }

    /// The path of the address, without host and query: `/nfse/123` of `https://h/nfse/123?x=1`.
    #[must_use]
    pub fn caminho(&self) -> &str {
        let sem_esquema = self.url.split_once("://").map_or(self.url.as_str(), |(_, resto)| resto);
        let caminho = sem_esquema.find('/').map_or("/", |inicio| &sem_esquema[inicio..]);
        caminho.split(['?', '#']).next().unwrap_or("/")
    }

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
    /// The sandbox got a request no handler answers, with `on_unhandled(Error)`. For a REST service,
    /// `acao` is the method and the path.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_leaves_out_host_and_query() {
        let pedido = HttpRequest::get("https://sefin.nfse.gov.br/SefinNacional/nfse/123?x=1", Vec::new());
        assert_eq!(pedido.caminho(), "/SefinNacional/nfse/123");
        assert_eq!(HttpRequest::head("https://h", Vec::new()).caminho(), "/");
        assert_eq!(pedido.metodo.nome(), "GET");
    }
}
