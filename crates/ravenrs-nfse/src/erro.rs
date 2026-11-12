//! What can go wrong, in pt-BR, ready to show.

use std::fmt;

use ravenrs_dfe::{DfeError, TransportError};

/// A message of the Sefin: an error or an alert, by its code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mensagem {
    /// The code, as `E0014`.
    pub codigo: String,
    /// What it says.
    pub descricao: String,
    /// More about it, when there is.
    pub complemento: Option<String>,
}

impl fmt::Display for Mensagem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.codigo, self.descricao)?;
        if let Some(complemento) = &self.complemento {
            write!(f, " ({complemento})")?;
        }
        Ok(())
    }
}

/// An error of the NFS-e.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NfseError {
    /// A field breaks a rule checked before anything is sent.
    Validacao {
        /// The field, by its name in the layout.
        campo: String,
        /// What is wrong.
        motivo: String,
    },
    /// The emitter was configured in a way the POC does not allow.
    Configuracao(String),
    /// XML, signature, content or certificate.
    Dfe(DfeError),
    /// No request could be completed.
    Transporte(TransportError),
    /// The Sefin refused, with these messages.
    Rejeitada(Vec<Mensagem>),
    /// The answer was lost and the DPS was not found afterwards: send the same DPS again, with
    /// the same number, never a new one.
    NaoConfirmada {
        /// The Id of the DPS to send again.
        id_dps: String,
        /// What happened on the way.
        motivo: String,
    },
    /// The service answered with something unexpected.
    RetornoInvalido {
        /// The HTTP status.
        status: u16,
        /// What was missing.
        motivo: String,
    },
}

impl NfseError {
    pub(crate) fn validacao(campo: &str, motivo: impl Into<String>) -> Self {
        Self::Validacao { campo: campo.to_owned(), motivo: motivo.into() }
    }

    /// The codes of a refusal; empty for any other error.
    #[must_use]
    pub fn codigos(&self) -> Vec<&str> {
        match self {
            Self::Rejeitada(mensagens) => mensagens.iter().map(|m| m.codigo.as_str()).collect(),
            _ => Vec::new(),
        }
    }
}

impl fmt::Display for NfseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validacao { campo, motivo } => write!(f, "{campo}: {motivo}"),
            Self::Configuracao(motivo) => write!(f, "configuração inválida: {motivo}"),
            Self::Dfe(erro) => write!(f, "{erro}"),
            Self::Transporte(erro) => write!(f, "{erro}"),
            Self::Rejeitada(mensagens) => {
                let textos: Vec<String> = mensagens.iter().map(ToString::to_string).collect();
                write!(f, "NFS-e recusada: {}", textos.join("; "))
            }
            Self::NaoConfirmada { id_dps, motivo } => write!(
                f,
                "sem confirmação da Sefin ({motivo}); envie de novo a mesma DPS {id_dps}, sem trocar o número"
            ),
            Self::RetornoInvalido { status, motivo } => {
                write!(f, "retorno inválido (HTTP {status}): {motivo}")
            }
        }
    }
}

impl std::error::Error for NfseError {}

impl From<DfeError> for NfseError {
    fn from(erro: DfeError) -> Self {
        match erro {
            DfeError::Transporte(erro) => Self::Transporte(erro),
            outro => Self::Dfe(outro),
        }
    }
}

impl From<TransportError> for NfseError {
    fn from(erro: TransportError) -> Self {
        Self::Transporte(erro)
    }
}
