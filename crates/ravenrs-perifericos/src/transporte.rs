//! How a catalog arrives. The address is configuration: this crate knows none. What arrives is not
//! trusted because of where it came from, only because of its signature.

use std::fmt;

/// Why a catalog could not be fetched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErroTransporte(pub String);

impl fmt::Display for ErroTransporte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErroTransporte {}

/// Somewhere a catalog comes from.
pub trait TransporteCatalogo {
    /// The catalog's file as it is.
    fn baixar(&self) -> Result<Vec<u8>, ErroTransporte>;
}

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use http::TransporteHttp;

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
mod http {
    use std::time::Duration;

    use super::{ErroTransporte, TransporteCatalogo};

    /// The most it reads, a little above what a catalog may be, so the size error is the catalog's.
    const LIMITE_DOWNLOAD: u64 = 5 * 1024 * 1024;

    /// A catalog at an address.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TransporteHttp {
        /// The address, from the application's configuration.
        pub url: String,
        /// How long to wait for everything.
        pub tempo_limite: Duration,
    }

    impl TransporteHttp {
        /// An address, waiting thirty seconds.
        pub fn novo(url: impl Into<String>) -> Self {
            Self { url: url.into(), tempo_limite: Duration::from_secs(30) }
        }
    }

    impl TransporteCatalogo for TransporteHttp {
        fn baixar(&self) -> Result<Vec<u8>, ErroTransporte> {
            let agente: ureq::Agent =
                ureq::Agent::config_builder().timeout_global(Some(self.tempo_limite)).build().into();
            let falha = |erro: ureq::Error| ErroTransporte(format!("{}: {erro}", self.url));
            let mut resposta = agente.get(&self.url).call().map_err(falha)?;
            resposta.body_mut().with_config().limit(LIMITE_DOWNLOAD).read_to_vec().map_err(falha)
        }
    }
}
