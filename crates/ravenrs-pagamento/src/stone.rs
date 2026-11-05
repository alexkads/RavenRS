//! Stone: the fiscal data of Connect 2.0, and AutoTEF over local HTTP.
//!
//! Stone has more than one product and they work differently:
//!
//! - **Connect 2.0**: an order in the Pagar.me API goes to an Android machine, not a pinpad. It has
//!   no sandbox, a webhook is mandatory, and there is no refund route. Only [`dados_de_charge_paid`]
//!   exists here, for the server that receives the webhook.
//! - **TEF Stone**: a native library, reached through [`crate::auxiliar`].
//! - **AutoTEF**: [`AutoTef`]. **NOT CONFIRMED:** its documentation answered 403. Everything here
//!   comes from third-party code that integrates with it, so even the existence of the product
//!   today is unknown. Field names and units may differ. Nothing has run against the real one.

use serde_json::{Value, json};

use crate::dominio::{
    Aprovacao, AvisoAprovacao, Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, Pergunta,
    ProvedorId, ReferenciaTransacao, TipoPagamento, Vias,
};
use crate::fiscal::DadosFiscaisPagamento;
use crate::http::{Metodo, RequisicaoHttp, TransporteHttp};
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};

/// What a Connect 2.0 `charge.paid` webhook gives the fiscal document.
///
/// The webhook has no CNPJ of the acquirer: it comes from the table, with an alert. The
/// documentation's table says `metadata.schemeName` and its example says `metadata.scheme_name`;
/// the example is what is read, and the other is accepted too.
#[must_use]
pub fn dados_de_charge_paid(webhook: &Value) -> DadosFiscaisPagamento {
    let metadata = &webhook["data"]["metadata"];
    let mut dados = DadosFiscaisPagamento::integrado();
    dados.com_c_aut(
        metadata["authorization_code"].as_str().or_else(|| metadata["authorizationCode"].as_str()),
        "o webhook charge.paid veio sem authorization_code",
    );
    dados.com_cnpj_da_rede(Some("STONE"));
    dados.com_bandeira(metadata["scheme_name"].as_str().or_else(|| metadata["schemeName"].as_str()));
    dados.com_terminal(
        metadata["terminal_serial_number"].as_str().or_else(|| metadata["terminalSerialNumber"].as_str()),
    );
    dados
}

/// The address the third-party code uses by default for AutoTEF.
pub const URL_AUTOTEF: &str = "http://localhost:8000/";

/// Stone AutoTEF over local HTTP. **Not confirmed with Stone**; see the module text.
///
/// A payment call blocks until the customer finishes, up to the limit of the transport.
#[derive(Debug)]
pub struct AutoTef<T> {
    transporte: T,
    base: String,
    pix: Option<(IdPagamento, String)>,
}

impl<T: TransporteHttp> AutoTef<T> {
    /// An adapter at the default local address.
    #[must_use]
    pub fn new(transporte: T) -> Self {
        Self { transporte, base: URL_AUTOTEF.to_owned(), pix: None }
    }

    /// `POST api/Activate`: ties the service to a Stone code and a pinpad port.
    pub fn ativar(&self, codigo_stone: &str, porta_pinpad: &str, parceiro: &str) -> Result<(), ErroProvedor> {
        self.pedir(
            "api/Activate",
            &json!({ "StoneCode": codigo_stone, "ConnectionName": porta_pinpad, "PartnerName": parceiro }),
        )
        .map(|_| ())
    }

    fn pedir(&self, caminho: &str, corpo: &Value) -> Result<Value, ErroProvedor> {
        let requisicao = RequisicaoHttp {
            metodo: Metodo::Post,
            url: format!("{}{caminho}", self.base),
            cabecalhos: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            corpo: Some(corpo.to_string().into_bytes()),
        };
        let resposta = self.transporte.enviar(&requisicao)?;
        let json: Value = serde_json::from_slice(&resposta.corpo).unwrap_or(Value::Null);
        match resposta.status {
            200..=299 => Ok(json),
            408 | 429 | 500..=599 => Err(ErroProvedor::Indisponivel(format!("HTTP {}", resposta.status))),
            status => Err(ErroProvedor::Recusado {
                codigo: status.to_string(),
                motivo: json["message"].as_str().map_or_else(|| format!("HTTP {status}"), str::to_owned),
            }),
        }
    }

    fn aprovacao(&self, id: &IdPagamento, valor_centavos: u64, resposta: &Value) -> Aprovacao {
        let recibo = &resposta["receipt"];
        let nsu = recibo["acquirerTransactionKey"].as_str().or_else(|| resposta["transactionId"].as_str());
        let mut fiscais = DadosFiscaisPagamento::integrado();
        fiscais.com_c_aut(recibo["authorisationCode"].as_str(), "a resposta veio sem authorisationCode");
        fiscais.com_cnpj_da_rede(Some("STONE"));
        fiscais.com_bandeira(recibo["brandName"].as_str());
        let linhas = |via: &Value| -> Vec<String> {
            via.as_str().map_or_else(Vec::new, |texto| {
                texto.replace("\\r\\n", "\n").lines().map(str::to_owned).collect()
            })
        };
        Aprovacao {
            referencia: ReferenciaTransacao {
                id: Some(id.clone()),
                rede: Some("STONE".to_owned()),
                nsu: nsu.map(str::to_owned),
                controle: recibo["initiatorTransactionKey"].as_str().map(str::to_owned),
                valor_centavos,
                extras: std::collections::BTreeMap::new(),
            },
            dados_fiscais: fiscais,
            vias: Vias {
                cliente: linhas(&recibo["clientVia"]),
                estabelecimento: linhas(&recibo["merchantVia"]),
                unica: Vec::new(),
                impressas_no_terminal: false,
            },
            mensagem_operador: None,
            tipo_efetivo: None,
            avisos: vec![AvisoAprovacao::Outro("AutoTEF não confirmado com a Stone".to_owned())],
        }
    }
}

fn valor_decimal(centavos: u64) -> Value {
    // The third-party code sends the amount in reais as a number, so this is the one place money is
    // a float. Two decimals survive this at any amount a checkout sees.
    json!(centavos as f64 / 100.0)
}

impl<T: TransporteHttp> ProvedorPagamento for AutoTef<T> {
    fn id(&self) -> ProvedorId {
        ProvedorId::StoneAutoTef
    }

    fn capacidades(&self) -> Capacidades {
        Capacidades::STONE_AUTOTEF
    }

    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        if pedido.tipo == TipoPagamento::Pix {
            let resposta = self.pedir(
                "api/Pix/Pay",
                &json!({ "amount": valor_decimal(pedido.valor_centavos), "expiresIn": 600 }),
            )?;
            let transacao = resposta["transactionId"]
                .as_str()
                .ok_or_else(|| ErroProvedor::Protocolo("Pix sem transactionId".into()))?;
            self.pix = Some((pedido.id.clone(), transacao.to_owned()));
            return Ok(Passo::Perguntar(Pergunta::QrCode {
                conteudo: resposta["qrCodeContent"].as_str().unwrap_or_default().to_owned(),
                mensagem: Some("Pague com Pix".to_owned()),
            }));
        }
        let (conta, parcelas, tipo_parcela) = match pedido.tipo {
            TipoPagamento::Debito => ("Debit", 1, 0),
            TipoPagamento::Credito { parcelas } if parcelas > 1 => ("Credit", parcelas, 3),
            TipoPagamento::Credito { .. } => ("Credit", 1, 1),
            _ => ("Undefined", 1, 1),
        };
        let resposta = self.pedir(
            "api/Pay",
            &json!({
                "amount": valor_decimal(pedido.valor_centavos),
                "hasAlcoholicDrink": false,
                "accountType": conta,
                "installment": { "number": parcelas, "type": tipo_parcela },
            }),
        )?;
        let aprovado = resposta["receipt"]["acquirerTransactionKey"].as_str().is_some_and(|k| !k.is_empty())
            || resposta["wasSuccessfull"].as_bool() == Some(true);
        if aprovado {
            Ok(Passo::Aprovado(Box::new(self.aprovacao(&pedido.id, pedido.valor_centavos, &resposta))))
        } else {
            Ok(Passo::Negado {
                codigo: "AUTOTEF".to_owned(),
                motivo: resposta["message"].as_str().unwrap_or("Transação não aprovada").to_owned(),
            })
        }
    }

    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        let Some((_, transacao)) = self.pix.clone().filter(|(pix, _)| pix == id) else {
            return Err(ErroProvedor::Protocolo("nada em andamento neste adaptador".into()));
        };
        if entrada == EntradaOperador::Cancelar {
            self.pedir("api/Pix/Cancel", &json!({ "transactionId": transacao }))?;
            self.pix = None;
            return Ok(Passo::Cancelado { motivo: "Pix cancelado".to_owned() });
        }
        let resposta = self.pedir("api/Pix/Status", &json!({ "transactionId": transacao }))?;
        if resposta["isExpired"].as_bool() == Some(true) {
            self.pix = None;
            return Ok(Passo::Cancelado { motivo: "Pix expirou".to_owned() });
        }
        if resposta["statusPaymentPix"].as_i64() == Some(1) {
            self.pix = None;
            let mut com_id = resposta;
            com_id["transactionId"] = json!(transacao);
            return Ok(Passo::Aprovado(Box::new(self.aprovacao(id, 0, &com_id))));
        }
        Ok(Passo::Aguardar { mensagem: Some("Aguardando o Pix".to_owned()), tentar_em_ms: 2_000 })
    }

    /// The third-party code sends no third leg, so this is never called.
    fn confirmar(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Ok(())
    }

    fn desfazer(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Err(ErroProvedor::SemSuporte("o AutoTEF, como lido, só tem cancelamento".into()))
    }

    fn cancelar(&mut self, _: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        Err(ErroProvedor::SemSuporte("o operador não interrompe um pagamento com cartão no AutoTEF".into()))
    }

    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        let nsu =
            referencia.nsu.as_deref().ok_or_else(|| ErroProvedor::Protocolo("referência sem NSU".into()))?;
        // The third-party code tells Pix by the length of the key.
        let corpo = if nsu.len() >= 27 {
            self.pedir("api/Pix/Cancel", &json!({ "transactionId": nsu }))
        } else {
            self.pedir(
                "api/cancel",
                &json!({
                    "acquirerTransactionKey": nsu,
                    "amount": valor_decimal(referencia.valor_centavos),
                    "transactionType": "Undefined",
                }),
            )
        };
        corpo.map(|_| ())
    }

    /// There is no query in what was read, so a lost answer cannot be recovered here.
    fn consultar(&mut self, _: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        Ok(SituacaoProvedor::Indeterminada)
    }

    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::fiscal::AlertaFiscal;
    use crate::http::{ErroTransporte, RespostaHttp};

    #[test]
    fn charge_paid_do_exemplo_da_documentacao() {
        let webhook = json!({
            "type": "charge.paid",
            "data": {
                "code": "11831338033297",
                "amount": 2538,
                "metadata": {
                    "scheme_name": "MasterCard",
                    "authorization_code": "0124659",
                    "terminal_serial_number": "6C5680991"
                }
            }
        });
        let dados = dados_de_charge_paid(&webhook);
        assert_eq!(dados.c_aut.as_deref(), Some("0124659"));
        assert_eq!(dados.t_band.as_deref(), Some("02"));
        assert_eq!(dados.cnpj.as_deref(), Some("16501555000157"));
        assert_eq!(dados.id_term_pag.as_deref(), Some("6C5680991"));
        assert!(matches!(dados.alertas[0], AlertaFiscal::CnpjDeTabelaNaoConferida { .. }));
    }

    #[test]
    fn charge_paid_sem_autorizacao_vira_alerta() {
        let dados = dados_de_charge_paid(&json!({ "data": { "metadata": {} } }));
        assert_eq!(dados.c_aut, None);
        assert!(dados.alertas.iter().any(|a| matches!(a, AlertaFiscal::CAutAusente { .. })));
    }

    #[derive(Default)]
    struct Local {
        pedidos: RefCell<Vec<RequisicaoHttp>>,
        resposta: RefCell<Value>,
    }

    impl TransporteHttp for Local {
        fn enviar(&self, requisicao: &RequisicaoHttp) -> Result<RespostaHttp, ErroTransporte> {
            self.pedidos.borrow_mut().push(requisicao.clone());
            Ok(RespostaHttp { status: 200, corpo: self.resposta.borrow().to_string().into_bytes() })
        }
    }

    fn pedido(tipo: TipoPagamento) -> PedidoPagamento {
        PedidoPagamento::new(IdPagamento::new("a1").expect("id"), 1990, tipo).expect("pedido")
    }

    #[test]
    fn pagamento_com_recibo_aprova_e_mapeia() {
        let local = Local::default();
        *local.resposta.borrow_mut() = json!({
            "receipt": {
                "acquirerTransactionKey": "NSU1",
                "initiatorTransactionKey": "INI1",
                "authorisationCode": "AUT1",
                "brandName": "VISA",
                "clientVia": "LINHA 1\\r\\nLINHA 2",
                "merchantVia": "LOJA"
            }
        });
        let mut autotef = AutoTef::new(&local);
        let Passo::Aprovado(aprovacao) = autotef.iniciar(&pedido(TipoPagamento::Debito)).expect("paga")
        else {
            panic!("devia aprovar");
        };
        assert_eq!(aprovacao.dados_fiscais.c_aut.as_deref(), Some("AUT1"));
        assert_eq!(aprovacao.dados_fiscais.t_band.as_deref(), Some("01"));
        assert_eq!(aprovacao.vias.via_cliente(), ["LINHA 1", "LINHA 2"]);
        assert!(
            aprovacao
                .avisos
                .iter()
                .any(|a| matches!(a, AvisoAprovacao::Outro(t) if t.contains("não confirmado")))
        );
        let enviado = &local.pedidos.borrow()[0];
        assert_eq!(enviado.url, "http://localhost:8000/api/Pay");
        assert_eq!(enviado.json().expect("json")["accountType"], "Debit");
    }

    #[test]
    fn sem_chave_nem_sucesso_e_negado() {
        let local = Local::default();
        *local.resposta.borrow_mut() = json!({ "message": "recusada" });
        let mut autotef = AutoTef::new(&local);
        assert!(matches!(autotef.iniciar(&pedido(TipoPagamento::Debito)), Ok(Passo::Negado { .. })));
    }

    #[test]
    fn pix_mostra_qr_e_acompanha() {
        let local = Local::default();
        *local.resposta.borrow_mut() = json!({ "transactionId": "T1", "qrCodeContent": "PIX" });
        let mut autotef = AutoTef::new(&local);
        let id = IdPagamento::new("a1").expect("id");
        assert!(matches!(
            autotef.iniciar(&pedido(TipoPagamento::Pix)),
            Ok(Passo::Perguntar(Pergunta::QrCode { .. }))
        ));
        *local.resposta.borrow_mut() = json!({ "statusPaymentPix": 0 });
        assert!(matches!(autotef.continuar(&id, EntradaOperador::Nada), Ok(Passo::Aguardar { .. })));
        *local.resposta.borrow_mut() = json!({ "statusPaymentPix": 1 });
        let Passo::Aprovado(aprovacao) = autotef.continuar(&id, EntradaOperador::Nada).expect("pago") else {
            panic!("devia aprovar");
        };
        assert_eq!(aprovacao.referencia.nsu.as_deref(), Some("T1"));
    }

    #[test]
    fn sem_terceira_perna_e_sem_desfazer() {
        let local = Local::default();
        let mut autotef = AutoTef::new(&local);
        assert!(!autotef.capacidades().terceira_perna);
        assert!(autotef.desfazer(&ReferenciaTransacao::default()).is_err());
        assert!(autotef.ativar("123", "COM3", "RAVENRS").is_ok());
    }
}
