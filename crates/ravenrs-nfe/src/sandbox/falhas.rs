//! Deterministic failure injection: the same seed and the same requests give the same faults.

use std::time::Duration;

use super::tempo::Instante;
use crate::soap::Servico;

/// A failure to inject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Falha {
    /// The answer takes this long. With a controlled clock the time passes on it.
    Latencia(Duration),
    /// The connection falls after the server processed the message. The dangerous case: the
    /// document may be authorized and the sender does not know. Consult before sending again.
    QuedaAntesDaResposta,
    /// A SOAP 1.2 fault with this text; the message was not processed.
    SoapFault(String),
    /// An HTTP status with no SOAP in the body, such as 500 or 503; not processed.
    Http(u16),
    /// A TLS failure with this text; not processed.
    Tls(String),
    /// The server processed the message and the answer arrives cut, with this percentage of it.
    RespostaTruncada(u8),
    /// The answer is not XML; not processed.
    XmlInvalido,
    /// The service answers this `cStat` of paralysis, 108 or 109.
    Paralisado(u32),
}

impl Falha {
    /// A short description for the timeline.
    #[must_use]
    pub fn descricao(&self) -> String {
        match self {
            Self::Latencia(tempo) => format!("latência de {} ms", tempo.as_millis()),
            Self::QuedaAntesDaResposta => "queda antes da resposta".to_string(),
            Self::SoapFault(texto) => format!("SOAP Fault: {texto}"),
            Self::Http(status) => format!("HTTP {status}"),
            Self::Tls(texto) => format!("falha de TLS: {texto}"),
            Self::RespostaTruncada(parte) => format!("resposta truncada em {parte}%"),
            Self::XmlInvalido => "resposta sem XML válido".to_string(),
            Self::Paralisado(c_stat) => format!("serviço paralisado ({c_stat})"),
        }
    }
}

/// When a failure applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quando {
    /// Always.
    Sempre,
    /// For the next requests, this many times.
    Vezes(u32),
    /// Inside a window of the sandbox clock, start included and end excluded.
    Janela {
        /// Start.
        de: Instante,
        /// End.
        ate: Instante,
    },
    /// In this share of the requests, in thousandths, from a fixed seed.
    Intermitente {
        /// The seed.
        semente: u64,
        /// How many in a thousand fail.
        por_mil: u32,
    },
}

/// A failure, when it applies, and on which service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegraDeFalha {
    pub(super) falha: Falha,
    pub(super) quando: Quando,
    pub(super) servico: Option<Servico>,
    pub(super) contador: u64,
}

impl RegraDeFalha {
    /// A failure that applies to every request of every service.
    #[must_use]
    pub fn nova(falha: Falha) -> Self {
        Self { falha, quando: Quando::Sempre, servico: None, contador: 0 }
    }

    /// Only on this service.
    #[must_use]
    pub fn em(mut self, servico: Servico) -> Self {
        self.servico = Some(servico);
        self
    }

    /// Only this many times.
    #[must_use]
    pub fn vezes(mut self, vezes: u32) -> Self {
        self.quando = Quando::Vezes(vezes);
        self
    }

    /// Only inside this window of the sandbox clock.
    #[must_use]
    pub fn janela(mut self, de: Instante, ate: Instante) -> Self {
        self.quando = Quando::Janela { de, ate };
        self
    }

    /// In this share of the requests, in thousandths, repeatable by the seed.
    #[must_use]
    pub fn intermitente(mut self, semente: u64, por_mil: u32) -> Self {
        self.quando = Quando::Intermitente { semente, por_mil };
        self
    }

    /// Whether it applies to this request, advancing its own count.
    pub(super) fn aplica(&mut self, servico: Option<Servico>, agora: Option<Instante>) -> bool {
        if self.servico.is_some() && self.servico != servico {
            return false;
        }
        match &mut self.quando {
            Quando::Sempre => true,
            Quando::Vezes(restantes) => {
                if *restantes == 0 {
                    return false;
                }
                *restantes -= 1;
                true
            }
            Quando::Janela { de, ate } => agora.is_some_and(|momento| momento >= *de && momento < *ate),
            Quando::Intermitente { semente, por_mil } => {
                self.contador += 1;
                splitmix64(semente.wrapping_add(self.contador)) % 1000 < u64::from(*por_mil)
            }
        }
    }
}

/// A small mixing function; only needs to be fixed and well spread.
fn splitmix64(valor: u64) -> u64 {
    let mut z = valor.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_the_same_sequence() {
        let sequencia = |semente| {
            let mut regra = RegraDeFalha::nova(Falha::Http(503)).intermitente(semente, 300);
            (0..64).map(|_| regra.aplica(None, None)).collect::<Vec<bool>>()
        };
        assert_eq!(sequencia(7), sequencia(7));
        assert_ne!(sequencia(7), sequencia(8));
        let falhas = sequencia(7).iter().filter(|f| **f).count();
        assert!((5..40).contains(&falhas), "{falhas}");
    }

    #[test]
    fn count_window_and_service_limit_a_rule() {
        let mut vezes = RegraDeFalha::nova(Falha::XmlInvalido).vezes(1);
        assert!(vezes.aplica(None, None));
        assert!(!vezes.aplica(None, None));

        let mut janela = RegraDeFalha::nova(Falha::Paralisado(108)).janela(Instante(10), Instante(20));
        assert!(!janela.aplica(None, Some(Instante(9))));
        assert!(janela.aplica(None, Some(Instante(10))));
        assert!(!janela.aplica(None, Some(Instante(20))));
        assert!(!janela.aplica(None, None));

        let mut servico = RegraDeFalha::nova(Falha::Http(500)).em(Servico::Status);
        assert!(!servico.aplica(Some(Servico::Consulta), None));
        assert!(servico.aplica(Some(Servico::Status), None));
    }
}
