//! Writes the XML of a document and of the messages, deterministically: same input, same bytes.
//!
//! No spaces or line breaks between tags, `xmlns` on `NFe`, `Id` as `NFe` plus the key, and the
//! tags in the order of `leiauteNFe_v4.00.xsd`.

use rust_decimal::Decimal;
use sha1::{Digest, Sha1};

use crate::ambiente::{Ambiente, Modelo, Uf};
use crate::chave::{CamposChave, ChaveAcesso};
use crate::error::NfeError;
use crate::modelo::{
    Csosn, Destinatario, DocumentoDestinatario, Emitente, Endereco, FormaPagamento, Item, Rascunho,
    TEXTO_HOMOLOGACAO_DESTINATARIO, TEXTO_HOMOLOGACAO_ITEM, TEXTO_POC_BETA,
};
use crate::soap::NS_NFE;
use crate::totais::Totais;
use crate::xml::{escape_attr, escape_text};

/// The value of `verProc`. Every document says which software it came from.
pub const VER_PROC: &str = "RAVENRS-POC-BETA";

/// The kind of issue: always normal, since contingency is out of this POC.
const TP_EMIS: u8 = 1;

/// How the QR Code of an NFC-e is built.
#[derive(Clone, PartialEq, Eq)]
pub enum QrCode {
    /// Version 3 online: `?p=key|3|tpAmb`, which needs no CSC.
    V3,
    /// Version 2 online: `?p=key|2|tpAmb|cIdToken|hash`, with the taxpayer's CSC.
    V2 {
        /// The CSC identifier, without leading zeros.
        id_token: String,
        /// The CSC. It only enters the hash.
        csc: String,
    },
}

impl std::fmt::Debug for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V3 => write!(f, "V3"),
            Self::V2 { id_token, .. } => {
                f.debug_struct("V2").field("id_token", id_token).field("csc", &"<oculto>").finish()
            }
        }
    }
}

impl QrCode {
    /// The text of the QR Code for a key.
    #[must_use]
    pub fn texto(&self, url: &str, chave: &ChaveAcesso, ambiente: Ambiente) -> String {
        let tp_amb = ambiente.tp_amb();
        match self {
            Self::V3 => format!("{url}?p={chave}|3|{tp_amb}"),
            Self::V2 { id_token, csc } => {
                let base = format!("{chave}|2|{tp_amb}|{id_token}");
                let hash = Sha1::digest(format!("{base}{csc}").as_bytes());
                let hex: String = hash.iter().map(|byte| format!("{byte:02X}")).collect();
                format!("{url}?p={base}|{hex}")
            }
        }
    }
}

/// Everything needed to write a document.
pub(crate) struct Entrada<'a> {
    pub ambiente: Ambiente,
    pub emitente: &'a Emitente,
    pub rascunho: &'a Rascunho,
    pub totais: &'a Totais,
    pub chave: &'a ChaveAcesso,
    pub c_nf: &'a str,
    /// Text of the QR Code and `urlChave`, for an NFC-e.
    pub suplemento: Option<(&'a str, &'a str)>,
}

/// The `cNF` of a draft: the one given, or one derived from the document itself.
///
/// It comes from a hash so the same draft always gives the same key and the crate needs no
/// random source, which keeps it deterministic and working in WebAssembly.
#[must_use]
pub fn codigo_numerico(rascunho: &Rascunho, cnpj: &str) -> String {
    if let Some(codigo) = &rascunho.codigo_numerico {
        return codigo.clone();
    }
    let semente = format!(
        "{cnpj}|{}|{}|{}|{}",
        rascunho.modelo.codigo(),
        rascunho.serie,
        rascunho.numero,
        rascunho.emitido_em
    );
    let hash = Sha1::digest(semente.as_bytes());
    let valor = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 100_000_000;
    let valor = if valor == rascunho.numero { (valor + 1) % 100_000_000 } else { valor };
    format!("{valor:08}")
}

/// The key of a draft.
pub fn chave_de(rascunho: &Rascunho, emitente: &Emitente, c_nf: &str) -> Result<ChaveAcesso, NfeError> {
    let dh = rascunho.emitido_em.as_str();
    let aamm = format!("{}{}", dh.get(2..4).unwrap_or(""), dh.get(5..7).unwrap_or(""));
    ChaveAcesso::montar(&CamposChave {
        uf: emitente.endereco.uf,
        aamm: &aamm,
        cnpj: &emitente.cnpj,
        modelo: rascunho.modelo,
        serie: rascunho.serie,
        numero: rascunho.numero,
        tp_emis: TP_EMIS,
        c_nf,
    })
}

fn tag(out: &mut String, name: &str, value: &str) {
    out.push('<');
    out.push_str(name);
    out.push('>');
    out.push_str(&escape_text(value));
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn open(out: &mut String, name: &str) {
    out.push('<');
    out.push_str(name);
    out.push('>');
}

fn close(out: &mut String, name: &str) {
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn dinheiro(valor: Decimal) -> String {
    format!("{valor:.2}")
}

fn quantidade(valor: Decimal) -> String {
    format!("{valor:.4}")
}

fn preco(valor: Decimal) -> String {
    let casas = valor.normalize().scale().max(2) as usize;
    format!("{valor:.casas$}")
}

/// Writes `<NFe>` with `infNFe` and, in an NFC-e, `infNFeSupl`, and no signature yet.
pub(crate) fn escrever_nfe(entrada: &Entrada<'_>) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<NFe xmlns=\"");
    out.push_str(NS_NFE);
    out.push_str("\"><infNFe Id=\"NFe");
    out.push_str(entrada.chave.as_str());
    out.push_str("\" versao=\"4.00\">");
    escrever_ide(&mut out, entrada);
    escrever_emit(&mut out, entrada.emitente);
    if let Some(destinatario) = &entrada.rascunho.destinatario {
        escrever_dest(&mut out, destinatario, entrada.rascunho.modelo);
    }
    for (posicao, item) in entrada.rascunho.itens.iter().enumerate() {
        escrever_det(&mut out, entrada, posicao, item);
    }
    escrever_total(&mut out, entrada.totais);
    out.push_str("<transp><modFrete>9</modFrete></transp>");
    escrever_pag(&mut out, entrada);
    escrever_inf_adic(&mut out, entrada.rascunho);
    if let Some(responsavel) = &entrada.rascunho.responsavel_tecnico {
        open(&mut out, "infRespTec");
        tag(&mut out, "CNPJ", &responsavel.cnpj);
        tag(&mut out, "xContato", &responsavel.contato);
        tag(&mut out, "email", &responsavel.email);
        tag(&mut out, "fone", &responsavel.fone);
        close(&mut out, "infRespTec");
    }
    out.push_str("</infNFe>");
    if let Some((qr, url_chave)) = entrada.suplemento {
        open(&mut out, "infNFeSupl");
        tag(&mut out, "qrCode", qr);
        tag(&mut out, "urlChave", url_chave);
        close(&mut out, "infNFeSupl");
    }
    out.push_str("</NFe>");
    out
}

fn id_dest(emitente: &Emitente, rascunho: &Rascunho) -> &'static str {
    let destino = rascunho.destinatario.as_ref().and_then(|d| d.endereco.as_ref());
    match destino {
        Some(endereco) if endereco.uf != emitente.endereco.uf => "2",
        _ => "1",
    }
}

fn escrever_ide(out: &mut String, entrada: &Entrada<'_>) {
    let rascunho = entrada.rascunho;
    let emitente = entrada.emitente;
    open(out, "ide");
    tag(out, "cUF", emitente.endereco.uf.codigo_ibge());
    tag(out, "cNF", entrada.c_nf);
    tag(out, "natOp", &rascunho.natureza);
    tag(out, "mod", rascunho.modelo.codigo());
    tag(out, "serie", &rascunho.serie.to_string());
    tag(out, "nNF", &rascunho.numero.to_string());
    tag(out, "dhEmi", &rascunho.emitido_em);
    tag(out, "tpNF", "1");
    tag(out, "idDest", id_dest(emitente, rascunho));
    tag(out, "cMunFG", &emitente.endereco.codigo_municipio);
    tag(out, "tpImp", if rascunho.modelo == Modelo::Nfce65 { "4" } else { "1" });
    tag(out, "tpEmis", &TP_EMIS.to_string());
    tag(out, "cDV", entrada.chave.dv());
    tag(out, "tpAmb", entrada.ambiente.tp_amb());
    tag(out, "finNFe", "1");
    tag(out, "indFinal", "1");
    tag(out, "indPres", "1");
    tag(out, "procEmi", "0");
    tag(out, "verProc", VER_PROC);
    close(out, "ide");
}

fn escrever_endereco(out: &mut String, name: &str, endereco: &Endereco) {
    open(out, name);
    tag(out, "xLgr", &endereco.logradouro);
    tag(out, "nro", &endereco.numero);
    tag(out, "xBairro", &endereco.bairro);
    tag(out, "cMun", &endereco.codigo_municipio);
    tag(out, "xMun", &endereco.municipio);
    tag(out, "UF", endereco.uf.sigla());
    tag(out, "CEP", &endereco.cep);
    tag(out, "cPais", "1058");
    tag(out, "xPais", "BRASIL");
    close(out, name);
}

fn escrever_emit(out: &mut String, emitente: &Emitente) {
    open(out, "emit");
    tag(out, "CNPJ", &emitente.cnpj);
    tag(out, "xNome", &emitente.razao_social);
    if let Some(fantasia) = &emitente.nome_fantasia {
        tag(out, "xFant", fantasia);
    }
    escrever_endereco(out, "enderEmit", &emitente.endereco);
    tag(out, "IE", &emitente.inscricao_estadual);
    tag(out, "CRT", "1");
    close(out, "emit");
}

fn escrever_dest(out: &mut String, destinatario: &Destinatario, modelo: Modelo) {
    open(out, "dest");
    match &destinatario.documento {
        DocumentoDestinatario::Cnpj(cnpj) => tag(out, "CNPJ", cnpj),
        DocumentoDestinatario::Cpf(cpf) => tag(out, "CPF", cpf),
    }
    match modelo {
        // Both environments of this crate are tpAmb=2, so the required text is always the one.
        Modelo::Nfe55 => tag(out, "xNome", TEXTO_HOMOLOGACAO_DESTINATARIO),
        Modelo::Nfce65 => {
            if let Some(nome) = &destinatario.nome {
                tag(out, "xNome", nome);
            }
        }
    }
    if let Some(endereco) = &destinatario.endereco {
        escrever_endereco(out, "enderDest", endereco);
    }
    tag(out, "indIEDest", "9");
    close(out, "dest");
}

fn escrever_det(out: &mut String, entrada: &Entrada<'_>, posicao: usize, item: &Item) {
    let totais = entrada.totais.itens[posicao];
    let modelo = entrada.rascunho.modelo;
    let gtin = item.gtin.as_deref().unwrap_or("SEM GTIN");
    let descricao = if modelo == Modelo::Nfce65 && posicao == 0 {
        TEXTO_HOMOLOGACAO_ITEM
    } else {
        item.descricao.as_str()
    };
    out.push_str("<det nItem=\"");
    out.push_str(&escape_attr(&(posicao + 1).to_string()));
    out.push_str("\">");
    open(out, "prod");
    tag(out, "cProd", &item.codigo);
    tag(out, "cEAN", gtin);
    tag(out, "xProd", descricao);
    tag(out, "NCM", &item.ncm);
    if let Some(cest) = &item.cest {
        tag(out, "CEST", cest);
    }
    tag(out, "CFOP", &item.cfop);
    tag(out, "uCom", &item.unidade);
    tag(out, "qCom", &quantidade(item.quantidade));
    tag(out, "vUnCom", &preco(item.valor_unitario));
    tag(out, "vProd", &dinheiro(totais.v_prod));
    tag(out, "cEANTrib", gtin);
    tag(out, "uTrib", &item.unidade);
    tag(out, "qTrib", &quantidade(item.quantidade));
    tag(out, "vUnTrib", &preco(item.valor_unitario));
    if !totais.v_desc.is_zero() {
        tag(out, "vDesc", &dinheiro(totais.v_desc));
    }
    tag(out, "indTot", "1");
    close(out, "prod");

    open(out, "imposto");
    open(out, "ICMS");
    let (grupo, csosn) = match item.csosn {
        Csosn::S102 => ("ICMSSN102", "102"),
        Csosn::S500 => ("ICMSSN500", "500"),
    };
    open(out, grupo);
    tag(out, "orig", &item.origem.to_string());
    tag(out, "CSOSN", csosn);
    close(out, grupo);
    close(out, "ICMS");
    // The groups are optional in the schema and the NFC-e of the Simples leaves them out.
    if modelo == Modelo::Nfe55 {
        out.push_str(
            "<PIS><PISOutr><CST>99</CST><vBC>0.00</vBC><pPIS>0.00</pPIS><vPIS>0.00</vPIS></PISOutr></PIS>",
        );
        out.push_str("<COFINS><COFINSOutr><CST>99</CST><vBC>0.00</vBC><pCOFINS>0.00</pCOFINS><vCOFINS>0.00</vCOFINS></COFINSOutr></COFINS>");
    }
    close(out, "imposto");
    close(out, "det");
}

fn escrever_total(out: &mut String, totais: &Totais) {
    const ZERO: &str = "0.00";
    open(out, "total");
    open(out, "ICMSTot");
    for name in ["vBC", "vICMS", "vICMSDeson", "vFCP", "vBCST", "vST", "vFCPST", "vFCPSTRet"] {
        tag(out, name, ZERO);
    }
    tag(out, "vProd", &dinheiro(totais.v_prod));
    tag(out, "vFrete", ZERO);
    tag(out, "vSeg", ZERO);
    tag(out, "vDesc", &dinheiro(totais.v_desc));
    for name in ["vII", "vIPI", "vIPIDevol", "vPIS", "vCOFINS", "vOutro"] {
        tag(out, name, ZERO);
    }
    tag(out, "vNF", &dinheiro(totais.v_nf));
    close(out, "ICMSTot");
    close(out, "total");
}

fn escrever_pag(out: &mut String, entrada: &Entrada<'_>) {
    open(out, "pag");
    for pagamento in &entrada.rascunho.pagamentos {
        open(out, "detPag");
        tag(out, "tPag", pagamento.forma.codigo());
        if pagamento.forma == FormaPagamento::Outros {
            tag(out, "xPag", "OUTROS");
        }
        tag(out, "vPag", &dinheiro(pagamento.valor));
        if matches!(
            pagamento.forma,
            FormaPagamento::CartaoCredito | FormaPagamento::CartaoDebito | FormaPagamento::Pix
        ) {
            // 2: the payment did not go through the emitter's own integration.
            out.push_str("<card><tpIntegra>2</tpIntegra></card>");
        }
        close(out, "detPag");
    }
    if !entrada.totais.v_troco.is_zero() {
        tag(out, "vTroco", &dinheiro(entrada.totais.v_troco));
    }
    close(out, "pag");
}

fn escrever_inf_adic(out: &mut String, rascunho: &Rascunho) {
    open(out, "infAdic");
    let texto = match &rascunho.observacao {
        Some(observacao) => format!("{TEXTO_POC_BETA} {observacao}"),
        None => TEXTO_POC_BETA.to_string(),
    };
    tag(out, "infCpl", &texto);
    close(out, "infAdic");
}

/// The batch of one document, always synchronous: `indSinc` has no other value in this crate.
pub fn lote(id_lote: u64, nfe_assinada: &str) -> String {
    format!(
        "<enviNFe xmlns=\"{NS_NFE}\" versao=\"4.00\"><idLote>{id_lote}</idLote><indSinc>1</indSinc>{nfe_assinada}</enviNFe>"
    )
}

/// The message of the service status.
#[must_use]
pub fn consulta_status(uf: Uf, ambiente: Ambiente) -> String {
    format!(
        "<consStatServ xmlns=\"{NS_NFE}\" versao=\"4.00\"><tpAmb>{}</tpAmb><cUF>{}</cUF><xServ>STATUS</xServ></consStatServ>",
        ambiente.tp_amb(),
        uf.codigo_ibge()
    )
}

/// The message of the protocol query.
#[must_use]
pub fn consulta_protocolo(chave: &ChaveAcesso, ambiente: Ambiente) -> String {
    format!(
        "<consSitNFe xmlns=\"{NS_NFE}\" versao=\"4.00\"><tpAmb>{}</tpAmb><xServ>CONSULTAR</xServ><chNFe>{chave}</chNFe></consSitNFe>",
        ambiente.tp_amb()
    )
}

/// The event code of cancellation.
pub const TP_EVENTO_CANCELAMENTO: &str = "110111";

/// The `Id` of a cancellation event.
#[must_use]
pub fn id_cancelamento(chave: &ChaveAcesso, sequencia: u8) -> String {
    format!("ID{TP_EVENTO_CANCELAMENTO}{chave}{sequencia:02}")
}

/// The data of a cancellation event.
pub(crate) struct DadosCancelamento<'a> {
    pub ambiente: Ambiente,
    pub uf: Uf,
    pub cnpj: &'a str,
    pub chave: &'a ChaveAcesso,
    pub protocolo: &'a str,
    pub justificativa: &'a str,
    pub quando: &'a str,
    pub sequencia: u8,
}

/// Writes the `evento` of a cancellation, without its signature.
pub(crate) fn evento_cancelamento(dados: &DadosCancelamento<'_>) -> String {
    let mut out = String::with_capacity(512);
    out.push_str("<evento xmlns=\"");
    out.push_str(NS_NFE);
    out.push_str("\" versao=\"1.00\"><infEvento Id=\"");
    out.push_str(&id_cancelamento(dados.chave, dados.sequencia));
    out.push_str("\">");
    tag(&mut out, "cOrgao", dados.uf.codigo_ibge());
    tag(&mut out, "tpAmb", dados.ambiente.tp_amb());
    tag(&mut out, "CNPJ", dados.cnpj);
    tag(&mut out, "chNFe", dados.chave.as_str());
    tag(&mut out, "dhEvento", dados.quando);
    tag(&mut out, "tpEvento", TP_EVENTO_CANCELAMENTO);
    tag(&mut out, "nSeqEvento", &dados.sequencia.to_string());
    tag(&mut out, "verEvento", "1.00");
    out.push_str("<detEvento versao=\"1.00\">");
    tag(&mut out, "descEvento", "Cancelamento");
    tag(&mut out, "nProt", dados.protocolo);
    tag(&mut out, "xJust", dados.justificativa);
    out.push_str("</detEvento></infEvento></evento>");
    out
}

/// The batch of one event.
#[must_use]
pub fn lote_evento(id_lote: u64, evento_assinado: &str) -> String {
    format!(
        "<envEvento xmlns=\"{NS_NFE}\" versao=\"1.00\"><idLote>{id_lote}</idLote>{evento_assinado}</envEvento>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_quantity_and_price_have_the_layout_places() {
        let d = |t: &str| t.parse::<Decimal>().expect("decimal");
        assert_eq!(dinheiro(d("1.5")), "1.50");
        assert_eq!(quantidade(d("2")), "2.0000");
        assert_eq!(preco(d("3")), "3.00");
        assert_eq!(preco(d("3.1234500")), "3.12345");
    }

    #[test]
    fn qr_code_v3_needs_no_csc() {
        let chave = ChaveAcesso::montar(&CamposChave {
            uf: Uf::SP,
            aamm: "2611",
            cnpj: "11222333000181",
            modelo: Modelo::Nfce65,
            serie: 1,
            numero: 1,
            tp_emis: 1,
            c_nf: "12345678",
        })
        .expect("key");
        let texto = QrCode::V3.texto("https://x/qrcode", &chave, Ambiente::Sandbox);
        assert_eq!(texto, format!("https://x/qrcode?p={chave}|3|2"));
    }

    #[test]
    fn qr_code_v2_hashes_the_csc_and_hides_it() {
        let chave = ChaveAcesso::montar(&CamposChave {
            uf: Uf::SP,
            aamm: "2611",
            cnpj: "11222333000181",
            modelo: Modelo::Nfce65,
            serie: 1,
            numero: 1,
            tp_emis: 1,
            c_nf: "12345678",
        })
        .expect("key");
        let qr = QrCode::V2 { id_token: "1".into(), csc: "SEGREDO".into() };
        let texto = qr.texto("https://x/q", &chave, Ambiente::Homologacao);
        assert!(!texto.contains("SEGREDO"));
        let parts: Vec<&str> = texto.split('|').collect();
        assert_eq!(parts[1..4], ["2", "2", "1"]);
        assert_eq!(parts[4].len(), 40);
        assert!(!format!("{qr:?}").contains("SEGREDO"));
    }
}
