//! Writes the answers of the sandbox in the real SOAP format.

use crate::soap::{NS_NFE, NS_SOAP12, Servico};
use crate::transporte::{HttpResponse, TransportError};
use crate::xml::{Element, escape_text};

use super::{PREFIXO_MOTIVO, Resposta, SandboxRequest, VER_APLIC};

/// The codes that stop at the batch, with no `protNFe` or `retEvento` inside.
const SO_NO_LOTE: [u32; 14] = [108, 109, 126, 129, 215, 225, 239, 243, 452, 516, 517, 565, 587, 588];

/// The fallback state when a message does not say which one it is from.
const UF_PADRAO: &str = "35";

fn motivo(texto: &str) -> String {
    escape_text(&format!("{PREFIXO_MOTIVO}{texto}"))
}

/// Wraps a return in the SOAP 1.2 envelope of a service.
pub(super) fn soap(servico: Servico, retorno: &str) -> HttpResponse {
    let corpo = if servico == Servico::Distribuicao {
        format!(
            "<nfeDistDFeInteresseResponse xmlns=\"{}\"><nfeDistDFeInteresseResult>{retorno}</nfeDistDFeInteresseResult></nfeDistDFeInteresseResponse>",
            servico.namespace()
        )
    } else {
        format!("<nfeResultMsg xmlns=\"{}\">{retorno}</nfeResultMsg>", servico.namespace())
    };
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"{NS_SOAP12}\"><soap:Body>{corpo}</soap:Body></soap:Envelope>"
    );
    HttpResponse {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/soap+xml; charset=utf-8".to_string())],
        body: body.into_bytes(),
    }
}

/// A SOAP 1.2 fault, which is how a server refuses an envelope it cannot route.
pub(super) fn fault(status: u16, texto: &str) -> HttpResponse {
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"{NS_SOAP12}\"><soap:Body><soap:Fault><soap:Code><soap:Value>soap:Sender</soap:Value></soap:Code><soap:Reason><soap:Text xml:lang=\"pt-BR\">{}</soap:Text></soap:Reason></soap:Fault></soap:Body></soap:Envelope>",
        motivo(texto)
    );
    HttpResponse {
        status,
        headers: vec![("Content-Type".to_string(), "application/soap+xml; charset=utf-8".to_string())],
        body: body.into_bytes(),
    }
}

pub(super) struct DadosProtocolo<'a> {
    pub chave: &'a str,
    pub dh: &'a str,
    pub n_prot: Option<&'a str>,
    pub dig_val: Option<&'a str>,
    pub c_stat: u32,
    pub motivo: &'a str,
}

pub(super) fn prot_nfe(dados: &DadosProtocolo<'_>) -> String {
    let n_prot = dados.n_prot.map_or(String::new(), |n| format!("<nProt>{n}</nProt>"));
    let dig_val = dados.dig_val.map_or(String::new(), |d| format!("<digVal>{d}</digVal>"));
    format!(
        "<protNFe versao=\"4.00\"><infProt><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><chNFe>{}</chNFe><dhRecbto>{}</dhRecbto>{n_prot}{dig_val}<cStat>{}</cStat><xMotivo>{}</xMotivo></infProt></protNFe>",
        dados.chave,
        dados.dh,
        dados.c_stat,
        motivo(dados.motivo)
    )
}

pub(super) fn ret_lote(c_stat: u32, texto: &str, c_uf: &str, dh: &str, prot: Option<&str>) -> String {
    format!(
        "<retEnviNFe xmlns=\"{NS_NFE}\" versao=\"4.00\"><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cStat>{c_stat}</cStat><xMotivo>{}</xMotivo><cUF>{c_uf}</cUF><dhRecbto>{dh}</dhRecbto>{}</retEnviNFe>",
        motivo(texto),
        prot.unwrap_or("")
    )
}

pub(super) fn ret_status(c_stat: u32, texto: &str, c_uf: &str, dh: &str) -> String {
    format!(
        "<retConsStatServ xmlns=\"{NS_NFE}\" versao=\"4.00\"><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cStat>{c_stat}</cStat><xMotivo>{}</xMotivo><cUF>{c_uf}</cUF><dhRecbto>{dh}</dhRecbto><tMed>1</tMed></retConsStatServ>",
        motivo(texto)
    )
}

pub(super) fn ret_consulta(
    c_stat: u32,
    texto: &str,
    c_uf: &str,
    dh: &str,
    chave: &str,
    prot: Option<&str>,
) -> String {
    format!(
        "<retConsSitNFe xmlns=\"{NS_NFE}\" versao=\"4.00\"><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cStat>{c_stat}</cStat><xMotivo>{}</xMotivo><cUF>{c_uf}</cUF><dhRecbto>{dh}</dhRecbto><chNFe>{chave}</chNFe>{}</retConsSitNFe>",
        motivo(texto),
        prot.unwrap_or("")
    )
}

/// The return of the distribution service, each document compressed with GZip.
pub(super) fn ret_distribuicao(
    c_stat: u32,
    texto: &str,
    dh: &str,
    ult_nsu: u64,
    max_nsu: u64,
    documentos: &[(u64, String, String)],
) -> String {
    use std::io::Write;

    use base64::Engine;

    let mut lote = String::new();
    if !documentos.is_empty() {
        lote.push_str("<loteDistDFeInt>");
        for (nsu, schema, xml) in documentos {
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            let comprimido = gz
                .write_all(xml.as_bytes())
                .and_then(|()| gz.try_finish())
                .map(|()| gz.get_ref().clone())
                .unwrap_or_default();
            let base = base64::engine::general_purpose::STANDARD.encode(comprimido);
            lote.push_str(&format!("<docZip NSU=\"{nsu:015}\" schema=\"{schema}\">{base}</docZip>"));
        }
        lote.push_str("</loteDistDFeInt>");
    }
    format!(
        "<retDistDFeInt xmlns=\"{NS_NFE}\" versao=\"1.01\"><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cStat>{c_stat}</cStat><xMotivo>{}</xMotivo><dhResp>{dh}</dhResp><ultNSU>{ult_nsu:015}</ultNSU><maxNSU>{max_nsu:015}</maxNSU>{lote}</retDistDFeInt>",
        motivo(texto)
    )
}

pub(super) struct DadosEvento<'a> {
    pub c_orgao: &'a str,
    pub c_stat: u32,
    pub motivo: &'a str,
    pub chave: &'a str,
    pub tp_evento: &'a str,
    pub sequencia: &'a str,
    pub dh: &'a str,
    pub n_prot: Option<&'a str>,
}

pub(super) fn ret_evento(dados: &DadosEvento<'_>) -> String {
    let registro = dados
        .n_prot
        .map_or(String::new(), |n| format!("<dhRegEvento>{}</dhRegEvento><nProt>{n}</nProt>", dados.dh));
    let descricao = if dados.n_prot.is_some() { "<xEvento>Cancelamento registrado</xEvento>" } else { "" };
    format!(
        "<retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cOrgao>{}</cOrgao><cStat>{}</cStat><xMotivo>{}</xMotivo><chNFe>{}</chNFe><tpEvento>{}</tpEvento>{descricao}<nSeqEvento>{}</nSeqEvento>{registro}</infEvento></retEvento>",
        dados.c_orgao,
        dados.c_stat,
        motivo(dados.motivo),
        dados.chave,
        dados.tp_evento,
        dados.sequencia,
    )
}

pub(super) fn ret_lote_evento(
    id_lote: &str,
    c_stat: u32,
    texto: &str,
    c_orgao: &str,
    evento: Option<&str>,
) -> String {
    format!(
        "<retEnvEvento xmlns=\"{NS_NFE}\" versao=\"1.00\"><idLote>{id_lote}</idLote><tpAmb>2</tpAmb><verAplic>{VER_APLIC}</verAplic><cOrgao>{c_orgao}</cOrgao><cStat>{c_stat}</cStat><xMotivo>{}</xMotivo>{}</retEnvEvento>",
        motivo(texto),
        evento.unwrap_or("")
    )
}

fn uf_de(dados: Option<&Element>) -> String {
    let Some(dados) = dados else { return UF_PADRAO.to_string() };
    dados
        .find_text("cUF")
        .or_else(|| dados.find_text("cOrgao"))
        .or_else(|| dados.find_text("chNFe").and_then(|c| c.get(..2).map(str::to_string)))
        .unwrap_or_else(|| UF_PADRAO.to_string())
}

/// The key of a message, or a valid-looking one made of zeros when it has none.
fn chave_de(dados: Option<&Element>) -> String {
    let id = dados
        .and_then(|d| d.find("infNFe"))
        .and_then(|inf| inf.attr("Id"))
        .and_then(|id| id.strip_prefix("NFe"))
        .map(str::to_string);
    id.or_else(|| dados.and_then(|d| d.find_text("chNFe")))
        .filter(|chave| crate::chave::forma_valida(chave))
        .unwrap_or_else(|| "0".repeat(44))
}

/// Answers a `cStat` in the format of the service of the request.
pub(super) fn responder(pedido: &SandboxRequest, c_stat: u32, texto: &str) -> Resposta {
    let Some(servico) = pedido.servico else {
        return Resposta::Erro(TransportError::Outro(
            "um handler só responde um cStat em um dos quatro serviços".to_string(),
        ));
    };
    let dados = pedido.dados();
    let dados = dados.as_ref();
    let c_uf = uf_de(dados);
    let dh = pedido.dh_recbto.as_str();
    let chave = chave_de(dados);
    let retorno = match servico {
        Servico::Status => ret_status(c_stat, texto, &c_uf, dh),
        Servico::Autorizacao if SO_NO_LOTE.contains(&c_stat) => ret_lote(c_stat, texto, &c_uf, dh, None),
        Servico::Autorizacao => {
            let n_prot = matches!(c_stat, 100 | 150).then(|| pedido.proximo_protocolo());
            let dig_val = dados.and_then(|d| d.find_text("DigestValue"));
            let prot = prot_nfe(&DadosProtocolo {
                chave: &chave,
                dh,
                n_prot: n_prot.as_deref(),
                dig_val: dig_val.as_deref(),
                c_stat,
                motivo: texto,
            });
            ret_lote(104, "Lote processado", &c_uf, dh, Some(&prot))
        }
        Servico::Consulta => ret_consulta(c_stat, texto, &c_uf, dh, &chave, None),
        Servico::Distribuicao => ret_distribuicao(c_stat, texto, dh, 0, 0, &[]),
        Servico::Evento => {
            let id_lote = dados.and_then(|d| d.child_text("idLote")).unwrap_or_else(|| "1".to_string());
            if SO_NO_LOTE.contains(&c_stat) {
                ret_lote_evento(&id_lote, c_stat, texto, &c_uf, None)
            } else {
                let sequencia =
                    dados.and_then(|d| d.find_text("nSeqEvento")).unwrap_or_else(|| "1".to_string());
                let n_prot = matches!(c_stat, 135 | 136 | 155).then(|| pedido.proximo_protocolo());
                let evento = ret_evento(&DadosEvento {
                    c_orgao: &c_uf,
                    c_stat,
                    motivo: texto,
                    chave: &chave,
                    tp_evento: &dados
                        .and_then(|d| d.find_text("tpEvento"))
                        .unwrap_or_else(|| "110111".to_string()),
                    sequencia: &sequencia,
                    dh,
                    n_prot: n_prot.as_deref(),
                });
                ret_lote_evento(&id_lote, 128, "Lote de evento processado", &c_uf, Some(&evento))
            }
        }
    };
    Resposta::Http(soap(servico, &retorno))
}
