//! Where a document goes: the environment, the beta acknowledgement, the model and the state.

/// The explicit acknowledgement that this crate is an experimental proof of concept.
///
/// It has no public constructor other than the one with the long name, so an emitter
/// can only exist in code that says it out loud.
///
/// ```compile_fail,E0423
/// let ack = ravenrs_nfe::BetaAck(());
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

/// The environment of a document. Production does not exist in this enum on purpose.
///
/// ```compile_fail,E0599
/// let producao = ravenrs_nfe::Ambiente::Producao;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ambiente {
    /// Everything stays in the local `SefazSandbox`; nothing reaches the SEFAZ.
    Sandbox,
    /// The real homologation servers of the SEFAZ, with no fiscal value.
    Homologacao,
}

impl Ambiente {
    /// The value of `tpAmb`, which is `2` in every variant: no path here writes `1`.
    #[must_use]
    pub const fn tp_amb(self) -> &'static str {
        "2"
    }
}

/// Who produced a document, kept with it so a test note never passes for a real one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    /// Produced for the local sandbox.
    Sandbox,
    /// Produced for the homologation servers.
    Homologacao,
}

impl From<Ambiente> for Origem {
    fn from(ambiente: Ambiente) -> Self {
        match ambiente {
            Ambiente::Sandbox => Self::Sandbox,
            Ambiente::Homologacao => Self::Homologacao,
        }
    }
}

/// The document model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modelo {
    /// NF-e, model 55.
    Nfe55,
    /// NFC-e, model 65: what a cash register issues.
    Nfce65,
}

impl Modelo {
    /// The value of `mod`.
    #[must_use]
    pub const fn codigo(self) -> &'static str {
        match self {
            Self::Nfe55 => "55",
            Self::Nfce65 => "65",
        }
    }
}

/// A state with homologation addresses in the table of this POC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "each variant is the state's own abbreviation")]
pub enum Uf {
    SP,
    RS,
    PR,
    MG,
    SC,
    RJ,
    ES,
    DF,
}

impl Uf {
    /// The IBGE code of the state, `cUF` in the layout.
    #[must_use]
    pub const fn codigo_ibge(self) -> &'static str {
        match self {
            Self::SP => "35",
            Self::RS => "43",
            Self::PR => "41",
            Self::MG => "31",
            Self::SC => "42",
            Self::RJ => "33",
            Self::ES => "32",
            Self::DF => "53",
        }
    }

    /// The abbreviation, `UF` in the layout.
    #[must_use]
    pub const fn sigla(self) -> &'static str {
        match self {
            Self::SP => "SP",
            Self::RS => "RS",
            Self::PR => "PR",
            Self::MG => "MG",
            Self::SC => "SC",
            Self::RJ => "RJ",
            Self::ES => "ES",
            Self::DF => "DF",
        }
    }
}
