//! Helpers shared by the integration tests.
#![allow(dead_code, reason = "each test file uses a part of these")]
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use ravenrs_nfe::assinatura::sign_enveloped;
use ravenrs_nfe::documento::lote;
use ravenrs_nfe::sandbox::{SandboxSigner, SefazSandbox};
use ravenrs_nfe::soap::request;
use ravenrs_nfe::urls::url_servico;
use ravenrs_nfe::{
    Ambiente, BetaAck, Csosn, Decimal, Destinatario, Documento, DocumentoDestinatario, Emissor, Emitente,
    Endereco, FormaPagamento, Item, Modelo, Rascunho, Servico, Transport, Uf,
};

pub const AGORA: &str = "2026-11-05T14:30:00-03:00";

static PROXIMO_NUMERO: AtomicU32 = AtomicU32::new(1);

pub fn proximo_numero() -> u32 {
    PROXIMO_NUMERO.fetch_add(1, Ordering::Relaxed)
}

pub fn d(texto: &str) -> Decimal {
    Decimal::from_str(texto).expect("decimal")
}

pub fn endereco() -> Endereco {
    Endereco {
        logradouro: "RUA DAS FLORES".into(),
        numero: "100".into(),
        bairro: "CENTRO".into(),
        codigo_municipio: "3550308".into(),
        municipio: "SAO PAULO".into(),
        uf: Uf::SP,
        cep: "01001000".into(),
    }
}

pub fn emitente(cnpj: &str) -> Emitente {
    Emitente {
        cnpj: cnpj.into(),
        razao_social: "MERCADO DE TESTE & CIA LTDA".into(),
        nome_fantasia: Some("MERCADO".into()),
        inscricao_estadual: "123456789012".into(),
        endereco: endereco(),
    }
}

pub fn emissor_com(server: &SefazSandbox, cnpj: &str) -> Emissor {
    Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .emitente(emitente(cnpj))
        .signer(SandboxSigner::new().expect("signer"))
        .transport(server.clone())
        .build()
        .expect("emitter")
}

pub fn emissor(server: &SefazSandbox) -> Emissor {
    emissor_com(server, "11222333000181")
}

pub fn item(codigo: &str, quantidade: &str, valor: &str, desconto: &str) -> Item {
    Item {
        codigo: codigo.into(),
        gtin: Some("7891000315507".into()),
        descricao: "ARROZ TIPO 1 5KG".into(),
        ncm: "10063021".into(),
        cest: None,
        cfop: "5102".into(),
        unidade: "UN".into(),
        quantidade: d(quantidade),
        valor_unitario: d(valor),
        desconto: d(desconto),
        csosn: Csosn::S102,
        origem: 0,
    }
}

pub fn nfce(numero: u32) -> Rascunho {
    Rascunho::novo(Modelo::Nfce65, 1, numero, "VENDA AO CONSUMIDOR", AGORA)
        .com_item(item("1", "2", "10.50", "1.00"))
        .com_item(Item {
            csosn: Csosn::S500,
            cest: Some("1700100".into()),
            cfop: "5405".into(),
            ..item("2", "0.333", "9.99", "0")
        })
        .com_pagamento(FormaPagamento::Dinheiro, d("30"))
        .com_pagamento(FormaPagamento::Pix, d("1"))
}

pub fn nfe55(numero: u32) -> Rascunho {
    Rascunho::novo(Modelo::Nfe55, 1, numero, "VENDA DE MERCADORIA", AGORA)
        .com_destinatario(Destinatario {
            documento: DocumentoDestinatario::Cpf("11144477735".into()),
            nome: Some("CLIENTE REAL".into()),
            endereco: Some(endereco()),
        })
        .com_item(item("1", "1", "100", "0"))
        .com_pagamento(FormaPagamento::CartaoCredito, d("100"))
}

/// Removes `Signature` from a signed document.
pub fn sem_assinatura(xml: &str) -> String {
    let inicio = xml.find("<Signature").expect("signature start");
    let fim = xml.find("</Signature>").expect("signature end") + "</Signature>".len();
    format!("{}{}", &xml[..inicio], &xml[fim..])
}

/// Changes a signed document and signs it again, so only the change itself can be refused.
pub fn alterar_e_reassinar(documento: &Documento, alterar: impl FnOnce(String) -> String) -> String {
    let id = format!("NFe{}", documento.chave);
    let xml = alterar(sem_assinatura(&documento.xml));
    sign_enveloped(&xml, "NFe", &id, &SandboxSigner::new().expect("signer")).expect("signed")
}

/// Sends a document as it is through the transport, skipping the emitter.
pub fn enviar_bruto(server: &SefazSandbox, modelo: Modelo, nfe: &str) -> ravenrs_nfe::HttpResponse {
    let pedido =
        request(Servico::Autorizacao, url_servico(Uf::SP, modelo, Servico::Autorizacao), &lote(1, nfe));
    server.send(&pedido).expect("response")
}

/// The first `cStat` of the document inside a return, or that of the batch.
pub fn c_stat(resposta: &ravenrs_nfe::HttpResponse) -> u32 {
    let texto = std::str::from_utf8(&resposta.body).expect("utf-8");
    let root = ravenrs_nfe::xml::parse(texto).expect("xml");
    let inner = root
        .find("infProt")
        .or_else(|| root.find("infEvento"))
        .or_else(|| root.find("retEnviNFe"))
        .or_else(|| root.find("retEnvEvento"))
        .or_else(|| root.find("retConsSitNFe"))
        .or_else(|| root.find("retConsStatServ"))
        .expect("return");
    inner.child_text("cStat").expect("cStat").parse().expect("number")
}

/// A folder of its own under the system temp, removed when the test ends.
pub struct PastaTemporaria(std::path::PathBuf);

impl std::ops::Deref for PastaTemporaria {
    type Target = std::path::Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Creates a folder for the files the external tools read.
pub fn pasta_temporaria(nome: &str) -> PastaTemporaria {
    let pasta = std::env::temp_dir().join(format!("ravenrs-nfe-{}-{nome}", std::process::id()));
    std::fs::create_dir_all(&pasta).expect("temp");
    PastaTemporaria(pasta)
}
