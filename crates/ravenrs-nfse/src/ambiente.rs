//! Where a DPS goes, and the acknowledgement that this is a beta.

/// The namespace of the DPS, the NFS-e and their events, from the XSD v1.01.
pub const NS_NFSE: &str = "http://www.sped.fazenda.gov.br/nfse";
/// The version of the layout written.
pub const VERSAO: &str = "1.01";
/// `verAplic` of what this crate writes.
pub const VER_APLIC: &str = "RAVENRS-NFSE-BETA";

/// Proof that the code calling the emitter knows it is a beta. Its field is private, so it can
/// only exist in code that says it out loud.
///
/// ```compile_fail,E0423
/// let ack = ravenrs_nfse::BetaAck(());
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BetaAck(());

impl BetaAck {
    /// Acknowledges that this is a beta, not homologated and not for real fiscal documents.
    #[must_use]
    pub const fn i_understand_this_is_experimental() -> Self {
        Self(())
    }
}

/// Where the DPS goes. Production does not exist in this enum on purpose: `tpAmb` is always 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ambiente {
    /// Nothing leaves the computer: the transport must intercept, like [`crate::sandbox::SefinSandbox`].
    Sandbox,
    /// The restricted production of the Sefin Nacional, the test environment of the national
    /// standard. Needs a real ICP-Brasil certificate; the laboratory's is refused.
    ProducaoRestrita,
}

impl Ambiente {
    /// The base of the Sefin Nacional API.
    ///
    /// The sources disagree on the prefix (`/SefinNacional`, `/API/SefinNacional`); the routes are
    /// matched from their end, so only the real one needs to be right. Not confirmed on a real send.
    #[must_use]
    pub const fn base_sefin(self) -> &'static str {
        match self {
            Self::Sandbox => "https://sefin.sandbox.ravenrs.invalid/SefinNacional",
            Self::ProducaoRestrita => "https://sefin.producaorestrita.nfse.gov.br/SefinNacional",
        }
    }

    /// The base of the parameters of the cities, at the ADN. Not confirmed on a real send.
    #[must_use]
    pub const fn base_parametros(self) -> &'static str {
        match self {
            Self::Sandbox => "https://adn.sandbox.ravenrs.invalid/parametrizacao",
            Self::ProducaoRestrita => "https://adn.producaorestrita.nfse.gov.br/parametrizacao",
        }
    }
}
