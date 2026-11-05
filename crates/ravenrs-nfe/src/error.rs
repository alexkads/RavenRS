//! Errors of the fiscal engine.

use std::fmt;

use crate::transporte::TransportError;

/// Why an operation of the engine did not finish.
///
/// A SEFAZ rejection is not an error here: it comes back as a regular return with its `cStat`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NfeError {
    /// A field of the document breaks a rule checked before anything is sent.
    Validacao {
        /// The field, by its name in the layout when it has one.
        campo: String,
        /// What is wrong, in pt-BR, ready to show to the user.
        motivo: String,
    },
    /// The emitter was configured in a way the POC does not allow.
    Configuracao(String),
    /// A piece of XML could not be read.
    Xml(String),
    /// The document could not be signed.
    Assinatura(String),
    /// No request could be completed.
    Transporte(TransportError),
    /// The service answered with something other than the expected return.
    RetornoInvalido {
        /// The HTTP status of the answer.
        status: u16,
        /// What was missing, or the SOAP fault text.
        motivo: String,
    },
}

impl NfeError {
    pub(crate) fn validacao(campo: &str, motivo: impl Into<String>) -> Self {
        Self::Validacao { campo: campo.to_string(), motivo: motivo.into() }
    }
}

impl fmt::Display for NfeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validacao { campo, motivo } => write!(f, "{campo}: {motivo}"),
            Self::Configuracao(motivo) => write!(f, "configuração inválida: {motivo}"),
            Self::Xml(motivo) => write!(f, "XML inválido: {motivo}"),
            Self::Assinatura(motivo) => write!(f, "falha na assinatura: {motivo}"),
            Self::Transporte(erro) => write!(f, "{erro}"),
            Self::RetornoInvalido { status, motivo } => {
                write!(f, "retorno inválido (HTTP {status}): {motivo}")
            }
        }
    }
}

impl std::error::Error for NfeError {}

impl From<TransportError> for NfeError {
    fn from(erro: TransportError) -> Self {
        Self::Transporte(erro)
    }
}
