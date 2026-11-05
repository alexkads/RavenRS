//! Checks by tools that share no code with this crate: `xmllint` and `openssl`.
//!
//! A test whose tool is missing says so and passes; the XSD test is ignored unless asked for,
//! through `scripts/validar-xsd.sh`.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod comum;

use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use comum::*;
use ravenrs_nfe::c14n::canonicalize_document;
use ravenrs_nfe::sandbox::{CERTIFICADO_PEM, SandboxSigner, SefazSandbox, cenarios, status};
use ravenrs_nfe::xml::{parse, parse_document, to_string};
use ravenrs_nfe::{Cancelamento, Modelo, Signer, Transport};

fn ferramenta(nome: &str, ajuda: &str) -> Option<PathBuf> {
    let candidato = std::env::var_os(format!("{}_BIN", nome.to_uppercase()))
        .map(PathBuf::from)
        .unwrap_or_else(|| nome.into());
    Command::new(&candidato).arg(ajuda).output().ok().map(|_| candidato)
}

fn xmllint_c14n(xmllint: &Path, pasta: &Path, nome: &str, xml: &str) -> String {
    let arquivo = pasta.join(nome);
    std::fs::write(&arquivo, xml).expect("write");
    let saida = Command::new(xmllint).arg("--c14n").arg(&arquivo).output().expect("xmllint");
    assert!(saida.status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
    String::from_utf8(saida.stdout).expect("utf-8")
}

/// Cuts an element out as text.
fn recortar<'a>(xml: &'a str, abre: &str, fecha: &str) -> &'a str {
    let inicio = xml.find(abre).expect("start");
    let fim = xml.find(fecha).expect("end") + fecha.len();
    &xml[inicio..fim]
}

#[test]
fn c14n_matches_xmllint_on_tricky_documents() {
    let Some(xmllint) = ferramenta("xmllint", "--version") else {
        eprintln!("xmllint ausente: comparação de C14N não rodou");
        return;
    };
    let pasta = pasta_temporaria("c14n");
    // No comments here: `xmllint --c14n` keeps them, and the signature variant drops them.
    let documentos = [
        "<a z=\"1\" b=\"2\"/>",
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><a><b/>texto<c>x</c></a>",
        "<a b:x=\"1\" y=\"2\" a:x=\"3\" xmlns:b=\"urn:1\" xmlns:a=\"urn:2\" xmlns=\"urn:0\"/>",
        "<a xmlns=\"urn:u\"><b xmlns=\"urn:u\"/><c xmlns=\"\"><d/></c><e xmlns:p=\"urn:q\"><p:f p:g=\"1\"/></e></a>",
        "<a t=\"&quot;&#xA;&#x9;&lt;&amp;'&#xD;\">&lt;&gt;&amp;&#xD;\"'&#x41;&#233;</a>",
        "<?p  um dois?>\n<a><?q?><?r x?></a>\n<?s?>",
        "<a xml:lang=\"pt\" xml:space=\"preserve\">  <b>  </b>\n\t</a>",
        "<a><![CDATA[<x> & y]]></a>",
        "<a t=\"linha\nnova\ttab\"/>",
        "<açúcar preço=\"R$ 1,50\">ç</açúcar>",
        "<n1:a xmlns:n1=\"urn:1\" xmlns:n2=\"urn:2\" n2:b=\"1\" c=\"2\" n1:d=\"3\"><n2:e xmlns:n1=\"urn:1\"/></n1:a>",
        "<a xmlns:x=\"urn:x\"><b xmlns:x=\"urn:y\"><c xmlns:x=\"urn:x\"/></b></a>",
    ];
    for (posicao, xml) in documentos.iter().enumerate() {
        let esperado = xmllint_c14n(&xmllint, &pasta, &format!("doc{posicao}.xml"), xml);
        let nosso = canonicalize_document(&parse_document(xml).expect("xml")).expect("c14n");
        assert_eq!(nosso, esperado, "documento {posicao}: {xml}");
    }
}

#[test]
fn digest_and_signature_verify_with_xmllint_and_openssl() {
    let Some(xmllint) = ferramenta("xmllint", "--version") else {
        eprintln!("xmllint ausente: conferência independente não rodou");
        return;
    };
    let Some(openssl) = ferramenta("openssl", "version") else {
        eprintln!("openssl ausente: conferência independente não rodou");
        return;
    };
    let pasta = pasta_temporaria("assinatura");
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let xml = &documento.xml;

    // infNFe cut as text, with the namespace it inherits, canonicalized by xmllint alone.
    let inf_nfe = recortar(xml, "<infNFe ", "</infNFe>");
    let solto = inf_nfe.replacen("<infNFe ", "<infNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" ", 1);
    let canonico = xmllint_c14n(&xmllint, &pasta, "infnfe.xml", &solto);
    let arquivo = pasta.join("infnfe.c14n");
    std::fs::write(&arquivo, &canonico).expect("write");
    let saida =
        Command::new(&openssl).args(["dgst", "-sha1", "-binary"]).arg(&arquivo).output().expect("openssl");
    let digest_openssl = STANDARD.encode(&saida.stdout);
    let raiz = parse(xml).expect("xml");
    assert_eq!(raiz.find_text("DigestValue"), Some(digest_openssl));

    // SignedInfo the same way, verified with the public key taken from the certificate.
    let signed_info = recortar(xml, "<SignedInfo>", "</SignedInfo>");
    let solto =
        signed_info.replacen("<SignedInfo>", "<SignedInfo xmlns=\"http://www.w3.org/2000/09/xmldsig#\">", 1);
    let canonico = xmllint_c14n(&xmllint, &pasta, "signedinfo.xml", &solto);
    let info = pasta.join("signedinfo.c14n");
    std::fs::write(&info, &canonico).expect("write");
    let assinatura = pasta.join("assinatura.bin");
    let valor = raiz.find_text("SignatureValue").expect("value");
    std::fs::write(&assinatura, STANDARD.decode(valor).expect("base64")).expect("write");

    let certificado = pasta.join("cert.pem");
    std::fs::write(&certificado, CERTIFICADO_PEM).expect("write");
    let chave_publica = pasta.join("pub.pem");
    let saida = Command::new(&openssl)
        .args(["x509", "-pubkey", "-noout", "-in"])
        .arg(&certificado)
        .output()
        .expect("openssl");
    std::fs::write(&chave_publica, &saida.stdout).expect("write");
    let certificado_xml = raiz.find_text("X509Certificate").expect("certificate");
    let certificado_der = SandboxSigner::new().expect("signer");
    assert_eq!(STANDARD.decode(certificado_xml).expect("base64"), certificado_der.certificate_der());

    let verificar = |arquivo: &Path| {
        Command::new(&openssl)
            .args(["dgst", "-sha1", "-verify"])
            .arg(&chave_publica)
            .arg("-signature")
            .arg(&assinatura)
            .arg(arquivo)
            .output()
            .expect("openssl")
    };
    let saida = verificar(&info);
    assert!(saida.status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
    assert!(String::from_utf8_lossy(&saida.stdout).contains("Verified OK"));

    // One changed byte must fail in openssl too, so the check above means something.
    let alterado = pasta.join("alterado.c14n");
    std::fs::write(&alterado, canonico.replace("Reference", "Referencf")).expect("write");
    assert!(!verificar(&alterado).status.success());
}

fn validar(xmllint: &Path, schemas: &Path, esquema: &str, pasta: &Path, nome: &str, xml: &str) {
    let arquivo = pasta.join(nome);
    std::fs::write(&arquivo, xml).expect("write");
    let saida = Command::new(xmllint)
        .arg("--noout")
        .arg("--schema")
        .arg(schemas.join(esquema))
        .arg(&arquivo)
        .output()
        .expect("xmllint");
    assert!(saida.status.success(), "{nome} contra {esquema}:\n{}", String::from_utf8_lossy(&saida.stderr));
    eprintln!("{}", String::from_utf8_lossy(&saida.stderr).trim());
}

/// The message or return inside a SOAP text, standing alone.
fn interno(corpo: &str, nome: &str) -> String {
    to_string(parse(corpo).expect("xml").find(nome).expect("element"))
}

/// Validates everything this crate writes and everything the sandbox answers against PL_010.
#[test]
#[ignore = "precisa de NFE_XSD_DIR; rode scripts/validar-xsd.sh"]
fn everything_written_passes_the_official_schema() {
    let schemas = PathBuf::from(std::env::var("NFE_XSD_DIR").expect("NFE_XSD_DIR"));
    let xmllint = ferramenta("xmllint", "--version").expect("xmllint");
    let pasta = pasta_temporaria("xsd");
    let server = SefazSandbox::new();
    let emissor = emissor(&server);

    emissor.status(Modelo::Nfce65).expect("status");
    let nota_65 = emissor.emitir(&nfce(proximo_numero())).expect("nfce");
    let nota_55 = emissor.emitir(&nfe55(proximo_numero())).expect("nfe");
    let chave = nota_65.documento.chave.clone();
    let protocolo = nota_65.autorizacao.protocolo.clone().and_then(|p| p.n_prot).expect("protocol");
    emissor.consultar(&chave).expect("query");
    let cancelamento = Cancelamento {
        chave: chave.clone(),
        protocolo,
        justificativa: "Venda desfeita a pedido do cliente".to_string(),
        quando: AGORA.to_string(),
        sequencia: 1,
    };
    emissor.cancelar(&cancelamento).expect("cancel");
    assert_eq!(nota_65.autorizacao.c_stat(), 100);
    assert_eq!(nota_55.autorizacao.c_stat(), 100);

    validar(&xmllint, &schemas, "nfe_v4.00.xsd", &pasta, "nfce.xml", &nota_65.documento.xml);
    validar(&xmllint, &schemas, "nfe_v4.00.xsd", &pasta, "nfe.xml", &nota_55.documento.xml);

    // A tag out of place must fail, so the passes around it mean something.
    let invalido = pasta.join("invalido.xml");
    let serie = "<serie>1</serie>";
    let trocado =
        nota_65.documento.xml.replacen(serie, "", 1).replacen("<dhEmi>", &format!("{serie}<dhEmi>"), 1);
    std::fs::write(&invalido, trocado).expect("write");
    let saida = Command::new(&xmllint)
        .arg("--noout")
        .arg("--schema")
        .arg(schemas.join("nfe_v4.00.xsd"))
        .arg(&invalido)
        .output()
        .expect("xmllint");
    assert!(!saida.status.success(), "o esquema deveria recusar a tag fora de ordem");

    let alfanumerico = emissor_com(&SefazSandbox::new(), "12ABC34501DE35");
    let nota = alfanumerico.preparar(&nfce(proximo_numero())).expect("alphanumeric");
    validar(&xmllint, &schemas, "nfe_v4.00.xsd", &pasta, "alfa.xml", &nota.xml);

    let pedidos = server.requests();
    let pedido = |posicao: usize, nome: &str| interno(pedidos[posicao].body(), nome);
    validar(&xmllint, &schemas, "consStatServ_v4.00.xsd", &pasta, "status.xml", &pedido(0, "consStatServ"));
    validar(&xmllint, &schemas, "enviNFe_v4.00.xsd", &pasta, "lote.xml", &pedido(1, "enviNFe"));
    validar(&xmllint, &schemas, "consSitNFe_v4.00.xsd", &pasta, "consulta.xml", &pedido(3, "consSitNFe"));
    validar(&xmllint, &schemas, "envEventoCancNFe_v1.00.xsd", &pasta, "evento.xml", &pedido(4, "envEvento"));

    // Now the same messages through the transport again, to read the answers as text.
    let replay = SefazSandbox::new();
    let mut respostas = Vec::new();
    for registro in &pedidos {
        let resposta = replay.send(&registro.http).expect("answer");
        respostas.push(String::from_utf8(resposta.body).expect("utf-8"));
    }
    validar(
        &xmllint,
        &schemas,
        "retConsStatServ_v4.00.xsd",
        &pasta,
        "r-status.xml",
        &interno(&respostas[0], "retConsStatServ"),
    );
    validar(
        &xmllint,
        &schemas,
        "retEnviNFe_v4.00.xsd",
        &pasta,
        "r-lote.xml",
        &interno(&respostas[1], "retEnviNFe"),
    );
    validar(
        &xmllint,
        &schemas,
        "retConsSitNFe_v4.00.xsd",
        &pasta,
        "r-consulta.xml",
        &interno(&respostas[3], "retConsSitNFe"),
    );
    validar(
        &xmllint,
        &schemas,
        "retEnvEventoCancNFe_v1.00.xsd",
        &pasta,
        "r-evento.xml",
        &interno(&respostas[4], "retEnvEvento"),
    );

    // A rejection and a batch-level refusal have other shapes, so they are checked too.
    let rejeitada = enviar_bruto(&replay, Modelo::Nfce65, &nota_65.documento.xml.replace("<vNF>", "<vNF>9"));
    assert_eq!(c_stat(&rejeitada), 297);
    let texto = String::from_utf8(rejeitada.body).expect("utf-8");
    validar(&xmllint, &schemas, "retEnviNFe_v4.00.xsd", &pasta, "r-297.xml", &interno(&texto, "retEnviNFe"));

    replay.use_once(status(cenarios::paralisado_108));
    let parado = replay.send(&pedidos[0].http).expect("answer");
    let texto = String::from_utf8(parado.body).expect("utf-8");
    validar(
        &xmllint,
        &schemas,
        "retConsStatServ_v4.00.xsd",
        &pasta,
        "r-108.xml",
        &interno(&texto, "retConsStatServ"),
    );
}
