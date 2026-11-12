//! Reads a document received from a supplier, for the entry of goods.
//!
//! The text is treated as hostile: the size has a limit, document type declarations are refused
//! so no entity can be defined or fetched, and bad content is an error, never a panic.

use rust_decimal::Decimal;

use crate::ambiente::Modelo;
use crate::chave::forma_valida;
use crate::error::NfeError;
use crate::sandbox::autopsia::{AutopsiaAssinatura, autopsia_assinatura};
use crate::xml::{self, Element};

/// The largest XML accepted: far above any document of the layout, which allows 990 items.
pub const LIMITE_XML: usize = 10 * 1024 * 1024;

/// One line of a received document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRecebido {
    /// `nItem`.
    pub numero: u32,
    /// `cProd`: the supplier's code.
    pub codigo: String,
    /// `cEAN`, with `SEM GTIN` read as none.
    pub gtin: Option<String>,
    /// `xProd`.
    pub descricao: String,
    /// `NCM`.
    pub ncm: String,
    /// `CFOP`.
    pub cfop: String,
    /// `uCom`.
    pub unidade: String,
    /// `qCom`.
    pub quantidade: Decimal,
    /// `vUnCom`.
    pub valor_unitario: Decimal,
    /// `vProd`.
    pub valor_total: Decimal,
    /// `vFrete`, zero when absent.
    pub frete: Decimal,
    /// `vSeg`, zero when absent.
    pub seguro: Decimal,
    /// `vDesc`, zero when absent.
    pub desconto: Decimal,
    /// `vOutro`, zero when absent.
    pub outros: Decimal,
    /// `vICMSST` of any ICMS group, zero when absent.
    pub valor_icms_st: Decimal,
    /// `vIPI`, zero when absent.
    pub valor_ipi: Decimal,
    /// `uTrib`.
    pub unidade_tributavel: String,
    /// `qTrib`.
    pub quantidade_tributavel: Decimal,
    /// `cEANTrib`, with `SEM GTIN` read as none.
    pub gtin_tributavel: Option<String>,
}

/// A document received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotaRecebida {
    /// The access key.
    pub chave: String,
    /// The model.
    pub modelo: Modelo,
    /// `serie`.
    pub serie: u16,
    /// `nNF`.
    pub numero: u32,
    /// `dhEmi`.
    pub emitida_em: String,
    /// The issuer's CNPJ, or CPF for a rural producer.
    pub emitente_cnpj: String,
    /// The issuer's name.
    pub emitente_nome: String,
    /// The recipient's CNPJ or CPF, when there is one.
    pub destinatario_documento: Option<String>,
    /// `nProt`, absent when this is a `NFe` with no protocol.
    pub protocolo: Option<String>,
    /// `vNF`.
    pub total: Decimal,
    /// The lines.
    pub itens: Vec<ItemRecebido>,
}

impl NotaRecebida {
    /// Whether this document is for this tax id.
    #[must_use]
    pub fn destinada_a(&self, documento: &str) -> bool {
        self.destinatario_documento.as_deref() == Some(documento)
    }

    /// Whether the protocol is missing, which means nothing proves the SEFAZ authorized it.
    #[must_use]
    pub fn falta_protocolo(&self) -> bool {
        self.protocolo.is_none()
    }
}

fn invalido(motivo: impl Into<String>) -> NfeError {
    NfeError::Xml(motivo.into())
}

fn exigir(element: &Element, tag: &str, grupo: &str) -> Result<String, NfeError> {
    element.child_text(tag).ok_or_else(|| invalido(format!("falta {tag} em {grupo}")))
}

fn decimal(element: &Element, tag: &str, grupo: &str) -> Result<Decimal, NfeError> {
    exigir(element, tag, grupo)?.parse().map_err(|_| invalido(format!("{tag} em {grupo} não é um número")))
}

fn decimal_opcional(element: &Element, tag: &str, grupo: &str) -> Result<Decimal, NfeError> {
    match element.child_text(tag) {
        Some(texto) => texto.parse().map_err(|_| invalido(format!("{tag} em {grupo} não é um número"))),
        None => Ok(Decimal::ZERO),
    }
}

fn gtin(element: &Element, tag: &str) -> Option<String> {
    element.child_text(tag).filter(|valor| !valor.is_empty() && valor != "SEM GTIN")
}

fn ler_item(det: &Element, posicao: usize) -> Result<ItemRecebido, NfeError> {
    let grupo = format!("det[{}]", posicao + 1);
    let prod = det.child("prod").ok_or_else(|| invalido(format!("falta prod em {grupo}")))?;
    let imposto = det.child("imposto");
    let valor_em = |tag: &str| -> Result<Decimal, NfeError> {
        match imposto.and_then(|i| i.find_text(tag)) {
            Some(texto) => texto.parse().map_err(|_| invalido(format!("{tag} em {grupo} não é um número"))),
            None => Ok(Decimal::ZERO),
        }
    };
    let numero = det
        .attr("nItem")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| invalido(format!("nItem inválido em {grupo}")))?;
    Ok(ItemRecebido {
        numero,
        codigo: exigir(prod, "cProd", &grupo)?,
        gtin: gtin(prod, "cEAN"),
        descricao: exigir(prod, "xProd", &grupo)?,
        ncm: exigir(prod, "NCM", &grupo)?,
        cfop: exigir(prod, "CFOP", &grupo)?,
        unidade: exigir(prod, "uCom", &grupo)?,
        quantidade: decimal(prod, "qCom", &grupo)?,
        valor_unitario: decimal(prod, "vUnCom", &grupo)?,
        valor_total: decimal(prod, "vProd", &grupo)?,
        frete: decimal_opcional(prod, "vFrete", &grupo)?,
        seguro: decimal_opcional(prod, "vSeg", &grupo)?,
        desconto: decimal_opcional(prod, "vDesc", &grupo)?,
        outros: decimal_opcional(prod, "vOutro", &grupo)?,
        valor_icms_st: valor_em("vICMSST")?,
        valor_ipi: valor_em("vIPI")?,
        unidade_tributavel: exigir(prod, "uTrib", &grupo)?,
        quantidade_tributavel: decimal(prod, "qTrib", &grupo)?,
        gtin_tributavel: gtin(prod, "cEANTrib"),
    })
}

/// Reads an `nfeProc` or an `NFe`, on its own or inside other XML.
pub fn ler_nota_recebida(xml_texto: &str) -> Result<NotaRecebida, NfeError> {
    if xml_texto.len() > LIMITE_XML {
        return Err(invalido("XML maior que o limite aceito"));
    }
    let raiz = xml::parse(xml_texto)?;
    let nfe = raiz.find("NFe").ok_or_else(|| invalido("o XML não tem NFe"))?;
    let inf = nfe.child("infNFe").ok_or_else(|| invalido("o XML não tem infNFe"))?;
    let chave = inf
        .attr("Id")
        .and_then(|id| id.strip_prefix("NFe"))
        .filter(|chave| forma_valida(chave))
        .ok_or_else(|| invalido("o Id de infNFe não traz uma chave de acesso"))?
        .to_string();
    let ide = inf.child("ide").ok_or_else(|| invalido("o XML não tem ide"))?;
    let emit = inf.child("emit").ok_or_else(|| invalido("o XML não tem emit"))?;
    let modelo = match exigir(ide, "mod", "ide")?.as_str() {
        "55" => Modelo::Nfe55,
        "65" => Modelo::Nfce65,
        outro => return Err(invalido(format!("modelo {outro} não é de NF-e nem de NFC-e"))),
    };
    let total =
        inf.child("total").and_then(|t| t.child("ICMSTot")).ok_or_else(|| invalido("o XML não tem total"))?;
    let protocolo = raiz
        .find("protNFe")
        .and_then(|prot| prot.child("infProt"))
        .and_then(|inf_prot| inf_prot.child_text("nProt"));
    let dest = inf.child("dest");
    let destinatario_documento =
        dest.and_then(|d| d.child_text("CNPJ")).or_else(|| dest.and_then(|d| d.child_text("CPF")));
    let mut itens = Vec::new();
    for (posicao, det) in inf.elements().filter(|e| e.local_name() == "det").enumerate() {
        itens.push(ler_item(det, posicao)?);
    }
    if itens.is_empty() {
        return Err(invalido("o XML não tem itens"));
    }
    Ok(NotaRecebida {
        chave,
        modelo,
        serie: exigir(ide, "serie", "ide")?.parse().map_err(|_| invalido("serie inválida"))?,
        numero: exigir(ide, "nNF", "ide")?.parse().map_err(|_| invalido("nNF inválido"))?,
        emitida_em: exigir(ide, "dhEmi", "ide")?,
        emitente_cnpj: emit
            .child_text("CNPJ")
            .or_else(|| emit.child_text("CPF"))
            .ok_or_else(|| invalido("o emitente não tem CNPJ nem CPF"))?,
        emitente_nome: exigir(emit, "xNome", "emit")?,
        destinatario_documento,
        protocolo,
        total: decimal(total, "vNF", "ICMSTot")?,
        itens,
    })
}

/// Checks the signature of a received document against the certificate inside it.
///
/// Says nothing about the trust chain, which needs the list of the SEFAZ.
pub fn conferir_assinatura(xml_texto: &str) -> Result<AutopsiaAssinatura, NfeError> {
    if xml_texto.len() > LIMITE_XML {
        return Err(invalido("XML maior que o limite aceito"));
    }
    autopsia_assinatura(xml_texto)
}
