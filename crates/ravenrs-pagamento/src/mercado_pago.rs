//! Mercado Pago Point through the Orders API.
//!
//! **Never run against the real service: we have no account.** Written from the public documentation
//! and tested against fixtures of ours that follow its examples. The documentation says the access
//! token must stay on a backend, and webhooks need a public address, so a real deployment needs a
//! server of its own in front of this (see `docs/PAGAMENTOS.md`). Point machines are not pinpads.
//!
//! The documented order has no authorization code. So `cAut` comes out empty, with
//! [`AlertaFiscal::CAutAusente`](crate::AlertaFiscal::CAutAusente), and `reference_id` is not
//! used in its place: nothing says it is the number printed on the receipt.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Value, json};

use crate::dominio::{
    Aprovacao, Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, ProvedorId,
    ReferenciaTransacao, TipoPagamento, Vias,
};
use crate::fiscal::DadosFiscaisPagamento;
use crate::http::{Metodo, RequisicaoHttp, RespostaHttp, TransporteHttp};
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};

/// The address of the API.
pub const URL_API: &str = "https://api.mercadopago.com";
/// The documented virtual terminal, for tests with no machine. It has no real payments.
pub const TERMINAL_VIRTUAL: &str = "NEWLAND_N950__SBX0000001";
/// How long to wait between queries. The documentation discourages frequent ones.
const INTERVALO_CONSULTA_MS: u32 = 5_000;
const MOTIVO_SEM_C_AUT: &str = "a Orders API documentada não devolve código de autorização nem NSU";

/// An access token. `Debug` hides it.
#[derive(Clone)]
pub struct TokenAcesso(String);

impl TokenAcesso {
    /// Takes the token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }
}

impl fmt::Debug for TokenAcesso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TokenAcesso(<oculto>)")
    }
}

/// What can be simulated with test credentials through `POST /v1/orders/{id}/events`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventoSimulado {
    /// Paid. The fields are the documented body.
    Processado {
        /// `payment_method_type`, such as `credit_card`.
        tipo: String,
        /// `payment_method_id`, such as `master`.
        metodo: String,
        /// `installments`.
        parcelas: u8,
    },
    /// Failed with this `status_detail`.
    Falhou {
        /// `payment_method_type`.
        tipo: String,
        /// `payment_method_id`.
        metodo: String,
        /// `installments`.
        parcelas: u8,
        /// The documented detail, such as `insufficient_amount`.
        detalhe: String,
    },
    /// Refunded.
    Estornado,
    /// Cancelled.
    Cancelado,
    /// Expired.
    Expirado,
    /// Needs a decision on the terminal.
    AcaoNecessaria,
}

/// Mercado Pago Point over a transport.
#[derive(Debug)]
pub struct MercadoPagoPoint<T> {
    transporte: T,
    token: TokenAcesso,
    terminal: String,
    base: String,
    /// Our id to the order id. The journal keeps this across a restart through the `rastro`.
    ordens: BTreeMap<IdPagamento, String>,
}

impl<T: TransporteHttp> MercadoPagoPoint<T> {
    /// An adapter for one terminal, with the id exactly as `GET /terminals/v1/list` gives it.
    #[must_use]
    pub fn new(transporte: T, token: TokenAcesso, terminal: impl Into<String>) -> Self {
        Self {
            transporte,
            token,
            terminal: terminal.into(),
            base: URL_API.to_owned(),
            ordens: BTreeMap::new(),
        }
    }

    /// Uses another address, such as a server of the product that holds the token.
    #[must_use]
    pub fn com_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// `POST /v1/orders`. The idempotency key comes from our id, so a retry cannot charge twice.
    pub fn criar_ordem(&mut self, pedido: &PedidoPagamento) -> Result<Value, ErroProvedor> {
        let mut corpo = json!({
            "type": "point",
            "external_reference": pedido.id.as_str(),
            "transactions": { "payments": [ { "amount": reais(pedido.valor_centavos) } ] },
            "config": { "point": { "terminal_id": self.terminal } },
        });
        match pedido.tipo {
            TipoPagamento::Escolher => {}
            TipoPagamento::Debito => {
                corpo["config"]["payment_method"] = json!({ "default_type": "debit_card" })
            }
            TipoPagamento::Voucher => {
                corpo["config"]["payment_method"] = json!({ "default_type": "voucher_card" })
            }
            TipoPagamento::Pix => corpo["config"]["payment_method"] = json!({ "default_type": "qr" }),
            TipoPagamento::Credito { parcelas } => {
                let mut metodo = json!({ "default_type": "credit_card" });
                if parcelas > 1 {
                    metodo["default_installments"] = json!(parcelas);
                    metodo["installments_cost"] = json!("seller");
                }
                corpo["config"]["payment_method"] = metodo;
            }
        }
        let chave = format!("ravenrs-{}", pedido.id);
        let ordem = self.pedir(Metodo::Post, "/v1/orders", Some(&corpo), Some(&chave), &[])?;
        if let Some(id) = ordem["id"].as_str() {
            self.ordens.insert(pedido.id.clone(), id.to_owned());
        }
        Ok(ordem)
    }

    /// `GET /v1/orders/{id}`.
    pub fn consultar_ordem(&self, ordem: &str) -> Result<Value, ErroProvedor> {
        self.pedir(Metodo::Get, &format!("/v1/orders/{ordem}"), None, None, &[])
    }

    /// `POST /v1/orders/{id}/cancel`, also while the order is at the terminal.
    pub fn cancelar_ordem(&self, ordem: &str, chave: &str) -> Result<Value, ErroProvedor> {
        self.pedir(
            Metodo::Post,
            &format!("/v1/orders/{ordem}/cancel"),
            None,
            Some(chave),
            &[("x-allow-cancelable-status", "at_terminal")],
        )
    }

    /// `POST /v1/orders/{id}/refund`, with no body: the whole amount, within 90 days.
    pub fn estornar_ordem(&self, ordem: &str, chave: &str) -> Result<Value, ErroProvedor> {
        self.pedir(Metodo::Post, &format!("/v1/orders/{ordem}/refund"), None, Some(chave), &[])
    }

    /// `POST /v1/orders/{id}/events`: moves an order made with test credentials.
    pub fn simular_evento(&self, ordem: &str, evento: &EventoSimulado) -> Result<(), ErroProvedor> {
        let corpo = match evento {
            EventoSimulado::Processado { tipo, metodo, parcelas } => json!({
                "status": "processed",
                "payment_method_type": tipo,
                "payment_method_id": metodo,
                "installments": parcelas,
                "status_detail": "accredited",
            }),
            EventoSimulado::Falhou { tipo, metodo, parcelas, detalhe } => json!({
                "status": "failed",
                "payment_method_type": tipo,
                "payment_method_id": metodo,
                "installments": parcelas,
                "status_detail": detalhe,
            }),
            EventoSimulado::Estornado => json!({ "status": "refunded" }),
            EventoSimulado::Cancelado => json!({ "status": "canceled" }),
            EventoSimulado::Expirado => json!({ "status": "expired" }),
            EventoSimulado::AcaoNecessaria => json!({ "status": "action_required" }),
        };
        self.pedir(Metodo::Post, &format!("/v1/orders/{ordem}/events"), Some(&corpo), None, &[]).map(|_| ())
    }

    fn pedir(
        &self,
        metodo: Metodo,
        caminho: &str,
        corpo: Option<&Value>,
        idempotencia: Option<&str>,
        extras: &[(&str, &str)],
    ) -> Result<Value, ErroProvedor> {
        let mut cabecalhos = vec![
            ("Authorization".to_owned(), format!("Bearer {}", self.token.0)),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        if let Some(chave) = idempotencia {
            cabecalhos.push(("X-Idempotency-Key".to_owned(), chave.to_owned()));
        }
        for (nome, valor) in extras {
            cabecalhos.push(((*nome).to_owned(), (*valor).to_owned()));
        }
        let requisicao = RequisicaoHttp {
            metodo,
            url: format!("{}{caminho}", self.base),
            cabecalhos,
            corpo: corpo.map(|valor| valor.to_string().into_bytes()),
        };
        ler_resposta(&self.transporte.enviar(&requisicao)?)
    }

    fn ordem_de(&self, id: &IdPagamento) -> Option<&str> {
        self.ordens.get(id).map(String::as_str)
    }

    fn passo_da_ordem(&self, id: &IdPagamento, ordem: &Value) -> Result<Passo, ErroProvedor> {
        match ordem["status"].as_str() {
            Some("created") => Ok(aguardar("Enviado ao terminal")),
            Some("at_terminal") => Ok(aguardar("Pague no terminal")),
            Some("action_required") => Ok(aguardar("Confirme no terminal")),
            Some("processed") => Ok(Passo::Aprovado(Box::new(self.aprovacao_da_ordem(id, ordem)?))),
            Some("failed") => {
                let detalhe = detalhe_do_pagamento(ordem).unwrap_or_else(|| "failed".to_owned());
                Ok(Passo::Negado {
                    codigo: detalhe.clone(),
                    motivo: format!("Pagamento recusado: {detalhe}"),
                })
            }
            Some("canceled") => Ok(Passo::Cancelado { motivo: "Cancelado".into() }),
            Some("expired") => Ok(Passo::Cancelado { motivo: "Tempo esgotado no terminal".into() }),
            Some("refunded") => Err(ErroProvedor::Protocolo("ordem já estornada".into())),
            outro => Err(ErroProvedor::Protocolo(format!("status de ordem desconhecido: {outro:?}"))),
        }
    }

    fn aprovacao_da_ordem(&self, id: &IdPagamento, ordem: &Value) -> Result<Aprovacao, ErroProvedor> {
        let ordem_id = ordem["id"].as_str().ok_or_else(|| ErroProvedor::Protocolo("ordem sem id".into()))?;
        let pagamento = &ordem["transactions"]["payments"][0];
        let mut fiscais = DadosFiscaisPagamento::integrado();
        fiscais.com_c_aut(None, MOTIVO_SEM_C_AUT);
        fiscais.com_cnpj_da_rede(Some("MERCADO PAGO"));
        let tipo = pagamento["payment_method"]["type"].as_str();
        let e_pix = matches!(tipo, Some("qr" | "pix" | "account_money"));
        if !e_pix {
            fiscais.com_bandeira(pagamento["payment_method"]["id"].as_str());
        }
        fiscais
            .com_terminal(ordem["config"]["point"]["terminal_id"].as_str().or(Some(self.terminal.as_str())));
        let mut extras = BTreeMap::new();
        extras.insert("order_id".to_owned(), ordem_id.to_owned());
        if let Some(pagamento_id) = pagamento["id"].as_str() {
            extras.insert("payment_id".to_owned(), pagamento_id.to_owned());
        }
        if let Some(referencia) = pagamento["reference_id"].as_str() {
            extras.insert("reference_id".to_owned(), referencia.to_owned());
        }
        Ok(Aprovacao {
            referencia: ReferenciaTransacao {
                id: Some(id.clone()),
                rede: Some("MERCADO PAGO".to_owned()),
                nsu: None,
                controle: Some(ordem_id.to_owned()),
                valor_centavos: pagamento["amount"].as_str().and_then(centavos).unwrap_or(0),
                extras,
            },
            dados_fiscais: fiscais,
            vias: Vias { impressas_no_terminal: true, ..Vias::default() },
            mensagem_operador: None,
            tipo_efetivo: tipo_efetivo(tipo, pagamento["payment_method"]["installments"].as_u64()),
            avisos: Vec::new(),
        })
    }
}

fn aguardar(mensagem: &str) -> Passo {
    Passo::Aguardar { mensagem: Some(mensagem.to_owned()), tentar_em_ms: INTERVALO_CONSULTA_MS }
}

fn detalhe_do_pagamento(ordem: &Value) -> Option<String> {
    ordem["transactions"]["payments"][0]["status_detail"]
        .as_str()
        .or_else(|| ordem["status_detail"].as_str())
        .map(str::to_owned)
}

fn tipo_efetivo(tipo: Option<&str>, parcelas: Option<u64>) -> Option<TipoPagamento> {
    match tipo? {
        "debit_card" => Some(TipoPagamento::Debito),
        "credit_card" => Some(TipoPagamento::Credito {
            parcelas: parcelas.and_then(|n| u8::try_from(n).ok()).unwrap_or(1),
        }),
        "voucher_card" => Some(TipoPagamento::Voucher),
        "qr" | "pix" => Some(TipoPagamento::Pix),
        _ => None,
    }
}

fn ler_resposta(resposta: &RespostaHttp) -> Result<Value, ErroProvedor> {
    let json: Value = if resposta.corpo.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&resposta.corpo).unwrap_or(Value::Null)
    };
    match resposta.status {
        200..=299 => Ok(json),
        408 | 429 | 500..=599 => Err(ErroProvedor::Indisponivel(format!("HTTP {}", resposta.status))),
        status => Err(ErroProvedor::Recusado {
            codigo: json["errors"][0]["code"]
                .as_str()
                .or_else(|| json["code"].as_str())
                .map_or_else(|| status.to_string(), str::to_owned),
            motivo: json["errors"][0]["message"]
                .as_str()
                .or_else(|| json["message"].as_str())
                .map_or_else(|| format!("HTTP {status}"), str::to_owned),
        }),
    }
}

/// Cents to the string the API asks for, with two decimals.
fn reais(centavos: u64) -> String {
    format!("{}.{:02}", centavos / 100, centavos % 100)
}

/// The string with two decimals back to cents, exactly.
fn centavos(texto: &str) -> Option<u64> {
    let (inteiro, fracao) = texto.split_once('.')?;
    if fracao.len() != 2 {
        return None;
    }
    inteiro.parse::<u64>().ok()?.checked_mul(100)?.checked_add(fracao.parse().ok()?)
}

impl<T: TransporteHttp> ProvedorPagamento for MercadoPagoPoint<T> {
    fn id(&self) -> ProvedorId {
        ProvedorId::MercadoPagoPoint
    }

    fn capacidades(&self) -> Capacidades {
        Capacidades::MERCADO_PAGO_POINT
    }

    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        let ordem = self.criar_ordem(pedido)?;
        self.passo_da_ordem(&pedido.id, &ordem)
    }

    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        if entrada == EntradaOperador::Cancelar {
            return match self.cancelar(id)? {
                Cancelamento::Feito => Ok(Passo::Cancelado { motivo: "Cancelado pelo operador".into() }),
                Cancelamento::Solicitado => Ok(aguardar("Cancelando no terminal")),
            };
        }
        let ordem_id = self
            .ordem_de(id)
            .ok_or_else(|| ErroProvedor::Protocolo(format!("ordem de {id} não conhecida nesta sessão")))?;
        let ordem = self.consultar_ordem(ordem_id)?;
        self.passo_da_ordem(id, &ordem)
    }

    /// Never called: there is no third leg.
    fn confirmar(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Ok(())
    }

    fn desfazer(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Err(ErroProvedor::SemSuporte("Mercado Pago não tem desfazimento, só estorno".into()))
    }

    fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        let ordem_id = self
            .ordem_de(id)
            .ok_or_else(|| ErroProvedor::Protocolo(format!("ordem de {id} não conhecida nesta sessão")))?
            .to_owned();
        let resposta = self.cancelar_ordem(&ordem_id, &format!("ravenrs-cancelar-{id}"))?;
        // A 200 ends it. A 202 keeps `at_terminal`, and only a later answer settles it.
        Ok(if resposta["status"].as_str() == Some("canceled") {
            Cancelamento::Feito
        } else {
            Cancelamento::Solicitado
        })
    }

    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        let ordem = referencia
            .controle
            .as_deref()
            .ok_or_else(|| ErroProvedor::Protocolo("referência sem order_id".into()))?;
        self.estornar_ordem(ordem, &format!("ravenrs-estornar-{ordem}")).map(|_| ())
    }

    /// Asks by the order id. The journal holds it through [`rastro`](Self::rastro). If the power fell
    /// between creating the order and writing that down, the id is gone and the pages read offer no
    /// query by `external_reference`: the order may be on the terminal, so the answer is
    /// [`SituacaoProvedor::Indeterminada`].
    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        let Some(ordem_id) = self.ordem_de(id) else {
            return Ok(SituacaoProvedor::Indeterminada);
        };
        let ordem = self.consultar_ordem(ordem_id)?;
        Ok(match ordem["status"].as_str() {
            Some("created" | "at_terminal" | "action_required") => SituacaoProvedor::EmAndamento,
            Some("processed") => SituacaoProvedor::Aprovada(Box::new(self.aprovacao_da_ordem(id, &ordem)?)),
            Some("failed") => {
                let detalhe = detalhe_do_pagamento(&ordem).unwrap_or_else(|| "failed".to_owned());
                SituacaoProvedor::Negada { codigo: detalhe.clone(), motivo: detalhe }
            }
            Some("canceled" | "expired") => SituacaoProvedor::Cancelada,
            Some("refunded") => SituacaoProvedor::Estornada,
            outro => return Err(ErroProvedor::Protocolo(format!("status desconhecido: {outro:?}"))),
        })
    }

    /// There is no third leg, so nothing stays open on this side.
    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        Ok(Vec::new())
    }

    fn rastro(&self, id: &IdPagamento) -> Option<String> {
        self.ordens.get(id).cloned()
    }

    fn restaurar_rastro(&mut self, id: &IdPagamento, rastro: &str) {
        self.ordens.insert(id.clone(), rastro.to_owned());
    }
}

impl<T> MercadoPagoPoint<T> {
    /// Gives back an order id known from elsewhere, such as the server that got its webhook.
    pub fn lembrar_ordem(&mut self, id: IdPagamento, ordem: impl Into<String>) {
        self.ordens.insert(id, ordem.into());
    }
}

/// What a webhook says, once the server that received it has checked its `x-signature`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificacaoOrdem {
    /// `action`, such as `order.processed`.
    pub acao: String,
    /// `data.id`: the order.
    pub ordem: String,
    /// `data.status`.
    pub status: Option<String>,
    /// `data.external_reference`: our payment id.
    pub referencia_externa: Option<String>,
}

/// Reads a webhook body. It checks no signature: that needs the secret, which belongs on a server.
#[must_use]
pub fn ler_notificacao(corpo: &[u8]) -> Option<NotificacaoOrdem> {
    let json: Value = serde_json::from_slice(corpo).ok()?;
    Some(NotificacaoOrdem {
        acao: json["action"].as_str()?.to_owned(),
        ordem: json["data"]["id"].as_str()?.to_owned(),
        status: json["data"]["status"].as_str().map(str::to_owned),
        referencia_externa: json["data"]["external_reference"].as_str().map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dinheiro_vai_e_volta_exato() {
        assert_eq!(reais(1990), "19.90");
        assert_eq!(reais(5), "0.05");
        assert_eq!(reais(100_000), "1000.00");
        assert_eq!(centavos("19.90"), Some(1990));
        assert_eq!(centavos("0.05"), Some(5));
        assert_eq!(centavos("19.9"), None);
        assert_eq!(centavos("x"), None);
    }

    #[test]
    fn token_nao_aparece_em_debug() {
        assert!(!format!("{:?}", TokenAcesso::new("TEST-SEGREDO")).contains("SEGREDO"));
    }
}
