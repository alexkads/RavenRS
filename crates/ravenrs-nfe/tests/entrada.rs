//! Entry of goods: reading received documents, the national distribution and the manifestation.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod comum;

use std::time::Duration;

use comum::*;
use ravenrs_nfe::documento::TipoManifestacao;
use ravenrs_nfe::entrada::{LIMITE_XML, conferir_assinatura, ler_nota_recebida};
use ravenrs_nfe::sandbox::{
    Fornecedor, Instante, RelogioControlado, Resposta, SandboxSigner, SefazSandbox, distribuicao,
    gerar_nota_de_compra,
};
use ravenrs_nfe::{BetaAck, ChaveAcesso, Emissor, Modelo, NfeError, Transport};

const COMPRADOR: &str = "11222333000181";
const QUANDO: &str = "2026-11-05T10:00:00-03:00";

fn compra() -> String {
    let fornecedor = Fornecedor::novo("DISTRIBUIDORA TESTE SA", "04252011000110", COMPRADOR);
    gerar_nota_de_compra(&fornecedor, vec![item("A1", "2", "10.50", "1.00"), item("A2", "1", "5", "0")])
        .expect("purchase")
}

fn cliente(server: &SefazSandbox, relogio: &RelogioControlado) -> Emissor {
    Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .emitente(emitente(COMPRADOR))
        .signer(SandboxSigner::para("MERCADO DE TESTE", COMPRADOR).expect("signer"))
        .transport(server.clone())
        .relogio(relogio.clone())
        .build()
        .expect("emitter")
}

fn relogio() -> RelogioControlado {
    RelogioControlado::em(Instante::de_xml(QUANDO).expect("moment"))
}

fn chave_de(xml: &str) -> ChaveAcesso {
    ChaveAcesso::parse(&ler_nota_recebida(xml).expect("note").chave).expect("key")
}

// ---------------------------------------------------------------- reading

#[test]
fn reads_what_the_engine_itself_issued() {
    let server = SefazSandbox::new();
    let nota = emissor(&server).emitir(&nfe55(proximo_numero())).expect("emission");
    let proc_nfe = nota.autorizacao.nfe_proc.expect("proc");
    let lida = ler_nota_recebida(&proc_nfe).expect("read");
    assert_eq!(lida.chave, nota.documento.chave.as_str());
    assert_eq!(lida.modelo, Modelo::Nfe55);
    assert_eq!(lida.numero, nota.documento.numero);
    assert_eq!(lida.emitente_cnpj, COMPRADOR);
    assert_eq!(lida.total, d("100.00"));
    assert!(lida.protocolo.is_some());
    assert!(!lida.falta_protocolo());
    assert!(lida.destinada_a("11144477735"));
    assert!(!lida.destinada_a("11222333000181"));

    let item = &lida.itens[0];
    assert_eq!(item.numero, 1);
    assert_eq!(item.codigo, "1");
    assert_eq!(item.gtin.as_deref(), Some("7891000315507"));
    assert_eq!(item.gtin_tributavel.as_deref(), Some("7891000315507"));
    assert_eq!(item.quantidade, d("1"));
    assert_eq!(item.valor_unitario, d("100"));
    assert_eq!(item.valor_total, d("100"));
    assert_eq!(item.desconto, d("0"));
    assert_eq!(item.unidade_tributavel, "UN");
    assert_eq!(item.quantidade_tributavel, d("1"));
    assert_eq!(item.valor_icms_st, d("0"));
    assert_eq!(item.valor_ipi, d("0"));
    assert!(conferir_assinatura(&proc_nfe).expect("signature").integra());
}

#[test]
fn nfe_with_no_protocol_says_so() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfe55(proximo_numero())).expect("document");
    let lida = ler_nota_recebida(&documento.xml).expect("read");
    assert_eq!(lida.protocolo, None);
    assert!(lida.falta_protocolo());
}

#[test]
fn no_gtin_is_read_as_none_and_optional_values_as_read() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfe55(proximo_numero())).expect("document");
    let xml = sem_assinatura(&documento.xml)
        .replace("<cEAN>7891000315507</cEAN>", "<cEAN>SEM GTIN</cEAN>")
        .replace("<cEANTrib>7891000315507</cEANTrib>", "<cEANTrib>SEM GTIN</cEANTrib>")
        .replace(
            "<indTot>1</indTot>",
            "<vFrete>1.50</vFrete><vSeg>2.50</vSeg><vOutro>3.50</vOutro><indTot>1</indTot>",
        )
        .replace("</ICMS>", "<vICMSST>7.50</vICMSST></ICMS><IPI><vIPI>8.50</vIPI></IPI>");
    let lida = ler_nota_recebida(&xml).expect("read");
    let item = &lida.itens[0];
    assert_eq!(item.gtin, None);
    assert_eq!(item.gtin_tributavel, None);
    assert_eq!((item.frete, item.seguro, item.outros), (d("1.50"), d("2.50"), d("3.50")));
    assert_eq!((item.valor_icms_st, item.valor_ipi), (d("7.50"), d("8.50")));
}

#[test]
fn tampering_shows_in_the_signature_check() {
    let xml = compra();
    assert!(conferir_assinatura(&xml).expect("signature").integra());
    let adulterado = xml.replacen("<vProd>", "<vProd>9", 1);
    assert!(!conferir_assinatura(&adulterado).expect("signature").integra());
}

#[test]
fn malformed_and_hostile_xml_are_errors() {
    let bomba = "<!DOCTYPE a [<!ENTITY b \"bbbb\"><!ENTITY c \"&b;&b;&b;\">]><NFe>&c;</NFe>";
    let externa = "<!DOCTYPE a [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><NFe>&x;</NFe>";
    let fundo = format!("{}{}", "<a>".repeat(5000), "</a>".repeat(5000));
    let grande = " ".repeat(LIMITE_XML + 1);
    let sem_itens = "<NFe><infNFe Id=\"NFe35261111222333000181650010000000011123456786\" versao=\"4.00\"><ide><mod>55</mod></ide><emit/><total><ICMSTot/></total></infNFe></NFe>";
    for ruim in ["", "lixo", "<NFe>", "<NFe></NFe>", "<a/>", bomba, externa, &fundo, &grande, sem_itens] {
        assert!(matches!(ler_nota_recebida(ruim), Err(NfeError::Xml(_))), "{}", &ruim[..ruim.len().min(40)]);
    }
    assert!(conferir_assinatura(&grande).is_err());
    assert!(conferir_assinatura(bomba).is_err());
}

#[test]
fn bad_number_is_an_error_and_not_a_panic() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfe55(proximo_numero())).expect("document");
    let xml = documento.xml.replacen("<vNF>100.00</vNF>", "<vNF>cem</vNF>", 1);
    assert!(ler_nota_recebida(&xml).is_err());
    let xml = documento.xml.replacen("<qCom>1.0000</qCom>", "<qCom>1e999999</qCom>", 1);
    assert!(ler_nota_recebida(&xml).is_err());
    let xml = documento.xml.replacen("nItem=\"1\"", "nItem=\"x\"", 1);
    assert!(ler_nota_recebida(&xml).is_err());
}

// ---------------------------------------------------------------- distribution

#[test]
fn summary_then_ciencia_then_the_full_xml() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    let compra_xml = compra();
    let chave = chave_de(&compra_xml);
    server.colocar_nota_recebida(&compra_xml).expect("placed");
    let emissor = cliente(&server, &horas);

    // First comes the summary.
    let lote = emissor.buscar_documentos(0).expect("query");
    assert_eq!(lote.c_stat, 138);
    assert_eq!(lote.documentos.len(), 1);
    let resumo = &lote.documentos[0];
    assert!(resumo.eh_resumo());
    assert_eq!(resumo.nsu, Some(1));
    assert!(resumo.xml.contains("<resNFe"));
    assert!(!resumo.xml.contains("<det "));

    // By key, it is still only the summary.
    let antes = emissor.baixar_por_chave(&chave).expect("by key");
    assert!(antes.documentos[0].eh_resumo());

    // Having fetched everything, the limiter holds the next query for an hour.
    assert_eq!(lote.ult_nsu, lote.max_nsu);
    assert!(emissor.proxima_consulta_em().is_some());
    assert!(matches!(emissor.buscar_documentos(lote.ult_nsu), Err(NfeError::ConsultaAdiantada { .. })));

    // The ciencia releases it.
    let ciencia = emissor.manifestar(&chave, TipoManifestacao::Ciencia, None, QUANDO).expect("event");
    assert_eq!(ciencia.c_stat, 135, "{}", ciencia.x_motivo);

    horas.avancar(Duration::from_secs(3601));
    assert_eq!(emissor.proxima_consulta_em(), None);
    let depois = emissor.buscar_documentos(lote.ult_nsu).expect("query");
    assert_eq!(depois.c_stat, 138);
    let completa = &depois.documentos[0];
    assert!(completa.eh_nota_completa());
    assert_eq!(completa.nsu, Some(2));
    let lida = ler_nota_recebida(&completa.xml).expect("read");
    assert!(lida.destinada_a(COMPRADOR));
    assert_eq!(lida.emitente_nome, "DISTRIBUIDORA TESTE SA");
    assert!(lida.protocolo.is_some());
    assert_eq!(lida.itens.len(), 2);
    assert!(conferir_assinatura(&completa.xml).expect("signature").integra());

    // By key, now the full document too.
    assert!(emissor.baixar_por_chave(&chave).expect("by key").documentos[0].eh_nota_completa());
}

#[test]
fn nsu_grows_and_nothing_new_gives_137_then_656() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    for _ in 0..3 {
        server.colocar_nota_recebida(&compra()).expect("placed");
    }
    let emissor = cliente(&server, &horas);
    let lote = emissor.buscar_documentos(0).expect("query");
    let nsus: Vec<Option<u64>> = lote.documentos.iter().map(|d| d.nsu).collect();
    assert_eq!(nsus, [Some(1), Some(2), Some(3)]);
    assert_eq!((lote.ult_nsu, lote.max_nsu), (3, 3));

    horas.avancar(Duration::from_secs(3601));
    let vazio = emissor.buscar_documentos(lote.ult_nsu).expect("query");
    assert_eq!(vazio.c_stat, 137);
    assert!(vazio.documentos.is_empty());
    assert!(vazio.sem_novidade());

    // The emitter stops a second try by itself; going around it, the server says 656.
    assert!(matches!(emissor.buscar_documentos(lote.ult_nsu), Err(NfeError::ConsultaAdiantada { .. })));
    let outro = cliente(&server, &horas);
    let indevido = outro.buscar_documentos(lote.ult_nsu).expect("query");
    assert_eq!(indevido.c_stat, 656);
    assert!(indevido.x_motivo.contains("1 hora"), "{}", indevido.x_motivo);
    assert!(outro.proxima_consulta_em().is_some());

    // After the hour it is free again.
    horas.avancar(Duration::from_secs(3601));
    assert_eq!(cliente(&server, &horas).buscar_documentos(lote.ult_nsu).expect("query").c_stat, 137);
}

#[test]
fn going_back_on_the_nsu_gives_656() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    server.colocar_nota_recebida(&compra()).expect("placed");
    server.colocar_nota_recebida(&compra()).expect("placed");
    let primeiro = cliente(&server, &horas);
    assert_eq!(primeiro.buscar_documentos(0).expect("query").ult_nsu, 2);
    let segundo = cliente(&server, &horas);
    let indevido = segundo.buscar_documentos(0).expect("query");
    assert_eq!(indevido.c_stat, 656);
    assert!(indevido.x_motivo.contains("ultNSU"));
    assert_eq!(indevido.ult_nsu, 2, "o 656 traz o último consultado");
}

#[test]
fn someone_else_cannot_download_or_manifest() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    let compra_xml = compra();
    let chave = chave_de(&compra_xml);
    server.colocar_nota_recebida(&compra_xml).expect("placed");
    let outro = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .emitente(emitente("04252011000110"))
        .signer(SandboxSigner::para("OUTRO", "04252011000110").expect("signer"))
        .transport(server.clone())
        .relogio(horas.clone())
        .build()
        .expect("emitter");
    assert_eq!(outro.baixar_por_chave(&chave).expect("by key").c_stat, 640);
    assert_eq!(outro.manifestar(&chave, TipoManifestacao::Ciencia, None, QUANDO).expect("event").c_stat, 575);
    assert_eq!(outro.buscar_documentos(0).expect("query").c_stat, 137);
}

#[test]
fn manifestations_and_their_rules() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    let compra_xml = compra();
    let chave = chave_de(&compra_xml);
    server.colocar_nota_recebida(&compra_xml).expect("placed");
    let emissor = cliente(&server, &horas);

    assert!(matches!(
        emissor.manifestar(&chave, TipoManifestacao::NaoRealizada, None, QUANDO),
        Err(NfeError::Validacao { .. })
    ));
    let desconhecida =
        emissor.manifestar(&chave, TipoManifestacao::Desconhecimento, None, QUANDO).expect("event");
    assert_eq!(desconhecida.c_stat, 135);
    // "Desconhecimento" does not release the download.
    assert!(emissor.baixar_por_chave(&chave).expect("by key").documentos[0].eh_resumo());
    assert_eq!(
        emissor.manifestar(&chave, TipoManifestacao::Desconhecimento, None, QUANDO).expect("event").c_stat,
        573
    );
    let nao = emissor
        .manifestar(&chave, TipoManifestacao::NaoRealizada, Some("Mercadoria nao foi entregue aqui"), QUANDO)
        .expect("event");
    assert_eq!(nao.c_stat, 135);
    assert!(emissor.baixar_por_chave(&chave).expect("by key").documentos[0].eh_nota_completa());
    assert_eq!(
        emissor.manifestar(&chave, TipoManifestacao::Confirmacao, None, QUANDO).expect("event").c_stat,
        135
    );

    let desconhecido = emissor.preparar(&nfe55(proximo_numero())).expect("other").chave;
    assert_eq!(
        emissor.manifestar(&desconhecido, TipoManifestacao::Ciencia, None, QUANDO).expect("event").c_stat,
        494
    );
}

#[test]
fn handlers_work_on_the_distribution_like_on_the_others() {
    let server = SefazSandbox::new();
    let horas = relogio();
    server.usar_relogio(&horas);
    server.use_once(distribuicao(|pedido| pedido.responder(108)));
    server.use_once(distribuicao(|_| Resposta::Timeout));
    let emissor = cliente(&server, &horas);
    assert!(matches!(emissor.buscar_documentos(0), Err(NfeError::Transporte(_))));
    assert_eq!(emissor.buscar_documentos(0).expect("query").c_stat, 108);
    assert_eq!(emissor.buscar_documentos(0).expect("query").c_stat, 137);
}

#[test]
fn compressed_bomb_and_garbage_in_doc_zip_are_errors() {
    use std::io::Write;

    use base64::Engine;

    let responder = |corpo: Vec<u8>| {
        let texto = base64::engine::general_purpose::STANDARD.encode(corpo);
        let body = format!(
            "<retDistDFeInt><cStat>138</cStat><xMotivo>x</xMotivo><ultNSU>1</ultNSU><maxNSU>1</maxNSU><loteDistDFeInt><docZip NSU=\"1\" schema=\"procNFe_v4.00.xsd\">{texto}</docZip></loteDistDFeInt></retDistDFeInt>"
        );
        ravenrs_nfe::HttpResponse { status: 200, headers: Vec::new(), body: body.into_bytes() }
    };
    assert!(ravenrs_nfe::retorno::ler_distribuicao(&responder(b"nao e gzip".to_vec())).is_err());

    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let bloco = vec![b'a'; 1024 * 1024];
    for _ in 0..21 {
        gz.write_all(&bloco).expect("write");
    }
    let bomba = gz.finish().expect("gzip");
    assert!(bomba.len() < 100_000);
    assert!(ravenrs_nfe::retorno::ler_distribuicao(&responder(bomba)).is_err());
}

#[test]
fn purchase_comes_from_another_company_and_passes_the_checks() {
    let xml = compra();
    let lida = ler_nota_recebida(&xml).expect("read");
    assert_ne!(lida.emitente_cnpj, COMPRADOR);
    assert!(lida.destinada_a(COMPRADOR));
    // It can even be authorized in another sandbox: the certificate is the supplier's own.
    let server = SefazSandbox::new();
    let inicio = xml.find("<NFe ").expect("start");
    let fim = xml.find("</NFe>").expect("end") + "</NFe>".len();
    let nfe = &xml[inicio..fim];
    let resposta = server
        .send(&ravenrs_nfe::soap::request(
            ravenrs_nfe::Servico::Autorizacao,
            "https://x/a",
            &ravenrs_nfe::documento::lote(1, nfe),
        ))
        .expect("answer");
    assert_eq!(c_stat(&resposta), 100);
}
