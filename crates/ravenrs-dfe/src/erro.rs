//! What can go wrong in the pieces every electronic fiscal document shares.

use std::fmt;

use crate::transporte::TransportError;

/// An error of the shared pieces: XML, signature, certificate or transport.
///
/// Each engine has its own error and turns this one into it; the texts are in pt-BR, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DfeError {
    /// Something was configured in a way that cannot work.
    Configuracao(String),
    /// A piece of XML could not be read.
    Xml(String),
    /// The document could not be signed, or a certificate could not be used.
    Assinatura(String),
    /// A compressed or encoded content could not be read.
    Conteudo(String),
    /// No request could be completed.
    Transporte(TransportError),
}

impl fmt::Display for DfeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuracao(motivo) => write!(f, "configuração inválida: {motivo}"),
            Self::Xml(motivo) => write!(f, "XML inválido: {motivo}"),
            Self::Assinatura(motivo) => write!(f, "falha na assinatura: {motivo}"),
            Self::Conteudo(motivo) => write!(f, "conteúdo inválido: {motivo}"),
            Self::Transporte(erro) => write!(f, "{erro}"),
        }
    }
}

impl std::error::Error for DfeError {}

impl From<TransportError> for DfeError {
    fn from(erro: TransportError) -> Self {
        Self::Transporte(erro)
    }
}
