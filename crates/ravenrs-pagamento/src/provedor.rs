//! The contract every payment provider meets, whatever its transport.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::dominio::{
    Aprovacao, Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, ProvedorId,
    ReferenciaTransacao,
};

/// Why the provider did not do what was asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErroProvedor {
    /// It answered no, for certain; the request had no effect.
    Recusado {
        /// The provider's code.
        codigo: String,
        /// The reason.
        motivo: String,
    },
    /// The time ran out; it may or may not have happened.
    Tempo,
    /// It could not be reached or the connection fell; it may or may not have happened.
    Indisponivel(String),
    /// The answer could not be understood; it may or may not have happened.
    Protocolo(String),
    /// This provider does not do this through the integration.
    SemSuporte(String),
}

impl ErroProvedor {
    /// True when the outcome is unknown and only a later query can settle it.
    #[must_use]
    pub const fn incerto(&self) -> bool {
        matches!(self, Self::Tempo | Self::Indisponivel(_) | Self::Protocolo(_))
    }
}

impl fmt::Display for ErroProvedor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recusado { codigo, motivo } => write!(f, "recusado ({codigo}): {motivo}"),
            Self::Tempo => f.write_str("o tempo acabou"),
            Self::Indisponivel(motivo) => write!(f, "indisponível: {motivo}"),
            Self::Protocolo(motivo) => write!(f, "resposta inválida: {motivo}"),
            Self::SemSuporte(motivo) => write!(f, "sem suporte: {motivo}"),
        }
    }
}

impl std::error::Error for ErroProvedor {}

/// What the provider knows about a payment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoProvedor {
    /// It never saw this payment, for certain.
    Desconhecida,
    /// It cannot tell, as when the provider's own id was lost. The money may be moving.
    Indeterminada,
    /// Still running.
    EmAndamento,
    /// Approved and waiting for the third leg, or captured when there is none.
    Aprovada(Box<Aprovacao>),
    /// Refused.
    Negada {
        /// The provider's code.
        codigo: String,
        /// The reason.
        motivo: String,
    },
    /// Abandoned before approval.
    Cancelada,
    /// Approved and confirmed.
    Confirmada(Box<Aprovacao>),
    /// Approved and undone.
    Desfeita,
    /// Paid back.
    Estornada,
}

/// The outcome of asking to cancel a payment in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cancelamento {
    /// It is cancelled.
    Feito,
    /// The provider took the request; only a later answer settles it.
    Solicitado,
}

/// A payment provider, in the terms that serve all of them.
///
/// The flow never blocks on the customer: [`iniciar`](Self::iniciar) and
/// [`continuar`](Self::continuar) return a [`Passo`] saying what the screen does next. That covers
/// the iterative questions of CliSiTef, the polling of an HTTP order and the wait for an IntPos
/// file with the same two calls. [`estornar`](Self::estornar) may block up to the provider's limit.
///
/// Providers with no third leg treat [`confirmar`](Self::confirmar) as a no-op that is never
/// called, and answer [`ErroProvedor::SemSuporte`] to [`desfazer`](Self::desfazer).
pub trait ProvedorPagamento {
    /// Who this is.
    fn id(&self) -> ProvedorId;

    /// What it can do.
    fn capacidades(&self) -> Capacidades;

    /// Starts a payment.
    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor>;

    /// Goes on with a payment, with what the operator answered.
    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor>;

    /// Says the sale finished. Calling it again for the same transaction must do no harm.
    fn confirmar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor>;

    /// Reverses an approval whose sale did not finish. Calling it twice must do no harm.
    fn desfazer(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor>;

    /// Abandons a payment in progress.
    fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroProvedor>;

    /// Pays back a finished sale.
    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor>;

    /// Asks what it knows about this payment.
    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor>;

    /// Lists what the provider holds open, including from earlier sessions.
    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor>;

    /// What this adapter needs in order to find a running payment after a restart, such as the
    /// provider's own id. It goes to the journal, so it must hold nothing sensitive.
    fn rastro(&self, _id: &IdPagamento) -> Option<String> {
        None
    }

    /// Gives back what [`rastro`](Self::rastro) said, after a restart.
    fn restaurar_rastro(&mut self, _id: &IdPagamento, _rastro: &str) {}
}

impl<P: ProvedorPagamento + ?Sized> ProvedorPagamento for Box<P> {
    fn id(&self) -> ProvedorId {
        (**self).id()
    }
    fn capacidades(&self) -> Capacidades {
        (**self).capacidades()
    }
    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        (**self).iniciar(pedido)
    }
    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        (**self).continuar(id, entrada)
    }
    fn confirmar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        (**self).confirmar(referencia)
    }
    fn desfazer(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        (**self).desfazer(referencia)
    }
    fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        (**self).cancelar(id)
    }
    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        (**self).estornar(referencia)
    }
    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        (**self).consultar(id)
    }
    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        (**self).pendencias()
    }
    fn rastro(&self, id: &IdPagamento) -> Option<String> {
        (**self).rastro(id)
    }
    fn restaurar_rastro(&mut self, id: &IdPagamento, rastro: &str) {
        (**self).restaurar_rastro(id, rastro);
    }
}
