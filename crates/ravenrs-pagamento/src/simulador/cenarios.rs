//! Ready answers, to use as handlers: `iniciar(cenarios::negado)`.

use super::{PREFIXO, RequisicaoSimulada, Resposta};
use crate::dominio::{AvisoAprovacao, OpcaoMenu, Passo, Pergunta, TipoCampo};
use crate::fiscal::AlertaFiscal;
use crate::provedor::ErroProvedor;

/// Approved, with the default approval.
#[must_use]
pub fn aprovado(pedido: &RequisicaoSimulada) -> Resposta {
    pedido.aprovar()
}

/// Refused by the issuer.
#[must_use]
pub fn negado(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Negado { codigo: "SIM-05".into(), motivo: format!("{PREFIXO}NÃO AUTORIZADA") })
}

/// The time ran out and the request never arrived.
#[must_use]
pub fn timeout(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Erro(ErroProvedor::Tempo)
}

/// The time ran out on the way back: the provider did it, and the caller does not know.
#[must_use]
pub fn timeout_depois_de_processar(_: &RequisicaoSimulada) -> Resposta {
    Resposta::PerdeuResposta(ErroProvedor::Tempo)
}

/// The connection fell before the provider got the confirmation. For [`super::confirmar`].
#[must_use]
pub fn queda_antes_da_confirmacao(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Erro(ErroProvedor::Indisponivel(format!("{PREFIXO}conexão caiu antes de confirmar")))
}

/// The provider confirmed, and the answer was lost on the way back. For [`super::confirmar`].
#[must_use]
pub fn queda_depois_da_confirmacao(_: &RequisicaoSimulada) -> Resposta {
    Resposta::PerdeuResposta(ErroProvedor::Indisponivel(format!("{PREFIXO}resposta perdida")))
}

/// The operator gave up on the provider's side.
#[must_use]
pub fn cancelado_pelo_operador(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Cancelado { motivo: format!("{PREFIXO}cancelado pelo operador") })
}

/// Approved, and the provider's terminal says it is out of paper.
#[must_use]
pub fn sem_papel(pedido: &RequisicaoSimulada) -> Resposta {
    pedido.aprovar_com(|aprovacao| aprovacao.avisos.push(AvisoAprovacao::SemPapel))
}

/// Approved with no `cAut`, as an API that does not return it.
#[must_use]
pub fn c_aut_ausente(pedido: &RequisicaoSimulada) -> Resposta {
    pedido.aprovar_com(|aprovacao| {
        aprovacao.dados_fiscais.c_aut = None;
        aprovacao
            .dados_fiscais
            .alertas
            .push(AlertaFiscal::CAutAusente { motivo: format!("{PREFIXO}o provedor não devolveu") });
    })
}

/// Asks the operator for the amount of installments. The default `continuar` then approves.
#[must_use]
pub fn pergunta_parcelas(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Perguntar(Pergunta::Campo {
        rotulo: format!("{PREFIXO}QUANTAS PARCELAS?"),
        tipo: TipoCampo::Numero,
        minimo: 1,
        maximo: 2,
    }))
}

/// Offers a choice. The default `continuar` then approves.
#[must_use]
pub fn menu_debito_credito(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Perguntar(Pergunta::Menu {
        titulo: format!("{PREFIXO}FORMA"),
        opcoes: vec![
            OpcaoMenu { valor: "1".into(), texto: "DEBITO".into() },
            OpcaoMenu { valor: "2".into(), texto: "CREDITO".into() },
        ],
    }))
}

/// Shows a QR code that means nothing and keeps the flow running.
#[must_use]
pub fn qr_pix(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Perguntar(Pergunta::QrCode {
        conteudo: "SIMULADOR-SEM-VALOR".into(),
        mensagem: Some(format!("{PREFIXO}PAGUE COM PIX")),
    }))
}

/// Keeps the screen waiting, as a terminal that has not answered yet.
#[must_use]
pub fn aguardando(_: &RequisicaoSimulada) -> Resposta {
    Resposta::Passo(Passo::Aguardar { mensagem: Some(format!("{PREFIXO}AGUARDE")), tentar_em_ms: 500 })
}
