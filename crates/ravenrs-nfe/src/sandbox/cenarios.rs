//! Ready answers, to use as handlers: `autorizacao(cenarios::paralisado_108)`.
//!
//! These answer without checking anything. To see a rejection earned by a real problem, send the
//! problem to the default checks instead. Each answer takes the format of the service it is on.

use super::{Resposta, SandboxRequest};

/// 100: authorized.
#[must_use]
pub fn autorizado_100(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(100)
}

/// 107: the service is up.
#[must_use]
pub fn em_operacao_107(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(107)
}

/// 108: the service is down for now.
#[must_use]
pub fn paralisado_108(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(108)
}

/// 204: this key was already authorized.
#[must_use]
pub fn duplicidade_204(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(204)
}

/// 225: the batch fails the schema.
#[must_use]
pub fn schema_225(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(225)
}

/// 297: the signature does not match.
#[must_use]
pub fn assinatura_297(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(297)
}

/// 539: this number already exists under another key.
#[must_use]
pub fn duplicidade_539(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(539)
}

/// 598: the recipient does not have the homologation text.
#[must_use]
pub fn homologacao_598(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(598)
}

/// 1115: the IBSCBS group is missing.
#[must_use]
pub fn ibscbs_1115(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(1115)
}

/// 129: a code in the plan whose meaning no source consulted confirms.
///
/// It exists so an application can see how a code it does not know behaves. See `REGRAS.md`.
#[must_use]
pub fn codigo_129(pedido: &SandboxRequest) -> Resposta {
    pedido.responder(129)
}

/// No answer arrives within the time limit.
#[must_use]
pub fn timeout(_pedido: &SandboxRequest) -> Resposta {
    Resposta::Timeout
}
