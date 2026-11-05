//! The only way to the network: [`TransporteHttp`], which any test or platform can replace.

use std::fmt;

use crate::provedor::ErroProvedor;

/// An HTTP method used by the adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metodo {
    /// GET.
    Get,
    /// POST.
    Post,
    /// PATCH.
    Patch,
}

/// A request, ready to go.
#[derive(Clone, PartialEq, Eq)]
pub struct RequisicaoHttp {
    /// The method.
    pub metodo: Metodo,
    /// The full address.
    pub url: String,
    /// The headers, in order.
    pub cabecalhos: Vec<(String, String)>,
    /// The body, when there is one.
    pub corpo: Option<Vec<u8>>,
}

impl RequisicaoHttp {
    /// The value of a header, ignoring case in its name.
    #[must_use]
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos.iter().find(|(k, _)| k.eq_ignore_ascii_case(nome)).map(|(_, v)| v.as_str())
    }

    /// The body as JSON, for tests and handlers.
    #[must_use]
    pub fn json(&self) -> Option<serde_json::Value> {
        self.corpo.as_deref().and_then(|corpo| serde_json::from_slice(corpo).ok())
    }
}

/// `Debug` hides `Authorization`, which holds a secret.
impl fmt::Debug for RequisicaoHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nomes: Vec<&str> = self.cabecalhos.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("RequisicaoHttp")
            .field("metodo", &self.metodo)
            .field("url", &self.url)
            .field("cabecalhos", &nomes)
            .field("corpo_bytes", &self.corpo.as_ref().map(Vec::len))
            .finish()
    }
}

/// A response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RespostaHttp {
    /// The status code.
    pub status: u16,
    /// The body.
    pub corpo: Vec<u8>,
}

/// Why a request got no HTTP response. The caller cannot know if it arrived.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroTransporte {
    /// The time limit ran out.
    Tempo,
    /// The network failed.
    Rede(String),
    /// This transport refuses this address.
    Recusado(String),
}

impl fmt::Display for ErroTransporte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tempo => f.write_str("o tempo acabou"),
            Self::Rede(motivo) => write!(f, "falha de rede: {motivo}"),
            Self::Recusado(motivo) => write!(f, "endereço recusado: {motivo}"),
        }
    }
}

impl std::error::Error for ErroTransporte {}

impl From<ErroTransporte> for ErroProvedor {
    fn from(erro: ErroTransporte) -> Self {
        match erro {
            ErroTransporte::Tempo => Self::Tempo,
            outra => Self::Indisponivel(outra.to_string()),
        }
    }
}

/// Sends a request and returns any HTTP response, also an error status.
pub trait TransporteHttp {
    /// Sends it.
    fn enviar(&self, requisicao: &RequisicaoHttp) -> Result<RespostaHttp, ErroTransporte>;
}

impl<T: TransporteHttp + ?Sized> TransporteHttp for &T {
    fn enviar(&self, requisicao: &RequisicaoHttp) -> Result<RespostaHttp, ErroTransporte> {
        (**self).enviar(requisicao)
    }
}

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use nativo::TransporteUreq;

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
mod nativo {
    use std::io::Read;
    use std::time::Duration;

    use ureq::Agent;
    use ureq::config::Config;

    use super::{ErroTransporte, Metodo, RequisicaoHttp, RespostaHttp, TransporteHttp};

    /// The largest response read, far above any documented answer.
    const LIMITE_RESPOSTA: u64 = 4 * 1024 * 1024;

    /// The real network through `ureq`. It follows no redirect, and only local addresses may use
    /// plain HTTP. It has never reached a real provider.
    #[derive(Debug)]
    pub struct TransporteUreq {
        agente: Agent,
    }

    impl TransporteUreq {
        /// A transport with this time limit per request.
        #[must_use]
        pub fn new(limite: Duration) -> Self {
            let config = Config::builder()
                .timeout_global(Some(limite))
                .max_redirects(0)
                .http_status_as_error(false)
                .build();
            Self { agente: Agent::new_with_config(config) }
        }
    }

    fn permitido(url: &str) -> bool {
        url.starts_with("https://")
            || url.starts_with("http://localhost:")
            || url.starts_with("http://127.0.0.1:")
    }

    fn erro_ureq(erro: &ureq::Error) -> ErroTransporte {
        if matches!(erro, ureq::Error::Timeout(_)) {
            ErroTransporte::Tempo
        } else {
            ErroTransporte::Rede(erro.to_string())
        }
    }

    impl TransporteHttp for TransporteUreq {
        fn enviar(&self, requisicao: &RequisicaoHttp) -> Result<RespostaHttp, ErroTransporte> {
            if !permitido(&requisicao.url) {
                return Err(ErroTransporte::Recusado("só https, ou http local".into()));
            }
            let corpo = requisicao.corpo.as_deref().unwrap_or_default();
            let cabecalhos = requisicao.cabecalhos.iter();
            let resposta = match requisicao.metodo {
                Metodo::Get => {
                    let mut pedido = self.agente.get(&requisicao.url);
                    for (nome, valor) in cabecalhos {
                        pedido = pedido.header(nome, valor);
                    }
                    pedido.call()
                }
                Metodo::Post => {
                    let mut pedido = self.agente.post(&requisicao.url);
                    for (nome, valor) in cabecalhos {
                        pedido = pedido.header(nome, valor);
                    }
                    pedido.send(corpo)
                }
                Metodo::Patch => {
                    let mut pedido = self.agente.patch(&requisicao.url);
                    for (nome, valor) in cabecalhos {
                        pedido = pedido.header(nome, valor);
                    }
                    pedido.send(corpo)
                }
            }
            .map_err(|erro| erro_ureq(&erro))?;
            let status = resposta.status().as_u16();
            let mut bytes = Vec::new();
            resposta
                .into_body()
                .into_reader()
                .take(LIMITE_RESPOSTA)
                .read_to_end(&mut bytes)
                .map_err(|erro| ErroTransporte::Rede(erro.to_string()))?;
            Ok(RespostaHttp { status, corpo: bytes })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn endereco_em_claro_fora_da_maquina_e_recusado_sem_tocar_a_rede() {
            let transporte = TransporteUreq::new(Duration::from_secs(1));
            let requisicao = RequisicaoHttp {
                metodo: Metodo::Get,
                url: "http://exemplo.invalid/".into(),
                cabecalhos: Vec::new(),
                corpo: None,
            };
            assert!(matches!(transporte.enviar(&requisicao), Err(ErroTransporte::Recusado(_))));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_nao_mostra_o_token() {
        let requisicao = RequisicaoHttp {
            metodo: Metodo::Post,
            url: "https://exemplo.invalid".into(),
            cabecalhos: vec![("Authorization".into(), "Bearer SEGREDO".into())],
            corpo: None,
        };
        assert!(!format!("{requisicao:?}").contains("SEGREDO"));
        assert_eq!(requisicao.cabecalho("authorization"), Some("Bearer SEGREDO"));
    }
}
