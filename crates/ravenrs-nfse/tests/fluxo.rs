//! The NFS-e from the DPS to the cancellation, against the sandbox of the Sefin.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_dfe::certificados::certificado_curinga;
use ravenrs_dfe::compactacao::gzip_base64;
use ravenrs_dfe::{Algoritmo, HttpRequest, HttpResponse, OnUnhandled, Transport, TransportError};
use ravenrs_nfse::sandbox::{Confianca, SefinSandbox, cenarios, regra};
use ravenrs_nfse::{
    Ambiente, BetaAck, ChaveNfse, Emissor, Inscricao, MotivoCancelamento, NfseError, dps, exemplo,
};

fn emissor(sandbox: &SefinSandbox) -> Emissor {
    Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .signer(certificado_curinga().expect("certificate"))
        .transport(sandbox.clone())
        .build()
        .expect("emitter")
}

fn autor() -> Inscricao {
    Inscricao::Cnpj(exemplo::CNPJ_PRESTADOR.to_owned())
}

#[test]
fn the_dps_becomes_an_nfse_with_the_dps_inside() {
    let sandbox = SefinSandbox::new();
    let emitida = emissor(&sandbox).emitir(&exemplo::dps(1)).expect("issued");
    assert!(!emitida.recuperada);
    assert_eq!(emitida.id_dps, dps::id_dps(&exemplo::dps(1)));
    assert_eq!(emitida.chave.municipio(), exemplo::MUNICIPIO);
    assert_eq!(emitida.chave.numero(), 1);
    assert!(emitida.xml.contains(&format!("Id=\"NFS{}\"", emitida.chave)));
    assert!(emitida.xml.contains("<Signature"), "the signed DPS goes inside");
    assert!(emitida.alertas.iter().any(|alerta| alerta.descricao.contains("sem validade jurídica")));
    let enviado = &sandbox.requests()[0];
    assert_eq!(enviado.http.metodo, ravenrs_dfe::Metodo::Post);
    assert!(enviado.http.caminho().ends_with("/nfse"));
    assert_eq!(enviado.status, Some(201));
}

#[test]
fn sha256_also_works_and_the_sandbox_can_refuse_it() {
    let sandbox = SefinSandbox::new();
    let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .signer(certificado_curinga().expect("certificate"))
        .transport(sandbox.clone())
        .algoritmo(Algoritmo::SHA256)
        .build()
        .expect("emitter");
    assert!(emissor.emitir(&exemplo::dps(1)).is_ok());
    sandbox.aceitar_hashes(&[ravenrs_dfe::HashAssinatura::Sha1]);
    assert_eq!(emissor.emitir(&exemplo::dps(2)).expect_err("refused").codigos(), ["E0714"]);
}

#[test]
fn a_lost_answer_is_found_by_the_id_of_the_dps() {
    let sandbox = SefinSandbox::new();
    sandbox.use_once(cenarios::resposta_perdida());
    let emitida = emissor(&sandbox).emitir(&exemplo::dps(7)).expect("recovered");
    assert!(emitida.recuperada);
    assert_eq!(sandbox.emitidas(), 1, "one NFS-e, not two");
    let caminhos: Vec<String> = sandbox
        .requests()
        .iter()
        .map(|registro| format!("{} {}", registro.http.metodo.nome(), registro.http.caminho()))
        .collect();
    let id = dps::id_dps(&exemplo::dps(7));
    assert_eq!(caminhos[1], format!("HEAD /SefinNacional/dps/{id}"));
    assert_eq!(caminhos[2], format!("GET /SefinNacional/dps/{id}"));
    assert!(caminhos[3].starts_with("GET /SefinNacional/nfse/"));
}

#[test]
fn nothing_processed_says_to_send_the_same_dps_again() {
    let sandbox = SefinSandbox::new();
    let emissor = emissor(&sandbox);
    sandbox.use_once(cenarios::timeout());
    let erro = emissor.emitir(&exemplo::dps(3)).expect_err("unconfirmed");
    let NfseError::NaoConfirmada { id_dps, .. } = &erro else { panic!("{erro:?}") };
    assert_eq!(*id_dps, dps::id_dps(&exemplo::dps(3)));
    assert!(erro.to_string().contains("sem trocar o número"));
    // Sending the same DPS again issues it once.
    assert!(!emissor.emitir(&exemplo::dps(3)).expect("issued").recuperada);
    assert_eq!(sandbox.emitidas(), 1);
}

#[test]
fn an_unavailable_sefin_is_not_a_refusal() {
    let sandbox = SefinSandbox::new();
    sandbox.use_once(cenarios::indisponivel());
    assert!(matches!(emissor(&sandbox).emitir(&exemplo::dps(1)), Err(NfseError::NaoConfirmada { .. })));
}

#[test]
fn sending_twice_returns_the_first_nfse() {
    let sandbox = SefinSandbox::new();
    let emissor = emissor(&sandbox);
    let primeira = emissor.emitir(&exemplo::dps(5)).expect("issued");
    let segunda = emissor.emitir(&exemplo::dps(5)).expect("the same one");
    assert!(segunda.recuperada);
    assert_eq!(primeira.chave, segunda.chave);
    assert_eq!(sandbox.requests()[1].codigos, ["E0014"]);
}

#[test]
fn a_city_without_agreement_is_refused_and_says_so_beforehand() {
    let sandbox = SefinSandbox::new();
    sandbox.sem_convenio(exemplo::MUNICIPIO);
    let emissor = emissor(&sandbox);
    let convenio = emissor.convenio(exemplo::MUNICIPIO).expect("answer");
    assert_eq!(convenio["aderente"], false);
    assert_eq!(emissor.emitir(&exemplo::dps(1)).expect_err("refused").codigos(), ["E0037"]);
}

#[test]
fn issued_in_the_future_is_refused() {
    let sandbox = SefinSandbox::new();
    sandbox.agora("2026-11-06T09:00:00-03:00");
    assert_eq!(emissor(&sandbox).emitir(&exemplo::dps(1)).expect_err("refused").codigos(), ["E0008"]);
}

fn enviar_dps(sandbox: &SefinSandbox, xml: &str) -> HttpResponse {
    let corpo = serde_json::json!({ "dpsXmlGZipB64": gzip_base64(xml).expect("zip") }).to_string();
    sandbox
        .send(&HttpRequest::post("https://x/SefinNacional/nfse", Vec::new(), corpo.into_bytes()))
        .expect("answer")
}

fn codigo(resposta: &HttpResponse) -> String {
    let corpo: serde_json::Value = serde_json::from_slice(&resposta.body).expect("json");
    corpo["erros"][0]["Codigo"].as_str().expect("code").to_owned()
}

#[test]
fn the_checks_catch_what_production_would() {
    let sandbox = SefinSandbox::new();
    let assinado = emissor(&sandbox).assinar(&exemplo::dps(9)).expect("signed");
    // Changed after signing.
    assert_eq!(codigo(&enviar_dps(&sandbox, &assinado.replace("150.00", "15.00"))), "E0714");
    // An Id that is not the one of its fields.
    let id = dps::id_dps(&exemplo::dps(9));
    let outro = format!("{}8", &id[..44]);
    assert_eq!(codigo(&enviar_dps(&sandbox, &assinado.replace(&id, &outro))), "E0004");
    // Production.
    assert_eq!(codigo(&enviar_dps(&sandbox, &assinado.replace("<tpAmb>2", "<tpAmb>1"))), "E0006");
    // Not a DPS.
    assert_eq!(codigo(&enviar_dps(&sandbox, "<NFe/>")), "SANDBOX-LEIAUTE");
    // Not GZip.
    let cru = sandbox
        .send(&HttpRequest::post(
            "https://x/SefinNacional/nfse",
            Vec::new(),
            br#"{"dpsXmlGZipB64":"xx"}"#.to_vec(),
        ))
        .expect("answer");
    assert_eq!((cru.status, codigo(&cru)), (400, "SANDBOX-CONTEUDO".to_owned()));
    assert_eq!(sandbox.emitidas(), 0);
}

#[test]
fn every_code_says_where_it_came_from() {
    for codigo in ["E0004", "E0006", "E0008", "E0014", "E0037", "E0714", "E0840"] {
        let regra = regra(codigo).expect(codigo);
        assert_eq!(regra.confianca, Confianca::Terceiros, "the official table was not read");
        assert!(!regra.como_corrigir.is_empty() && !regra.fonte.is_empty());
    }
}

#[test]
fn cancel_once_and_only_once() {
    let sandbox = SefinSandbox::new();
    let emissor = emissor(&sandbox);
    let emitida = emissor.emitir(&exemplo::dps(1)).expect("issued");
    let quando = "2026-11-06T10:00:00-03:00";
    let justificativa = "Serviço lançado em duplicidade no caixa";
    let evento = emissor
        .cancelar(&emitida.chave, &autor(), MotivoCancelamento::ErroNaEmissao, justificativa, quando)
        .expect("canceled");
    assert!(evento.contains("<tpEvento>101101</tpEvento>"));
    let de_novo = emissor
        .cancelar(&emitida.chave, &autor(), MotivoCancelamento::ErroNaEmissao, justificativa, quando)
        .expect_err("already");
    assert_eq!(de_novo.codigos(), ["E0840"]);
    let curta = emissor.cancelar(&emitida.chave, &autor(), MotivoCancelamento::Outros, "curta", quando);
    assert!(matches!(curta, Err(NfseError::Validacao { .. })));
    let inexistente = ChaveNfse::parse(&"1".repeat(50)).expect("key");
    assert!(emissor.consultar(&inexistente).is_err());
}

#[test]
fn unhandled_requests_follow_the_rule_and_handlers_reset() {
    let sandbox = SefinSandbox::new();
    let estranho = HttpRequest::get("https://x/SefinNacional/outra-coisa", Vec::new());
    assert!(matches!(sandbox.send(&estranho), Err(TransportError::NaoTratado { .. })));
    sandbox.on_unhandled(OnUnhandled::Warn);
    assert_eq!(sandbox.send(&estranho).expect("answer").status, 404);
    assert_eq!(sandbox.warnings().len(), 1);
    sandbox.use_handler(cenarios::recusar("E0037"));
    let emissor = emissor(&sandbox);
    assert_eq!(emissor.emitir(&exemplo::dps(1)).expect_err("refused").codigos(), ["E0037"]);
    sandbox.reset_handlers();
    assert!(emissor.emitir(&exemplo::dps(1)).is_ok());
}

#[test]
fn the_sandbox_only_takes_a_transport_that_stays_local_and_the_lab_stays_in_the_lab() {
    struct Rede;
    impl Transport for Rede {
        fn send(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
            Err(TransportError::Conexao("não deveria sair".to_owned()))
        }
    }
    let sandbox = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .signer(certificado_curinga().expect("certificate"))
        .transport(Rede)
        .build();
    assert!(matches!(sandbox, Err(NfseError::Configuracao(_))));
    let restrita = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::ProducaoRestrita)
        .signer(certificado_curinga().expect("certificate"))
        .transport(Rede)
        .build();
    assert!(matches!(restrita, Err(NfseError::Configuracao(motivo)) if motivo.contains("laboratório")));
}

#[test]
fn a_refused_mtls_is_a_403_with_no_body() {
    let sandbox = SefinSandbox::new();
    sandbox.use_once(cenarios::certificado_recusado());
    let erro = emissor(&sandbox).emitir(&exemplo::dps(1)).expect_err("refused");
    assert!(matches!(erro, NfseError::RetornoInvalido { status: 403, .. }), "{erro:?}");
}
