//! Purchases for testing the entry of goods: real signed documents from other companies.

use std::sync::atomic::{AtomicU32, Ordering};

use super::SefazSandbox;
use super::certificados::GeradorCertificado;
use crate::ambiente::{Ambiente, BetaAck, Modelo};
use crate::emissor::Emissor;
use crate::error::NfeError;
use crate::modelo::{
    Destinatario, DocumentoDestinatario, Emitente, Endereco, FormaPagamento, Item, Rascunho,
};
use crate::soap::NS_NFE;
use crate::xml::{self, Element, escape_text};

static PROXIMO_NUMERO: AtomicU32 = AtomicU32::new(1);

/// The supplier of a test purchase, and who is buying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fornecedor {
    /// The supplier's legal name.
    pub razao_social: String,
    /// The supplier's CNPJ.
    pub cnpj: String,
    /// The supplier's state registration.
    pub inscricao_estadual: String,
    /// The supplier's address.
    pub endereco: Endereco,
    /// The CNPJ of the buyer.
    pub comprador_cnpj: String,
    /// When the purchase was issued.
    pub emitida_em: String,
}

impl Fornecedor {
    /// A supplier in the buyer's state, with test defaults for the rest.
    #[must_use]
    pub fn novo(
        razao_social: impl Into<String>,
        cnpj: impl Into<String>,
        comprador_cnpj: impl Into<String>,
    ) -> Self {
        Self {
            razao_social: razao_social.into(),
            cnpj: cnpj.into(),
            inscricao_estadual: "111222333444".to_string(),
            endereco: Endereco {
                logradouro: "RUA DO FORNECEDOR".to_string(),
                numero: "1".to_string(),
                bairro: "DISTRITO".to_string(),
                codigo_municipio: "3550308".to_string(),
                municipio: "SAO PAULO".to_string(),
                uf: crate::ambiente::Uf::SP,
                cep: "01001000".to_string(),
            },
            comprador_cnpj: comprador_cnpj.into(),
            emitida_em: "2026-11-05T08:00:00-03:00".to_string(),
        }
    }
}

/// Generates a purchase as `nfeProc`: model 55, signed by a certificate generated for the supplier.
///
/// The protocol is the sandbox's own, so this is a test document, with `tpAmb=2` and the
/// homologation texts. Numbers grow on each call.
pub fn gerar_nota_de_compra(fornecedor: &Fornecedor, itens: Vec<Item>) -> Result<String, NfeError> {
    let certificado =
        GeradorCertificado::novo(fornecedor.razao_social.clone(), fornecedor.cnpj.clone()).emitir()?;
    let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .emitente(Emitente {
            cnpj: fornecedor.cnpj.clone(),
            razao_social: fornecedor.razao_social.clone(),
            nome_fantasia: None,
            inscricao_estadual: fornecedor.inscricao_estadual.clone(),
            endereco: fornecedor.endereco.clone(),
        })
        .signer(certificado)
        .transport(SefazSandbox::new())
        .build()?;
    let numero = PROXIMO_NUMERO.fetch_add(1, Ordering::Relaxed);
    let mut rascunho =
        Rascunho::novo(Modelo::Nfe55, 1, numero, "VENDA DE MERCADORIA", fornecedor.emitida_em.clone())
            .com_destinatario(Destinatario {
                documento: DocumentoDestinatario::Cnpj(fornecedor.comprador_cnpj.clone()),
                nome: Some("COMPRADOR".to_string()),
                endereco: Some(fornecedor.endereco.clone()),
            });
    rascunho.itens = itens;
    // The payment of a sale between companies is outside what this test looks at.
    let rascunho = rascunho.com_pagamento(FormaPagamento::SemPagamento, rust_decimal::Decimal::ZERO);
    let documento = emissor.preparar(&rascunho)?;
    let dig_val = xml::parse(&documento.xml)?.find_text("DigestValue").unwrap_or_default();
    let protocolo = format!("2{numero:014}");
    let chave = documento.chave.as_str();
    Ok(format!(
        "<nfeProc xmlns=\"{NS_NFE}\" versao=\"4.00\">{}<protNFe versao=\"4.00\"><infProt><tpAmb>2</tpAmb><verAplic>RAVENRS-SANDBOX</verAplic><chNFe>{chave}</chNFe><dhRecbto>{}</dhRecbto><nProt>{protocolo}</nProt><digVal>{dig_val}</digVal><cStat>100</cStat><xMotivo>[SANDBOX] {}</xMotivo></infProt></protNFe></nfeProc>",
        documento.xml,
        escape_text(&fornecedor.emitida_em),
        escape_text("Autorizado o uso da NF-e"),
    ))
}

fn texto_ou_vazio(element: &Element, tag: &str) -> String {
    element.child_text(tag).unwrap_or_default()
}

/// The key, the recipient's CNPJ, the `nfeProc` and the summary of a document.
pub(super) fn separar(xml_texto: &str) -> Result<(String, String, String, String), NfeError> {
    let raiz = xml::parse(xml_texto)?;
    let invalido = |motivo: &str| NfeError::Xml(motivo.to_string());
    let nfe = raiz.find("NFe").ok_or_else(|| invalido("o XML não tem NFe"))?;
    let inf = nfe.child("infNFe").ok_or_else(|| invalido("o XML não tem infNFe"))?;
    let chave = inf
        .attr("Id")
        .and_then(|id| id.strip_prefix("NFe"))
        .ok_or_else(|| invalido("o XML não tem Id"))?
        .to_string();
    let dest = inf.child("dest").ok_or_else(|| invalido("o XML não tem destinatário"))?;
    let destinatario = dest.child_text("CNPJ").ok_or_else(|| invalido("o destinatário não tem CNPJ"))?;
    let ide = inf.child("ide").ok_or_else(|| invalido("o XML não tem ide"))?;
    let emit = inf.child("emit").ok_or_else(|| invalido("o XML não tem emit"))?;
    let total =
        inf.child("total").and_then(|t| t.child("ICMSTot")).ok_or_else(|| invalido("o XML não tem total"))?;
    let protocolo = raiz.find("infProt");
    let n_prot = protocolo.map_or_else(|| "000000000000000".to_string(), |p| texto_ou_vazio(p, "nProt"));
    let dh = protocolo.map_or_else(|| texto_ou_vazio(ide, "dhEmi"), |p| texto_ou_vazio(p, "dhRecbto"));
    let dig_val = nfe.find_text("DigestValue").unwrap_or_default();
    let resumo = format!(
        "<resNFe xmlns=\"{NS_NFE}\" versao=\"1.01\"><chNFe>{chave}</chNFe><CNPJ>{}</CNPJ><xNome>{}</xNome><IE>{}</IE><dhEmi>{}</dhEmi><tpNF>{}</tpNF><vNF>{}</vNF><digVal>{dig_val}</digVal><dhRecbto>{dh}</dhRecbto><nProt>{n_prot}</nProt><cSitNFe>1</cSitNFe></resNFe>",
        texto_ou_vazio(emit, "CNPJ"),
        escape_text(&texto_ou_vazio(emit, "xNome")),
        texto_ou_vazio(emit, "IE"),
        texto_ou_vazio(ide, "dhEmi"),
        texto_ou_vazio(ide, "tpNF"),
        texto_ou_vazio(total, "vNF"),
    );
    let proc_nfe = if raiz.local_name() == "nfeProc" {
        xml_texto.to_string()
    } else {
        format!("<nfeProc xmlns=\"{NS_NFE}\" versao=\"4.00\">{}</nfeProc>", xml::to_string(nfe))
    };
    Ok((chave, destinatario, proc_nfe, resumo))
}
