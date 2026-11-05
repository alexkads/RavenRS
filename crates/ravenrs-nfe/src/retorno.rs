//! Reads what the services answer, and the table of the `cStat` this crate knows.

use crate::error::NfeError;
use crate::transporte::HttpResponse;
use crate::xml::{self, Element};

/// The fields every return has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retorno {
    /// The status code.
    pub c_stat: u32,
    /// The text the server sent.
    pub x_motivo: String,
    /// Which application answered. The sandbox says `RAVENRS-SANDBOX`.
    pub ver_aplic: String,
    /// The environment the server says it is, `tpAmb`.
    pub tp_amb: String,
    /// When the server received the message, when it says.
    pub dh_recbto: Option<String>,
}

impl Retorno {
    /// Whether this came from the local sandbox.
    #[must_use]
    pub fn veio_do_sandbox(&self) -> bool {
        self.ver_aplic == "RAVENRS-SANDBOX"
    }
}

/// The answer of the service status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusServico {
    /// The common fields.
    pub retorno: Retorno,
    /// The average time the server reports, in seconds.
    pub t_med: Option<String>,
}

impl StatusServico {
    /// Whether the service is up: `cStat` 107.
    #[must_use]
    pub fn em_operacao(&self) -> bool {
        self.retorno.c_stat == 107
    }
}

/// The protocol of one document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Protocolo {
    /// The key.
    pub ch_nfe: String,
    /// When the server processed it.
    pub dh_recbto: Option<String>,
    /// The protocol number, when one was given.
    pub n_prot: Option<String>,
    /// The digest the server saw.
    pub dig_val: Option<String>,
    /// The status of the document.
    pub c_stat: u32,
    /// The text of the status.
    pub x_motivo: String,
}

/// The outcome of a document, read from its `cStat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Situacao {
    /// The use was authorized: 100 or 150.
    Autorizada,
    /// The use was denied: 110, 205, 301, 302 or 303.
    Denegada,
    /// The document already exists with this key or number: 204 or 539.
    Duplicada,
    /// The service is down for now: 108 or 109.
    Paralisado,
    /// The document was refused.
    Rejeitada,
}

/// The situation of a `cStat`.
#[must_use]
pub const fn situacao_de(c_stat: u32) -> Situacao {
    match c_stat {
        100 | 150 => Situacao::Autorizada,
        110 | 205 | 301 | 302 | 303 => Situacao::Denegada,
        204 | 539 => Situacao::Duplicada,
        108 | 109 => Situacao::Paralisado,
        _ => Situacao::Rejeitada,
    }
}

/// The answer of a synchronous authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autorizacao {
    /// The status of the batch.
    pub lote: Retorno,
    /// The protocol of the document, absent when the batch itself was refused.
    pub protocolo: Option<Protocolo>,
    /// The document with its protocol, `nfeProc`, present only when authorized.
    pub nfe_proc: Option<String>,
}

impl Autorizacao {
    /// The status that decides: the document's if it has one, otherwise the batch's.
    #[must_use]
    pub fn c_stat(&self) -> u32 {
        self.protocolo.as_ref().map_or(self.lote.c_stat, |protocolo| protocolo.c_stat)
    }

    /// The text of [`Self::c_stat`].
    #[must_use]
    pub fn x_motivo(&self) -> &str {
        self.protocolo.as_ref().map_or(self.lote.x_motivo.as_str(), |p| p.x_motivo.as_str())
    }

    /// The outcome.
    #[must_use]
    pub fn situacao(&self) -> Situacao {
        situacao_de(self.c_stat())
    }
}

/// The answer of a protocol query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsultaProtocolo {
    /// The common fields; `cStat` is the situation of the key.
    pub retorno: Retorno,
    /// The key the server answered about.
    pub ch_nfe: Option<String>,
    /// The protocol, when there is one.
    pub protocolo: Option<Protocolo>,
}

/// The answer to an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetornoEvento {
    /// The status of the batch.
    pub lote: Retorno,
    /// The status of the event, or of the batch if the event was not reached.
    pub c_stat: u32,
    /// The text of the status.
    pub x_motivo: String,
    /// The key of the document.
    pub ch_nfe: Option<String>,
    /// The protocol of the event.
    pub n_prot: Option<String>,
}

impl RetornoEvento {
    /// Whether the cancellation was registered: 135, 136 or 155.
    #[must_use]
    pub fn registrado(&self) -> bool {
        matches!(self.c_stat, 135 | 136 | 155)
    }
}

/// Finds the return element, or says what came instead: a SOAP fault or nothing usable.
fn corpo(resposta: &HttpResponse, raiz: &str) -> Result<Element, NfeError> {
    let invalido = |motivo: String| NfeError::RetornoInvalido { status: resposta.status, motivo };
    let texto = std::str::from_utf8(&resposta.body)
        .map_err(|_| invalido("corpo da resposta não é UTF-8".to_string()))?;
    let documento = xml::parse(texto).map_err(|erro| invalido(erro.to_string()))?;
    if let Some(retorno) = documento.find(raiz) {
        return Ok(retorno.clone());
    }
    if let Some(fault) = documento.find("Fault") {
        let motivo = fault
            .find("Reason")
            .and_then(|reason| reason.find_text("Text"))
            .or_else(|| fault.find_text("faultstring"))
            .unwrap_or_else(|| "SOAP Fault sem texto".to_string());
        return Err(invalido(motivo));
    }
    Err(invalido(format!("{raiz} não encontrado na resposta")))
}

fn comum(element: &Element, status: u16) -> Result<Retorno, NfeError> {
    let c_stat = element
        .child_text("cStat")
        .and_then(|text| text.trim().parse().ok())
        .ok_or_else(|| NfeError::RetornoInvalido { status, motivo: "retorno sem cStat".to_string() })?;
    Ok(Retorno {
        c_stat,
        x_motivo: element.child_text("xMotivo").unwrap_or_default(),
        ver_aplic: element.child_text("verAplic").unwrap_or_default(),
        tp_amb: element.child_text("tpAmb").unwrap_or_default(),
        dh_recbto: element.child_text("dhRecbto"),
    })
}

fn protocolo(element: &Element, status: u16) -> Result<Option<Protocolo>, NfeError> {
    let Some(inf) = element.child("protNFe").and_then(|prot| prot.child("infProt")) else {
        return Ok(None);
    };
    let base = comum(inf, status)?;
    Ok(Some(Protocolo {
        ch_nfe: inf.child_text("chNFe").unwrap_or_default(),
        dh_recbto: base.dh_recbto,
        n_prot: inf.child_text("nProt"),
        dig_val: inf.child_text("digVal"),
        c_stat: base.c_stat,
        x_motivo: base.x_motivo,
    }))
}

/// Reads the service status return.
pub fn ler_status(resposta: &HttpResponse) -> Result<StatusServico, NfeError> {
    let element = corpo(resposta, "retConsStatServ")?;
    Ok(StatusServico { retorno: comum(&element, resposta.status)?, t_med: element.child_text("tMed") })
}

/// Reads the authorization return; `nfe_assinada` becomes `nfeProc` when it was authorized.
pub fn ler_autorizacao(resposta: &HttpResponse, nfe_assinada: &str) -> Result<Autorizacao, NfeError> {
    let element = corpo(resposta, "retEnviNFe")?;
    let lote = comum(&element, resposta.status)?;
    let protocolo = protocolo(&element, resposta.status)?;
    let autorizada = protocolo.as_ref().is_some_and(|p| situacao_de(p.c_stat) == Situacao::Autorizada);
    let nfe_proc = element.child("protNFe").filter(|_| autorizada).map(|prot| {
        format!(
            "<nfeProc versao=\"4.00\" xmlns=\"{}\">{nfe_assinada}{}</nfeProc>",
            crate::soap::NS_NFE,
            xml::to_string(prot)
        )
    });
    Ok(Autorizacao { lote, protocolo, nfe_proc })
}

/// Reads the protocol query return.
pub fn ler_consulta(resposta: &HttpResponse) -> Result<ConsultaProtocolo, NfeError> {
    let element = corpo(resposta, "retConsSitNFe")?;
    Ok(ConsultaProtocolo {
        retorno: comum(&element, resposta.status)?,
        ch_nfe: element.child_text("chNFe"),
        protocolo: protocolo(&element, resposta.status)?,
    })
}

/// Reads the event return.
pub fn ler_evento(resposta: &HttpResponse) -> Result<RetornoEvento, NfeError> {
    let element = corpo(resposta, "retEnvEvento")?;
    let lote = comum(&element, resposta.status)?;
    let inf = element.child("retEvento").and_then(|retorno| retorno.child("infEvento"));
    let (c_stat, x_motivo) = match inf {
        Some(inf) => {
            let base = comum(inf, resposta.status)?;
            (base.c_stat, base.x_motivo)
        }
        None => (lote.c_stat, lote.x_motivo.clone()),
    };
    Ok(RetornoEvento {
        c_stat,
        x_motivo,
        ch_nfe: inf.and_then(|inf| inf.child_text("chNFe")),
        n_prot: inf.and_then(|inf| inf.child_text("nProt")),
        lote,
    })
}

/// The short description of a `cStat` this crate deals with, in pt-BR.
///
/// The server's own `xMotivo` is what counts. This table only labels the code for a screen, and
/// its texts are not copied from the MOC, which could not be read during the POC.
#[must_use]
pub const fn descricao_c_stat(c_stat: u32) -> Option<&'static str> {
    Some(match c_stat {
        100 => "Autorizado o uso da NF-e",
        101 => "Cancelamento de NF-e homologado",
        104 => "Lote processado",
        107 => "Serviço em operação",
        108 => "Serviço paralisado momentaneamente",
        109 => "Serviço paralisado sem previsão",
        128 => "Lote de evento processado",
        135 => "Evento registrado e vinculado à NF-e",
        204 => "Duplicidade de NF-e",
        217 => "NF-e não consta na base de dados",
        222 => "Protocolo difere do cadastrado",
        225 => "Falha no schema XML do lote",
        236 => "Chave de acesso com dígito verificador inválido",
        243 => "XML mal formado",
        252 => "Ambiente informado diverge do ambiente de recebimento",
        253 => "Dígito verificador da chave composta inválido",
        297 => "Assinatura difere do calculado",
        298 => "Assinatura difere do padrão do sistema",
        373 => "Descrição do primeiro item difere do texto de homologação",
        394 => "Nota fiscal sem as informações do QR Code",
        452 => "Resposta assíncrona solicitada para lote de uma NF-e",
        494 => "Chave de acesso inexistente",
        502 => "Chave de acesso difere da concatenação dos campos",
        537 => "Total do desconto difere do somatório dos itens",
        539 => "Duplicidade de NF-e com diferença na chave de acesso",
        564 => "Total do produto difere do somatório dos itens",
        573 => "Duplicidade de evento",
        588 => "Caracteres de edição entre as tags da mensagem",
        598 => "Razão social do destinatário difere do texto de homologação",
        610 => "Total da NF difere do somatório dos valores",
        629 => "Valor do produto difere de quantidade vezes valor unitário",
        865 => "Total dos pagamentos menor que o total da nota",
        869 => "Valor do troco incorreto",
        1115 => "Grupo IBSCBS não informado",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resposta(body: &str) -> HttpResponse {
        HttpResponse { status: 200, headers: Vec::new(), body: body.as_bytes().to_vec() }
    }

    #[test]
    fn reads_a_status_inside_a_prefixed_envelope() {
        let body = "<env:Envelope xmlns:env=\"http://www.w3.org/2003/05/soap-envelope\"><env:Body><nfeResultMsg xmlns=\"u\"><retConsStatServ versao=\"4.00\"><tpAmb>2</tpAmb><verAplic>SP_NFE</verAplic><cStat>107</cStat><xMotivo>ok</xMotivo><tMed>1</tMed></retConsStatServ></nfeResultMsg></env:Body></env:Envelope>";
        let status = ler_status(&resposta(body)).expect("status");
        assert!(status.em_operacao());
        assert!(!status.retorno.veio_do_sandbox());
        assert_eq!(status.t_med.as_deref(), Some("1"));
    }

    #[test]
    fn soap_fault_comes_as_an_invalid_return() {
        let body = "<e:Envelope xmlns:e=\"u\"><e:Body><e:Fault><e:Reason><e:Text>caiu</e:Text></e:Reason></e:Fault></e:Body></e:Envelope>";
        assert_eq!(
            ler_status(&resposta(body)),
            Err(NfeError::RetornoInvalido { status: 200, motivo: "caiu".to_string() })
        );
    }

    #[test]
    fn garbage_is_an_invalid_return() {
        assert!(matches!(ler_status(&resposta("não é xml")), Err(NfeError::RetornoInvalido { .. })));
        assert!(matches!(ler_status(&resposta("<a/>")), Err(NfeError::RetornoInvalido { .. })));
    }

    #[test]
    fn authorization_decides_by_the_document_status() {
        let body = "<retEnviNFe><tpAmb>2</tpAmb><verAplic>x</verAplic><cStat>104</cStat><xMotivo>lote</xMotivo><protNFe versao=\"4.00\"><infProt><chNFe>1</chNFe><cStat>297</cStat><xMotivo>nao</xMotivo></infProt></protNFe></retEnviNFe>";
        let retorno = ler_autorizacao(&resposta(body), "<NFe/>").expect("return");
        assert_eq!(retorno.c_stat(), 297);
        assert_eq!(retorno.x_motivo(), "nao");
        assert_eq!(retorno.situacao(), Situacao::Rejeitada);
        assert_eq!(retorno.nfe_proc, None);
    }

    #[test]
    fn authorized_document_gets_its_nfe_proc() {
        let body = "<retEnviNFe><cStat>104</cStat><protNFe versao=\"4.00\"><infProt><chNFe>1</chNFe><nProt>9</nProt><cStat>100</cStat><xMotivo>sim</xMotivo></infProt></protNFe></retEnviNFe>";
        let retorno = ler_autorizacao(&resposta(body), "<NFe/>").expect("return");
        assert_eq!(retorno.situacao(), Situacao::Autorizada);
        let proc_text = retorno.nfe_proc.expect("proc");
        assert!(proc_text.contains("<NFe/><protNFe versao=\"4.00\">"));
    }

    #[test]
    fn batch_refusal_has_no_protocol() {
        let body = "<retEnviNFe><cStat>108</cStat><xMotivo>parado</xMotivo></retEnviNFe>";
        let retorno = ler_autorizacao(&resposta(body), "<NFe/>").expect("return");
        assert_eq!(retorno.c_stat(), 108);
        assert_eq!(retorno.situacao(), Situacao::Paralisado);
    }

    #[test]
    fn event_status_comes_from_inside() {
        let body = "<retEnvEvento><cStat>128</cStat><xMotivo>lote</xMotivo><retEvento versao=\"1.00\"><infEvento><cStat>135</cStat><xMotivo>ok</xMotivo><chNFe>1</chNFe><nProt>2</nProt></infEvento></retEvento></retEnvEvento>";
        let retorno = ler_evento(&resposta(body)).expect("return");
        assert!(retorno.registrado());
        assert_eq!(retorno.lote.c_stat, 128);
    }
}
