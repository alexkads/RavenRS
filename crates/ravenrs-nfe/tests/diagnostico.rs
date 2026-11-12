//! The sandbox as a diagnosis tool: findings, autopsy, incidents, recording, faults and the story.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod comum;

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use comum::*;
use ravenrs_nfe::assinatura::sign_enveloped;
use ravenrs_nfe::documento::lote;
use ravenrs_nfe::sandbox::autopsia::autopsia_contra_original;
use ravenrs_nfe::sandbox::{
    Causa, Confianca, Efeito, EstadoChave, Evidencia, Falha, Fonte, Gravador, Incidente, Instante, Opcoes,
    RegraDeFalha, RelogioControlado, Reprodutor, SandboxSigner, SefazSandbox, Veredito, analisar,
    autopsia_assinatura, autorizacao, catalogo, cenarios, diagnosticar,
};
use ravenrs_nfe::soap::{envelope, request};
use ravenrs_nfe::{
    Cancelamento, Documento, Modelo, NfeError, Rascunho, Servico, Situacao, Transport, TransportError,
};
use sha1::{Digest, Sha1};

fn documento() -> (SefazSandbox, Documento) {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfe55(proximo_numero())).expect("document");
    (server, documento)
}

fn cancelamento(chave: &ravenrs_nfe::ChaveAcesso, protocolo: &str, quando: &str) -> Cancelamento {
    Cancelamento {
        chave: chave.clone(),
        protocolo: protocolo.to_string(),
        justificativa: "Venda desfeita a pedido do cliente".to_string(),
        quando: quando.to_string(),
        sequencia: 1,
    }
}

// ---------------------------------------------------------------- findings

#[test]
fn clean_document_has_no_refusal() {
    let (_server, documento) = documento();
    let achados = diagnosticar(&documento.xml);
    assert!(achados.iter().all(|a| a.efeito != Efeito::Rejeicao), "{achados:#?}");
}

#[test]
fn diagnosis_lists_every_problem_and_the_sandbox_answers_the_first() {
    let (server, documento) = documento();
    // Three independent problems: the first item's value, the total, and the payment.
    let ruim = alterar_e_reassinar(&documento, |xml| {
        xml.replacen("<vProd>100.00</vProd>", "<vProd>200.00</vProd>", 1)
            .replacen("<vNF>100.00</vNF>", "<vNF>500.00</vNF>", 1)
            .replacen("<tpImp>1</tpImp>", "<tpImp>4</tpImp>", 1)
    });
    let achados = diagnosticar(&ruim);
    let codigos: Vec<u32> = achados.iter().map(|a| a.c_stat).collect();
    for esperado in [710, 629, 610] {
        assert!(codigos.contains(&esperado), "{esperado} em {codigos:?}");
    }
    let resposta = enviar_bruto(&server, Modelo::Nfe55, &ruim);
    let primeira = achados.iter().find(|a| a.efeito == Efeito::Rejeicao).expect("refusal");
    assert_eq!(c_stat(&resposta), primeira.c_stat);
}

#[test]
fn a_finding_says_where_what_how_and_from_which_source() {
    let (_server, documento) = documento();
    let ruim = alterar_e_reassinar(&documento, |xml| xml.replacen("<tpImp>1</tpImp>", "<tpImp>4</tpImp>", 1));
    let achado = diagnosticar(&ruim).into_iter().find(|a| a.c_stat == 710).expect("710");
    assert_eq!(achado.caminho, "/NFe/infNFe/ide/tpImp");
    assert_eq!(achado.encontrado.as_deref(), Some("4"));
    assert_eq!(achado.confianca, Confianca::LidaNaFonte);
    assert!(matches!(achado.fonte, Fonte::Moc(_)));
    assert!(!achado.como_corrigir.is_empty() && !achado.explicacao.is_empty());
}

#[test]
fn findings_mask_personal_values() {
    let (_server, documento) = documento();
    let ruim = alterar_e_reassinar(&documento, |xml| {
        xml.replace("NF-E EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL", "MARIA DA SILVA SAURO")
    });
    let achado = diagnosticar(&ruim).into_iter().find(|a| a.c_stat == 598).expect("598");
    let encontrado = achado.encontrado.expect("found");
    assert!(!encontrado.contains("MARIA"), "{encontrado}");
}

#[test]
fn unreadable_text_is_a_finding_and_not_a_panic() {
    let achados = diagnosticar("<NFe><infNFe");
    assert_eq!(achados[0].c_stat, 243);
    assert_eq!(diagnosticar("<nada/>")[0].c_stat, 516);
}

#[test]
fn analysis_says_what_it_could_not_judge() {
    let (_server, documento) = documento();
    let analise = analisar(&documento.xml, &Opcoes::default());
    let texto = analise.nao_avaliado.join(" ");
    assert!(texto.contains("204"));
    assert!(texto.contains("cadeia"));
    assert!(texto.contains("703"));
}

#[test]
fn tolerances_of_the_moc_are_applied() {
    let (_server, documento) = documento();
    let centavo = alterar_e_reassinar(&documento, |xml| {
        xml.replacen("<vProd>100.00</vProd>", "<vProd>100.01</vProd>", 1)
    });
    assert!(diagnosticar(&centavo).iter().all(|a| a.c_stat != 629));
    let meio =
        alterar_e_reassinar(&documento, |xml| xml.replacen("<vNF>100.00</vNF>", "<vNF>100.50</vNF>", 1));
    assert!(diagnosticar(&meio).iter().all(|a| a.c_stat != 610));
    let acima =
        alterar_e_reassinar(&documento, |xml| xml.replacen("<vNF>100.00</vNF>", "<vNF>100.51</vNF>", 1));
    assert!(diagnosticar(&acima).iter().any(|a| a.c_stat == 610));
}

#[test]
fn forbidden_cnf_gives_897_and_generated_ones_avoid_the_list() {
    let (_server, documento) = documento();
    let c_nf = documento.chave.as_str()[35..43].to_string();
    let ruim = alterar_e_reassinar(&documento, |xml| {
        xml.replace(&format!("<cNF>{c_nf}</cNF>"), "<cNF>12345678</cNF>")
    });
    assert!(diagnosticar(&ruim).iter().any(|a| a.c_stat == 897));
    let rascunho = Rascunho { codigo_numerico: Some("12345678".into()), ..nfce(77) };
    assert!(matches!(emissor(&SefazSandbox::new()).preparar(&rascunho), Err(NfeError::Validacao { .. })));
}

#[test]
fn every_diagnosis_id_is_in_the_catalog() {
    for id in ["F02-DIGEST", "SIG-SHA256", "YA04-20", "GAP03a-6", "UB12-10", "DIST-656-ESPERA", "MANIF-595"] {
        assert!(catalogo::regra(id).is_some(), "{id}");
    }
    let contagem = catalogo::cobertura();
    assert_eq!(contagem.total, contagem.lidas + contagem.inferidas + contagem.de_memoria);
}

// ---------------------------------------------------------------- autopsy

fn causa(autopsia: &ravenrs_nfe::sandbox::AutopsiaAssinatura, alvo: Causa) -> Evidencia {
    autopsia
        .causas
        .iter()
        .find(|c| c.causa == alvo)
        .unwrap_or_else(|| panic!("{alvo:?} em {:#?}", autopsia.causas))
        .evidencia
}

#[test]
fn intact_signature_has_no_cause() {
    let (_server, documento) = documento();
    let autopsia = autopsia_assinatura(&documento.xml).expect("autopsy");
    assert!(autopsia.integra(), "{autopsia:#?}");
    assert_eq!(autopsia.digest_informado, autopsia.digest_calculado);
}

#[test]
fn signed_without_the_inherited_namespace() {
    let (_server, documento) = documento();
    let id = format!("NFe{}", documento.chave);
    let sem = sem_assinatura(&documento.xml).replacen(" xmlns=\"http://www.portalfiscal.inf.br/nfe\"", "", 1);
    let assinado = sign_enveloped(&sem, "NFe", &id, &SandboxSigner::new().expect("signer")).expect("signed");
    let recebido = assinado.replacen("<NFe>", "<NFe xmlns=\"http://www.portalfiscal.inf.br/nfe\">", 1);
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert!(!autopsia.digest_confere);
    assert_eq!(autopsia.causa_principal(), Some(Causa::XmlnsAusente));
    assert_eq!(causa(&autopsia, Causa::XmlnsAusente), Evidencia::Comprovada);
    let divergencia = autopsia.divergencia.expect("byte");
    assert!(divergencia.recebido.contains("xmlns") || divergencia.byte > 0);
}

#[test]
fn namespace_added_after_signing() {
    let (_server, documento) = documento();
    let recebido = documento.xml.replacen(
        "<NFe xmlns=",
        "<NFe xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=",
        1,
    );
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert_eq!(autopsia.causa_principal(), Some(Causa::XmlnsAMais));
    assert!(diagnosticar(&recebido).iter().any(|a| a.c_stat == 587));
}

#[test]
fn signed_inside_an_envelope_that_is_gone() {
    let (_server, documento) = documento();
    let id = format!("NFe{}", documento.chave);
    let soap = "xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\" ";
    let sem = sem_assinatura(&documento.xml).replacen("<NFe xmlns=", &format!("<NFe {soap}xmlns="), 1);
    let assinado = sign_enveloped(&sem, "NFe", &id, &SandboxSigner::new().expect("signer")).expect("signed");
    let recebido = assinado.replacen(soap, "", 1);
    assert_eq!(autopsia_assinatura(&recebido).expect("autopsy").causa_principal(), Some(Causa::XmlnsAMais));
}

#[test]
fn stray_spaces_and_line_breaks() {
    let (_server, documento) = documento();
    let entre_tags = documento.xml.replacen("<natOp>", " <natOp>", 1);
    let autopsia = autopsia_assinatura(&entre_tags).expect("autopsy");
    assert_eq!(autopsia.causa_principal(), Some(Causa::EspacosEQuebras));
    let no_valor = documento.xml.replacen("</natOp>", " \n</natOp>", 1);
    assert_eq!(
        autopsia_assinatura(&no_valor).expect("autopsy").causa_principal(),
        Some(Causa::EspacosEQuebras)
    );
}

#[test]
fn byte_order_mark() {
    let (server, documento) = documento();
    let recebido = format!("\u{feff}{}", documento.xml);
    assert_eq!(autopsia_assinatura(&recebido).expect("autopsy").causa_principal(), Some(Causa::Bom));
    assert_eq!(diagnosticar(&recebido)[0].c_stat, 588);
    let dentro = format!("\u{feff}{}", envelope(Servico::Autorizacao, &lote(1, &documento.xml)));
    let pedido = ravenrs_nfe::HttpRequest {
        metodo: Default::default(),
        url: "https://x/a".into(),
        headers: vec![("Content-Type".into(), Servico::Autorizacao.content_type())],
        body: dentro.into_bytes(),
    };
    assert_eq!(server.send(&pedido).expect("answer").status, 200);
}

#[test]
fn pretty_printing_after_signing_is_proved_and_pins_the_byte() {
    let (_server, documento) = documento();
    let bonito = documento
        .xml
        .replace("<ide>", "\n  <ide>")
        .replace("<emit>", "\n  <emit>")
        .replace("<dest>", "\n  <dest>")
        .replace("<total>", "\n  <total>");
    let autopsia = autopsia_assinatura(&bonito).expect("autopsy");
    assert_eq!(autopsia.causa_principal(), Some(Causa::Reformatado));
    assert_eq!(causa(&autopsia, Causa::Reformatado), Evidencia::Comprovada);
    let divergencia = autopsia.divergencia.expect("byte");
    let posicao = bonito.find('\n').expect("newline");
    assert!(divergencia.byte > 0 && divergencia.byte < posicao + 80, "{divergencia:?}");
    assert!(divergencia.recebido.contains('\n'));
}

#[test]
fn text_signed_without_canonicalizing_is_told_by_the_attribute_order() {
    let (_server, documento) = documento();
    let id = format!("NFe{}", documento.chave);
    let trocado = documento.xml.replacen(
        &format!("<infNFe Id=\"{id}\" versao=\"4.00\">"),
        &format!("<infNFe versao=\"4.00\" Id=\"{id}\">"),
        1,
    );
    let inicio = trocado.find("<infNFe ").expect("start");
    let fim = trocado.find("</infNFe>").expect("end") + "</infNFe>".len();
    let digest = STANDARD.encode(Sha1::digest(&trocado.as_bytes()[inicio..fim]));
    let de = trocado.find("<DigestValue>").expect("digest") + "<DigestValue>".len();
    let ate = trocado.find("</DigestValue>").expect("digest");
    let recebido = format!("{}{digest}{}", &trocado[..de], &trocado[ate..]);
    assert_eq!(
        autopsia_assinatura(&recebido).expect("autopsy").causa_principal(),
        Some(Causa::OrdemDeAtributos)
    );
    // Only reordering attributes changes nothing: C14N sorts them, so nothing breaks.
    assert!(autopsia_assinatura(&trocado).expect("autopsy").digest_confere);
}

#[test]
fn reference_uri_different_from_the_id() {
    let (_server, documento) = documento();
    let recebido = documento.xml.replacen(&format!("URI=\"#NFe{}\"", documento.chave), "URI=\"#outro\"", 1);
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert_eq!(causa(&autopsia, Causa::ReferenceUriDiferente), Evidencia::Comprovada);
    let achado = diagnosticar(&recebido);
    assert!(achado.iter().any(|a| a.c_stat == 298 && a.caminho.ends_with("/@URI")), "{achado:#?}");
}

#[test]
fn sha_256_where_sha_1_is_asked() {
    let (server, documento) = documento();
    let recebido = documento
        .xml
        .replace("http://www.w3.org/2000/09/xmldsig#sha1", "http://www.w3.org/2001/04/xmlenc#sha256")
        .replace(
            "http://www.w3.org/2000/09/xmldsig#rsa-sha1",
            "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256",
        );
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert_eq!(causa(&autopsia, Causa::Sha256NoLugarDeSha1), Evidencia::Comprovada);
    assert!(diagnosticar(&recebido).iter().any(|a| a.regra == "SIG-SHA256"));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfe55, &recebido)), 298);
}

#[test]
fn unknown_change_only_gets_a_sign_and_the_original_pins_the_byte() {
    let (_server, documento) = documento();
    let recebido = documento.xml.replacen("<natOp>VENDA", "<natOp>TROCA", 1);
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert_eq!(autopsia.causa_principal(), Some(Causa::ConteudoAlterado));
    assert_eq!(causa(&autopsia, Causa::ConteudoAlterado), Evidencia::Indicio);
    assert_eq!(autopsia.divergencia, None);
    let comparada = autopsia_contra_original(&recebido, &documento.xml).expect("autopsy");
    assert!(comparada.divergencia.expect("byte").recebido.contains("TROCA"));
}

#[test]
fn signature_of_another_key_keeps_the_digest_right() {
    let (_server, documento) = documento();
    let inicio = documento.xml.find("<SignatureValue>").expect("value") + "<SignatureValue>".len();
    let trocado = if &documento.xml[inicio..=inicio] == "A" { "B" } else { "A" };
    let recebido = format!("{}{}{}", &documento.xml[..inicio], trocado, &documento.xml[inicio + 1..]);
    let autopsia = autopsia_assinatura(&recebido).expect("autopsy");
    assert!(autopsia.digest_confere);
    assert_eq!(autopsia.assinatura_confere, Some(false));
    assert_eq!(autopsia.causa_principal(), Some(Causa::ChaveDiferenteDoCertificado));
}

// ---------------------------------------------------------------- incidents

fn retorno_com(c_stat: u32) -> String {
    format!(
        "<retEnviNFe><tpAmb>1</tpAmb><verAplic>SP_NFE_PL009</verAplic><cStat>104</cStat><xMotivo>Lote</xMotivo><protNFe versao=\"4.00\"><infProt><cStat>{c_stat}</cStat><xMotivo>Rejeicao do servidor</xMotivo></infProt></protNFe></retEnviNFe>"
    )
}

#[test]
fn incident_reproduces_a_real_rejection() {
    let (_server, documento) = documento();
    let adulterado = documento.xml.replacen("<natOp>VENDA", "<natOp>TROCA", 1);
    let incidente = Incidente::de_producao(&adulterado, Some(&retorno_com(297)));
    assert_eq!(incidente.veredito, Veredito::Reproduziu { c_stat: 297, regra: "F02-DIGEST" });
    assert!(incidente.conclusao().starts_with("Reproduziu"));
    assert_eq!(incidente.analise.chave.as_deref(), Some(documento.chave.as_str()));
}

#[test]
fn incident_does_not_invent_a_cause() {
    let (_server, documento) = documento();
    // Unknown to the catalog.
    let faltando = Incidente::de_producao(&documento.xml, Some(&retorno_com(7777)));
    assert_eq!(faltando.veredito, Veredito::RegraFaltando { c_stat: 7777, previsto: None });
    let texto = faltando.relatorio_markdown();
    assert!(texto.contains("não tem regra no catálogo"), "{texto}");
    assert!(texto.contains("Nada a sugerir sem uma causa"));

    // Has a rule, and the rule does not fire on this message.
    let nao = Incidente::de_producao(&documento.xml, Some(&retorno_com(610)));
    assert_eq!(nao.veredito, Veredito::NaoReproduziu { c_stat: 610, previsto: None });

    // Depends on what the server holds.
    let fora = Incidente::de_producao(&documento.xml, Some(&retorno_com(204)));
    assert!(matches!(fora.veredito, Veredito::ForaDoAlcance { c_stat: 204, regra: "2B08-20", .. }));
    assert!(fora.conclusao().contains("guarda"));

    // The server accepted what the catalog would refuse.
    let ruim = alterar_e_reassinar(&documento, |xml| xml.replacen("<tpImp>1</tpImp>", "<tpImp>4</tpImp>", 1));
    let aceita = Incidente::de_producao(&ruim, Some(&retorno_com(100)));
    assert_eq!(aceita.veredito, Veredito::AceitaMasOCatalogoRecusaria { c_stat: 100, previsto: 710 });
}

#[test]
fn incident_reads_soap_and_nfe_proc_and_works_with_no_answer() {
    let (_server, documento) = documento();
    let soap = envelope(Servico::Autorizacao, &lote(1, &documento.xml));
    let sem = Incidente::de_producao(&soap, None);
    assert_eq!(sem.veredito, Veredito::SemRetorno { previsto: None });
    assert_eq!(sem.analise.chave.as_deref(), Some(documento.chave.as_str()));
    let proc_nfe = format!(
        "<nfeProc versao=\"4.00\" xmlns=\"http://www.portalfiscal.inf.br/nfe\">{}<protNFe versao=\"4.00\"><infProt><nProt>135000000000001</nProt><cStat>100</cStat><xMotivo>ok</xMotivo></infProt></protNFe></nfeProc>",
        documento.xml
    );
    let proprio = Incidente::de_producao(&proc_nfe, None);
    assert_eq!(proprio.veredito, Veredito::AutorizadaSemAchados { c_stat: 100 });
    assert_eq!(proprio.retorno.expect("return").n_prot.as_deref(), Some("135000000000001"));
}

#[test]
fn incident_report_is_masked_and_has_the_five_parts() {
    let (_server, documento) = documento();
    let adulterado = documento.xml.replacen("<natOp>VENDA", "<natOp>TROCA", 1);
    let incidente = Incidente::de_producao(&adulterado, Some(&retorno_com(297)));
    let texto = incidente.relatorio_markdown();
    for parte in [
        "## O que aconteceu",
        "## Causa",
        "## Correção",
        "## Fonte",
        "## O que não foi possível concluir",
        "Autópsia",
    ] {
        assert!(texto.contains(parte), "{parte}\n{texto}");
    }
    assert!(!incidente.xml_mascarado.contains("11144477735"));
    assert!(!incidente.xml_mascarado.contains("MIIC"), "certificate left the report");
    assert!(!texto.contains("11144477735"));
    assert!(incidente.xml_mascarado.contains(documento.chave.as_str()));
}

// ---------------------------------------------------------------- recording

struct Compartilhado(std::sync::Arc<Gravador<SefazSandbox>>);

impl Transport for Compartilhado {
    fn send(&self, request: &ravenrs_nfe::HttpRequest) -> Result<ravenrs_nfe::HttpResponse, TransportError> {
        self.0.send(request)
    }

    fn intercepta(&self) -> bool {
        self.0.intercepta()
    }
}

fn emissor_em(transport: impl Transport + 'static) -> ravenrs_nfe::Emissor {
    ravenrs_nfe::Emissor::builder(ravenrs_nfe::BetaAck::i_understand_this_is_experimental())
        .emitente(emitente("11222333000181"))
        .signer(SandboxSigner::new().expect("signer"))
        .transport(transport)
        .build()
        .expect("emitter")
}

#[test]
fn recording_goes_there_and_back_with_no_network() {
    let server = SefazSandbox::new();
    server.use_once(ravenrs_nfe::sandbox::status(cenarios::paralisado_108));
    let relogio = RelogioControlado::em(Instante::utc(2026, 11, 5, 17, 30, 0));
    let gravador = std::sync::Arc::new(Gravador::novo(server.clone()).com_relogio(relogio));
    let emissor = emissor_em(Compartilhado(gravador.clone()));

    let numero = proximo_numero();
    let status = emissor.status(Modelo::Nfe55).expect("status");
    let nota = emissor.emitir(&nfe55(numero)).expect("emission");
    let consulta = emissor.consultar(&nota.documento.chave).expect("query");
    assert_eq!(status.retorno.c_stat, 108);
    assert_eq!(nota.autorizacao.c_stat(), 100);

    let gravacao = gravador.jsonl();
    assert_eq!(gravacao.lines().count(), 3);
    assert!(!gravacao.contains("11144477735"), "CPF na gravação");
    assert!(!gravacao.contains("X509Certificate>MII"), "certificado na gravação");
    assert!(gravacao.contains("[certificado removido]"));
    for linha in gravacao.lines() {
        let valor: serde_json::Value = serde_json::from_str(linha).expect("json");
        assert_eq!(valor["v"], 1);
        assert_eq!(valor["duracao_ms"], 0);
        assert!(valor["pedido"]["corpo"].is_string());
    }

    let reprodutor = std::sync::Arc::new(Reprodutor::de_jsonl(&gravacao).expect("recording"));
    assert_eq!(reprodutor.restantes(), 3);
    struct Repetir(std::sync::Arc<Reprodutor>);
    impl Transport for Repetir {
        fn send(
            &self,
            request: &ravenrs_nfe::HttpRequest,
        ) -> Result<ravenrs_nfe::HttpResponse, TransportError> {
            self.0.send(request)
        }
        fn intercepta(&self) -> bool {
            true
        }
    }
    let laboratorio = emissor_em(Repetir(reprodutor.clone()));
    assert_eq!(laboratorio.status(Modelo::Nfe55).expect("status").retorno.c_stat, 108);
    let repetida = laboratorio.emitir(&nfe55(numero)).expect("emission");
    assert_eq!(repetida.autorizacao.c_stat(), 100);
    assert_eq!(
        repetida.autorizacao.protocolo.as_ref().and_then(|p| p.n_prot.clone()),
        nota.autorizacao.protocolo.as_ref().and_then(|p| p.n_prot.clone())
    );
    assert_eq!(
        laboratorio.consultar(&nota.documento.chave).expect("query").retorno.c_stat,
        consulta.retorno.c_stat
    );
    assert_eq!(reprodutor.restantes(), 0);
    // Past the end of the tape it says so, and invents nothing.
    assert!(matches!(
        laboratorio.status(Modelo::Nfe55),
        Err(NfeError::Transporte(TransportError::NaoTratado { .. }))
    ));
}

#[test]
fn recording_keeps_transport_errors() {
    let server = SefazSandbox::new();
    server.injetar(RegraDeFalha::nova(Falha::Tls("handshake".to_string())));
    let gravador = Gravador::novo(server);
    let pedido = request(Servico::Status, "https://x/s", "<x/>");
    assert_eq!(gravador.send(&pedido), Err(TransportError::Tls("handshake".to_string())));
    let reprodutor = Reprodutor::de_jsonl(&gravador.jsonl()).expect("recording");
    assert_eq!(reprodutor.send(&pedido), Err(TransportError::Tls("handshake".to_string())));
    assert!(Reprodutor::de_jsonl("isto não é json").is_err());
}

// ---------------------------------------------------------------- faults

#[test]
fn fall_before_the_answer_leaves_a_note_authorized_without_a_return() {
    let server = SefazSandbox::new();
    server.injetar(RegraDeFalha::nova(Falha::QuedaAntesDaResposta).em(Servico::Autorizacao).vezes(1));
    let emissor = emissor(&server);
    let rascunho = nfce(proximo_numero());
    let documento = emissor.preparar(&rascunho).expect("document");
    assert!(matches!(emissor.autorizar(&documento), Err(NfeError::Transporte(TransportError::Conexao(_)))));
    let chave = documento.chave.as_str();
    assert!(matches!(server.estado_da_chave(chave), EstadoChave::AutorizadaSemRetorno { .. }));
    let passo = server.requests().pop().expect("step");
    assert!(passo.explicacao.expect("why").contains("Consulte a chave"));
    assert_eq!(passo.falha.as_deref(), Some("queda antes da resposta"));

    // The right reaction: asking shows it exists, and resending would only give 204.
    assert_eq!(emissor.consultar(&documento.chave).expect("query").retorno.c_stat, 100);
    assert!(matches!(server.estado_da_chave(chave), EstadoChave::Autorizada { .. }));
    assert_eq!(emissor.autorizar(&documento).expect("again").c_stat(), 204);
}

type Conferencia = fn(&Result<Situacao, NfeError>) -> bool;

#[test]
fn http_soap_fault_tls_and_broken_answers() {
    let casos: Vec<(Falha, Conferencia)> = vec![
        (Falha::Http(500), |r| matches!(r, Err(NfeError::RetornoInvalido { status: 500, .. }))),
        (Falha::Http(503), |r| matches!(r, Err(NfeError::RetornoInvalido { status: 503, .. }))),
        (
            Falha::SoapFault("caiu".into()),
            |r| matches!(r, Err(NfeError::RetornoInvalido { motivo, .. }) if motivo.contains("caiu")),
        ),
        (Falha::Tls("handshake".into()), |r| matches!(r, Err(NfeError::Transporte(TransportError::Tls(_))))),
        (Falha::XmlInvalido, |r| matches!(r, Err(NfeError::RetornoInvalido { status: 200, .. }))),
        (Falha::RespostaTruncada(50), |r| matches!(r, Err(NfeError::RetornoInvalido { .. }))),
        (Falha::Paralisado(109), |r| matches!(r, Ok(Situacao::Paralisado))),
    ];
    for (falha, confere) in casos {
        let server = SefazSandbox::new();
        server.injetar(RegraDeFalha::nova(falha.clone()).vezes(1));
        let emissor = emissor(&server);
        let resultado = emissor.emitir(&nfce(proximo_numero())).map(|e| e.autorizacao.situacao());
        assert!(confere(&resultado), "{falha:?}: {resultado:?}");
    }
}

#[test]
fn truncated_answer_was_processed_all_the_same() {
    let server = SefazSandbox::new();
    server.injetar(RegraDeFalha::nova(Falha::RespostaTruncada(40)).vezes(1));
    let emissor = emissor(&server);
    let documento = emissor.preparar(&nfce(proximo_numero())).expect("document");
    assert!(emissor.autorizar(&documento).is_err());
    assert_eq!(emissor.consultar(&documento.chave).expect("query").retorno.c_stat, 100);
}

#[test]
fn latency_moves_the_clock_and_past_the_limit_is_a_processed_timeout() {
    let relogio = RelogioControlado::em(Instante::utc(2026, 11, 5, 17, 30, 0));
    let server = SefazSandbox::new();
    server.usar_relogio(&relogio);
    server.limite_de_latencia(Duration::from_secs(30));
    server.injetar(RegraDeFalha::nova(Falha::Latencia(Duration::from_secs(5))).em(Servico::Status).vezes(1));
    let emissor = emissor(&server);
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 107);
    assert_eq!(relogio.agora(), Instante::utc(2026, 11, 5, 17, 30, 5));

    server.injetar(
        RegraDeFalha::nova(Falha::Latencia(Duration::from_secs(60))).em(Servico::Autorizacao).vezes(1),
    );
    let rascunho = Rascunho::novo(Modelo::Nfe55, 1, proximo_numero(), "VENDA", relogio.agora().para_xml());
    let rascunho = Rascunho {
        destinatario: nfe55(1).destinatario,
        itens: nfe55(1).itens,
        pagamentos: nfe55(1).pagamentos,
        ..rascunho
    };
    let documento = emissor.preparar(&rascunho).expect("document");
    assert_eq!(emissor.autorizar(&documento), Err(NfeError::Transporte(TransportError::Timeout)));
    assert!(matches!(
        server.estado_da_chave(documento.chave.as_str()),
        EstadoChave::AutorizadaSemRetorno { .. }
    ));
}

#[test]
fn stoppage_follows_a_time_window() {
    let relogio = RelogioControlado::em(Instante::utc(2026, 11, 5, 12, 0, 0));
    let server = SefazSandbox::new();
    server.usar_relogio(&relogio);
    server.injetar(
        RegraDeFalha::nova(Falha::Paralisado(108))
            .em(Servico::Status)
            .janela(Instante::utc(2026, 11, 5, 13, 0, 0), Instante::utc(2026, 11, 5, 14, 0, 0)),
    );
    let emissor = emissor(&server);
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 107);
    relogio.avancar(Duration::from_secs(3600));
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 108);
    relogio.avancar(Duration::from_secs(3600));
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 107);
}

#[test]
fn intermittent_failure_repeats_with_the_seed() {
    let rodar = |semente| {
        let server = SefazSandbox::new();
        server.injetar(RegraDeFalha::nova(Falha::Http(503)).em(Servico::Status).intermitente(semente, 400));
        let emissor = emissor(&server);
        (0..24).map(|_| emissor.status(Modelo::Nfe55).is_ok()).collect::<Vec<bool>>()
    };
    let primeira = rodar(42);
    assert_eq!(primeira, rodar(42));
    assert_ne!(primeira, rodar(43));
    assert!(primeira.contains(&true) && primeira.contains(&false));
}

#[test]
fn use_once_can_inject_a_failure_too() {
    let server = SefazSandbox::new();
    server.use_once(autorizacao(|_| ravenrs_nfe::sandbox::Resposta::Falha(Falha::QuedaAntesDaResposta)));
    let emissor = emissor(&server);
    let documento = emissor.preparar(&nfce(proximo_numero())).expect("document");
    assert!(emissor.autorizar(&documento).is_err());
    assert_eq!(emissor.autorizar(&documento).expect("again").c_stat(), 204);
}

#[test]
fn reset_takes_the_injected_failures_away() {
    let server = SefazSandbox::new();
    server.injetar(RegraDeFalha::nova(Falha::Http(500)));
    let emissor = emissor(&server);
    assert!(emissor.status(Modelo::Nfe55).is_err());
    server.reset_handlers();
    assert!(emissor.status(Modelo::Nfe55).is_ok());
}

// ---------------------------------------------------------------- dated rules

#[test]
fn asynchronous_changes_code_on_the_date_of_the_note() {
    let relogio = RelogioControlado::em(Instante::utc(2027, 1, 31, 12, 0, 0));
    let server = SefazSandbox::new();
    server.usar_relogio(&relogio);
    let documento = {
        let rascunho =
            Rascunho::novo(Modelo::Nfe55, 1, proximo_numero(), "VENDA", relogio.agora().para_xml());
        let rascunho = Rascunho {
            destinatario: nfe55(1).destinatario,
            itens: nfe55(1).itens,
            pagamentos: nfe55(1).pagamentos,
            ..rascunho
        };
        emissor(&server).preparar(&rascunho).expect("document")
    };
    let dados = lote(1, &documento.xml).replace("<indSinc>1</indSinc>", "<indSinc>0</indSinc>");
    let pedido = request(Servico::Autorizacao, "https://x/a", &dados);
    assert_eq!(c_stat(&server.send(&pedido).expect("answer")), 452);
    relogio.ir_para(Instante::utc(2027, 2, 1, 12, 0, 0));
    assert_eq!(c_stat(&server.send(&pedido).expect("answer")), 129);
}

#[test]
fn issue_date_is_checked_against_the_clock_when_there_is_one() {
    let relogio = RelogioControlado::em(Instante::de_xml(AGORA).expect("moment"));
    let server = SefazSandbox::new();
    server.usar_relogio(&relogio);
    let emissor = emissor(&server);
    let dentro = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    assert_eq!(dentro.autorizacao.c_stat(), 100);
    relogio.avancar(Duration::from_secs(301));
    assert_eq!(emissor.emitir(&nfce(proximo_numero())).expect("emission").autorizacao.c_stat(), 704);
    relogio.ir_para(Instante::utc(2026, 11, 5, 17, 0, 0));
    assert_eq!(emissor.emitir(&nfce(proximo_numero())).expect("emission").autorizacao.c_stat(), 703);
}

// ---------------------------------------------------------------- the story

#[test]
fn story_of_a_key_explains_each_refusal() {
    let relogio = RelogioControlado::em(Instante::de_xml(AGORA).expect("moment"));
    let server = SefazSandbox::new();
    server.usar_relogio(&relogio);
    let emissor = emissor(&server);

    // A rejected note cannot be cancelled: it never existed.
    let rejeitada = emissor.preparar(&nfe55(proximo_numero())).expect("document");
    let adulterado = rejeitada.xml.replacen("<natOp>VENDA", "<natOp>TROCA", 1);
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfe55, &adulterado)), 297);
    assert_eq!(server.estado_da_chave(rejeitada.chave.as_str()), EstadoChave::Rejeitada { c_stat: 297 });
    let negado = emissor.cancelar(&cancelamento(&rejeitada.chave, "135000000000000", AGORA)).expect("event");
    assert_eq!(negado.c_stat, 494);
    let passo = server.requests().pop().expect("step");
    assert!(passo.explicacao.expect("why").contains("rejeitada com 297"));

    // An authorized note, resent, then out of time.
    let nota = emissor.emitir(&nfe55(proximo_numero())).expect("emission");
    let chave = nota.documento.chave.clone();
    let protocolo = nota.autorizacao.protocolo.and_then(|p| p.n_prot).expect("protocol");
    assert_eq!(emissor.autorizar(&nota.documento).expect("again").c_stat(), 204);
    assert!(server.requests().pop().expect("step").explicacao.expect("why").contains("já existia"));
    relogio.avancar(Duration::from_secs(25 * 3600));
    let tarde = relogio.agora().para_xml();
    assert_eq!(emissor.cancelar(&cancelamento(&chave, &protocolo, &tarde)).expect("event").c_stat, 501);
    assert!(server.requests().pop().expect("step").explicacao.expect("why").contains("24 horas"));
    assert!(matches!(server.estado_da_chave(chave.as_str()), EstadoChave::Autorizada { .. }));

    // Inside the time, cancelled; once more is 580, by the order of the MOC.
    relogio.ir_para(Instante::de_xml(AGORA).expect("moment"));
    assert_eq!(emissor.cancelar(&cancelamento(&chave, &protocolo, AGORA)).expect("event").c_stat, 135);
    assert!(matches!(server.estado_da_chave(chave.as_str()), EstadoChave::Cancelada { .. }));
    assert_eq!(emissor.cancelar(&cancelamento(&chave, &protocolo, AGORA)).expect("event").c_stat, 580);

    let linha = server.linha_do_tempo();
    assert!(linha.iter().any(|p| p.diagnosticos.iter().any(|d| d.c_stat == 297)));
    let texto = server.relatorio_markdown(Some(chave.as_str()));
    for parte in [
        "## O que aconteceu",
        "## Causa",
        "## Correção",
        "## Fonte",
        "## O que não foi possível concluir",
        "cancelada",
        "24 horas",
    ] {
        assert!(texto.contains(parte), "{parte}\n{texto}");
    }
    assert!(!texto.contains("11144477735"));
}

#[test]
fn report_of_a_clean_day_says_nothing_was_wrong() {
    let server = SefazSandbox::new();
    emissor(&server).status(Modelo::Nfe55).expect("status");
    let texto = server.relatorio_markdown(None);
    assert!(texto.contains("nenhum pedido foi recusado"), "{texto}");
}
