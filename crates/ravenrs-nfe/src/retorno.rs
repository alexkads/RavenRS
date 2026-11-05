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

/// The name of a `cStat`, condensed from the code table of the MOC 7.0 (Anexo I, 4.4.1) and of the
/// technical notes that created later codes. The server's own `xMotivo` is what counts.
#[must_use]
pub const fn descricao_c_stat(c_stat: u32) -> Option<&'static str> {
    Some(match c_stat {
        100 => "Autorizado o uso da NF-e",
        101 => "Cancelamento de NF-e homologado",
        104 => "Lote processado",
        107 => "Serviço em Operação",
        108 => "Serviço Paralisado Momentaneamente (curto prazo)",
        109 => "Serviço Paralisado sem Previsão",
        126 => "Rejeição: Enviado lote com mais de 1 NFC-e",
        128 => "Lote de Evento Processado",
        129 => "Rejeição: A autorização deve ser solicitada de forma síncrona",
        135 => "Evento registrado e vinculado a NF-e",
        136 => "Evento registrado, mas não vinculado a NF-e",
        137 => "Nenhum documento localizado para o Destinatário",
        138 => "Documento localizado para o Destinatário",
        150 => "Autorizado o uso da NF-e, autorização fora de prazo",
        155 => "Cancelamento homologado fora de prazo",
        204 => "Rejeição: Duplicidade de NF-e",
        207 => "Rejeição: CNPJ do emitente inválido",
        213 => "Rejeição: CNPJ-Base do Emitente difere do CNPJ-Base do Certificado Digital",
        215 => "Rejeição: Falha no schema XML",
        217 => "Rejeição: NF-e não consta na base de dados da SEFAZ",
        222 => "Rejeição: Protocolo de Autorização de Uso difere do cadastrado",
        225 => "Rejeição: Falha no Schema XML do lote de NFe",
        227 => "Rejeição: CPF do Emitente difere do CPF do Certificado Digital",
        236 => "Rejeição: Chave de Acesso com dígito verificador inválido",
        239 => "Rejeição: Versão do arquivo XML não suportada",
        243 => "Rejeição: XML Mal Formado",
        252 => "Rejeição: Ambiente informado diverge do Ambiente de recebimento",
        253 => "Rejeição: Digito Verificador da chave de acesso composta inválida",
        271 => "Rejeição: Código Município do Fato Gerador difere da UF do emitente",
        280 => "Rejeição: Certificado Transmissor inválido",
        281 => "Rejeição: Certificado Transmissor Data Validade",
        282 => "Rejeição: Certificado Transmissor sem CNPJ/CPF",
        283 => "Rejeição: Certificado Transmissor - erro Cadeia de Certificação",
        284 => "Rejeição: Certificado Transmissor revogado",
        285 => "Rejeição: Certificado Transmissor difere ICP-Brasil",
        286 => "Rejeição: Certificado Transmissor erro no acesso a LCR",
        290 => "Rejeição: Certificado Assinatura inválido",
        291 => "Rejeição: Certificado Assinatura Data Validade",
        292 => "Rejeição: Certificado de Assinatura sem CNPJ/CPF",
        293 => "Rejeição: Certificado Assinatura - erro Cadeia de Certificação",
        294 => "Rejeição: Certificado Assinatura revogado",
        295 => "Rejeição: Certificado Assinatura difere ICP-Brasil",
        296 => "Rejeição: Certificado Assinatura erro no acesso a LCR",
        297 => "Rejeição: Assinatura difere do calculado",
        298 => "Rejeição: Assinatura difere do padrão do Sistema",
        373 => "Rejeição: Descrição do primeiro item diferente do texto de homologação",
        391 => "Rejeição: Não informados os dados do cartão nas Formas de Pagamento",
        392 => "Rejeição: Não informados os dados da operação de pagamento por cartão",
        394 => "Rejeição: Nota Fiscal sem a informação do QR-Code",
        397 => "Rejeição: Parâmetro do QR-Code divergente da Nota Fiscal",
        398 => "Rejeição: Parâmetro Versão informada no QR-Code diverge do previsto",
        437 => "Rejeição: CNPJ da instituição de pagamento inválido",
        452 => "Rejeição: Solicitada resposta assíncrona para Lote com somente 1 NF-e",
        489 => "Rejeição: CNPJ informado inválido",
        491 => "Rejeição: O tpEvento informado inválido",
        492 => "Rejeição: O verEvento informado inválido",
        493 => "Rejeição: Evento não atende o Schema XML específico",
        494 => "Rejeição: Chave de Acesso inexistente",
        501 => "Rejeição: Prazo de cancelamento superior ao previsto na Legislação",
        502 => "Rejeição: Chave de Acesso do campo Id difere da concatenação dos campos",
        516 => "Rejeição: Falha no schema XML – inexiste a tag raiz esperada para a mensagem",
        517 => "Rejeição: Falha no schema XML – inexiste atributo versao na tag raiz",
        537 => "Rejeição: Total do Desconto difere do somatório dos itens",
        539 => "Rejeição: Duplicidade de NF-e com diferença na Chave de Acesso",
        564 => "Rejeição: Total do Produto / Serviço difere do somatório dos itens",
        565 => "Rejeição: Falha no schema XML – inexiste a tag raiz esperada para o lote",
        573 => "Rejeição: Duplicidade de Evento",
        574 => "Rejeição: O autor do evento diverge do emissor da NF-e",
        575 => "Rejeição: O autor do evento diverge do destinatário da NF-e",
        577 => "Rejeição: A data do evento não pode ser menor que a data de emissão",
        578 => "Rejeição: A data do evento não pode ser maior que a data do processamento",
        579 => "Rejeição: A data do evento não pode ser menor que a data de autorização",
        580 => "Rejeição: O evento exige uma NF-e autorizada",
        587 => "Rejeição: Usar somente o namespace padrão da NF-e",
        588 => "Rejeição: Caracteres de edição no início, no fim ou entre as tags da mensagem",
        589 => "Rejeição: NSU informado superior ao maior NSU do Ambiente Nacional",
        593 => "Rejeição: CNPJ-Base consultado difere do CNPJ-Base do Certificado Digital",
        594 => "Rejeição: O número de sequencia do evento informado é maior que o permitido",
        595 => "Rejeição: Obrigatória a informação da justificativa do evento",
        598 => "Rejeição: Razão Social do destinatário diferente do texto de homologação",
        610 => "Rejeição: Total da NF difere do somatório dos Valores que compõem o Total",
        629 => "Rejeição: Valor do Produto difere de Valor Unitário de Comercialização vezes Quantidade",
        630 => "Rejeição: Valor do Produto difere de Valor Unitário de Tributação vezes Quantidade",
        640 => "Rejeição: CNPJ/CPF do interessado não possui permissão para consultar esta NF-e",
        656 => "Rejeição: Consumo Indevido",
        703 => "Rejeição: Data-Hora de Emissão posterior ao horário de recebimento",
        704 => "Rejeição: NFC-e com Data-Hora de emissão atrasada",
        707 => "Rejeição: NFC-e para operação interestadual ou com o exterior",
        709 => "Rejeição: NFC-e com formato de DANFE inválido",
        710 => "Rejeição: NF-e com formato de DANFE inválido",
        715 => "Rejeição: NFC-e com finalidade inválida",
        716 => "Rejeição: NFC-e em operação não destinada a consumidor final",
        717 => "Rejeição: NFC-e em operação não presencial",
        719 => "Rejeição: NF-e sem a identificação do destinatário",
        724 => "Rejeição: NF-e sem o nome do destinatário",
        725 => "Rejeição: NFC-e com CFOP inválido",
        726 => "Rejeição: NF-e sem a informação de endereço do destinatário",
        789 => "Rejeição: NFC-e para destinatário contribuinte de ICMS",
        796 => "Rejeição: CNPJ recebedor do pagamento inválido",
        806 => "Rejeição: Operação com ICMS-ST sem informação do CEST",
        865 => "Rejeição: Total dos pagamentos menor que o total da nota",
        866 => "Rejeição: Ausência de troco quando os pagamentos passam do total da nota",
        869 => "Rejeição: Valor do troco incorreto",
        878 => "Rejeição: Endereço do site da UF da Consulta por chave de acesso diverge do previsto",
        897 => "Rejeição: Código numérico em formato inválido",
        899 => "Rejeição: Informado incorretamente o campo meio de pagamento",
        904 => "Rejeição: Informado indevidamente campo valor de pagamento",
        963 => "Rejeição: Tipo de pagamento não aceita o grupo de cartões ou boletos",
        1115 => "Rejeição: Grupo IBSCBS não informado",
        _ => return None,
    })
}

/// One document of a distribution batch, already decompressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentoDfe {
    /// The NSU; since NT 2014.002 v1.15 the server may leave it out.
    pub nsu: Option<u64>,
    /// The schema that names the kind: `resNFe_v1.01.xsd`, `procNFe_v4.00.xsd` and so on.
    pub schema: String,
    /// The XML.
    pub xml: String,
}

impl DocumentoDfe {
    /// Whether this is only the summary, which asks for a manifestation before the full XML.
    #[must_use]
    pub fn eh_resumo(&self) -> bool {
        self.schema.starts_with("resNFe")
    }

    /// Whether this is the full document with its protocol.
    #[must_use]
    pub fn eh_nota_completa(&self) -> bool {
        self.schema.starts_with("procNFe")
    }
}

/// The answer of the distribution service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoteDfe {
    /// The last NSU handed over: the next query starts here.
    pub ult_nsu: u64,
    /// The largest NSU there is.
    pub max_nsu: u64,
    /// The documents.
    pub documentos: Vec<DocumentoDfe>,
    /// The status.
    pub c_stat: u32,
    /// The text of the status.
    pub x_motivo: String,
}

impl LoteDfe {
    /// Whether nothing is left to fetch: the next query should wait one hour (NT 2014.002, 3.11.4).
    #[must_use]
    pub fn sem_novidade(&self) -> bool {
        self.c_stat == 137 || (self.c_stat == 138 && self.ult_nsu >= self.max_nsu)
    }
}

/// The largest size a decompressed document may reach. A gzip bomb stops here.
const LIMITE_DOCUMENTO: u64 = 20 * 1024 * 1024;

/// Reads the distribution return, decompressing each document with a limit.
pub fn ler_distribuicao(resposta: &HttpResponse) -> Result<LoteDfe, NfeError> {
    use std::io::Read;

    use base64::Engine;

    let element = corpo(resposta, "retDistDFeInt")?;
    let retorno = comum(&element, resposta.status)?;
    let numero = |tag: &str| element.child_text(tag).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
    let mut documentos = Vec::new();
    if let Some(lote) = element.child("loteDistDFeInt") {
        for zip in lote.elements().filter(|e| e.local_name() == "docZip") {
            let invalido = |motivo: &str| NfeError::RetornoInvalido {
                status: resposta.status,
                motivo: motivo.to_string(),
            };
            let texto: String = zip.text().split_whitespace().collect();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(texto)
                .map_err(|_| invalido("docZip não é base64"))?;
            let mut xml = String::new();
            flate2::read::GzDecoder::new(bytes.as_slice())
                .take(LIMITE_DOCUMENTO + 1)
                .read_to_string(&mut xml)
                .map_err(|_| invalido("docZip não é GZip de texto UTF-8"))?;
            if xml.len() as u64 > LIMITE_DOCUMENTO {
                return Err(invalido("documento descompactado grande demais"));
            }
            documentos.push(DocumentoDfe {
                nsu: zip.attr("NSU").and_then(|n| n.parse().ok()),
                schema: zip.attr("schema").unwrap_or_default().to_string(),
                xml,
            });
        }
    }
    Ok(LoteDfe {
        ult_nsu: numero("ultNSU"),
        max_nsu: numero("maxNSU"),
        documentos,
        c_stat: retorno.c_stat,
        x_motivo: retorno.x_motivo,
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
