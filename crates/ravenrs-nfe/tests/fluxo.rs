//! The whole flow against `SefazSandbox`: what the emitter sends and what the checks answer.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod comum;

use std::sync::Arc;

use comum::*;
use ravenrs_nfe::documento::{QrCode, lote};
use ravenrs_nfe::modelo::{TEXTO_HOMOLOGACAO_DESTINATARIO, TEXTO_HOMOLOGACAO_ITEM};
use ravenrs_nfe::sandbox::{
    OnUnhandled, PREFIXO_MOTIVO, Resposta, SandboxSigner, SefazSandbox, VER_APLIC, autorizacao, cenarios,
    consulta, post, status,
};
use ravenrs_nfe::soap::request;
use ravenrs_nfe::urls::url_servico;
use ravenrs_nfe::{
    Ambiente, BetaAck, Cancelamento, ChaveAcesso, Emissor, HttpRequest, HttpResponse, Modelo, NfeError,
    Rascunho, Servico, Situacao, Transport, TransportError, Uf,
};

fn cancelamento(chave: &ChaveAcesso, protocolo: &str) -> Cancelamento {
    Cancelamento {
        chave: chave.clone(),
        protocolo: protocolo.to_string(),
        justificativa: "Venda desfeita a pedido do cliente".to_string(),
        quando: AGORA.to_string(),
        sequencia: 1,
    }
}

#[test]
fn nfce_is_authorized_and_says_it_came_from_the_sandbox() {
    let server = SefazSandbox::new();
    let emissao = emissor(&server).emitir(&nfce(proximo_numero())).expect("emission");
    let autorizacao = &emissao.autorizacao;
    assert_eq!(autorizacao.lote.c_stat, 104);
    assert_eq!(autorizacao.c_stat(), 100);
    assert_eq!(autorizacao.situacao(), Situacao::Autorizada);
    assert_eq!(autorizacao.lote.ver_aplic, VER_APLIC);
    assert!(autorizacao.lote.veio_do_sandbox());
    assert!(autorizacao.x_motivo().starts_with(PREFIXO_MOTIVO));
    let protocolo = autorizacao.protocolo.as_ref().expect("protocol");
    assert_eq!(protocolo.ch_nfe, emissao.documento.chave.as_str());
    assert_eq!(protocolo.n_prot.as_ref().map(String::len), Some(15));
    assert!(autorizacao.nfe_proc.as_ref().expect("proc").contains("</NFe><protNFe"));
}

#[test]
fn nfe55_is_authorized_with_the_recipient_text() {
    let server = SefazSandbox::new();
    let emissao = emissor(&server).emitir(&nfe55(proximo_numero())).expect("emission");
    assert_eq!(emissao.autorizacao.c_stat(), 100, "{}", emissao.autorizacao.x_motivo());
    assert!(emissao.documento.xml.contains(TEXTO_HOMOLOGACAO_DESTINATARIO));
    assert!(emissao.documento.qr_code.is_none());
}

#[test]
fn alphanumeric_cnpj_flows_through() {
    let server = SefazSandbox::new();
    let emissor = emissor_com(&server, "12ABC34501DE35");
    let emissao = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    assert_eq!(emissao.autorizacao.c_stat(), 100, "{}", emissao.autorizacao.x_motivo());
    assert_eq!(&emissao.documento.chave.as_str()[6..20], "12ABC34501DE35");
}

#[test]
fn xml_has_no_spaces_between_tags_and_keeps_the_layout_order() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let xml = &documento.xml;
    assert!(xml.starts_with("<NFe xmlns=\"http://www.portalfiscal.inf.br/nfe\"><infNFe Id=\"NFe"));
    assert!(!xml.contains("> <") && !xml.contains('\n'));
    let supl = xml.find("</infNFe><infNFeSupl><qrCode>").expect("supplement right after infNFe");
    let assinatura = xml.find("</infNFeSupl><Signature xmlns=").expect("signature after it");
    assert!(supl < assinatura);
    assert!(xml.ends_with("</Signature></NFe>"));
    assert!(xml.contains("<tpAmb>2</tpAmb>"));
    assert!(!xml.contains("<tpAmb>1</tpAmb>"));
    assert!(xml.contains("<infCpl>EMITIDO POR RAVENRS-NFE: PROVA DE CONCEITO BETA"));
    assert!(xml.contains("<verProc>RAVENRS-POC-BETA</verProc>"));
    assert!(xml.contains("MERCADO DE TESTE &amp; CIA LTDA"));
    // The first item carries the homologation text; the second keeps its own description.
    assert!(xml.contains(TEXTO_HOMOLOGACAO_ITEM));
    assert!(xml.contains("<xProd>ARROZ TIPO 1 5KG</xProd>"));
}

#[test]
fn same_draft_gives_the_same_bytes() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let rascunho = nfce(proximo_numero());
    assert_eq!(emissor.preparar(&rascunho).expect("a"), emissor.preparar(&rascunho).expect("b"));
}

#[test]
fn qr_code_of_nfce_turns_into_modules() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    assert!(documento.qr_code.as_deref().expect("qr").contains("|3|2"));
    assert!(documento.qr_code_modulos().expect("modules").is_ok());
}

#[test]
fn qr_code_v2_passes_with_a_csc() {
    let server = SefazSandbox::new();
    let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .emitente(emitente("11222333000181"))
        .signer(SandboxSigner::new().expect("signer"))
        .transport(server)
        .qr_code(QrCode::V2 { id_token: "1".into(), csc: "CSC-DE-TESTE".into() })
        .build()
        .expect("emitter");
    let emissao = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    assert_eq!(emissao.autorizacao.c_stat(), 100);
    assert!(emissao.documento.qr_code.expect("qr").contains("|2|2|1|"));
}

#[test]
fn real_tampering_gives_297() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let adulterado = documento.xml.replace("<vNF>", "<vNF>9");
    assert_ne!(adulterado, documento.xml);
    let resposta = enviar_bruto(&server, Modelo::Nfce65, &adulterado);
    assert_eq!(c_stat(&resposta), 297);
    assert!(std::str::from_utf8(&resposta.body).expect("utf-8").contains("[SANDBOX] "));
}

#[test]
fn tampered_signature_value_gives_297() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let inicio = documento.xml.find("<SignatureValue>").expect("value") + "<SignatureValue>".len();
    let trocado = if &documento.xml[inicio..=inicio] == "A" { "B" } else { "A" };
    let adulterado = format!("{}{}{}", &documento.xml[..inicio], trocado, &documento.xml[inicio + 1..]);
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &adulterado)), 297);
}

#[test]
fn missing_signature_gives_298() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &sem_assinatura(&documento.xml))), 298);
}

#[test]
fn resending_gives_204_and_the_protocol_of_the_first() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let rascunho = nfce(proximo_numero());
    let primeira = emissor.emitir(&rascunho).expect("first");
    assert_eq!(primeira.autorizacao.c_stat(), 100);
    let segunda = emissor.emitir(&rascunho).expect("second");
    assert_eq!(segunda.autorizacao.c_stat(), 204);
    assert_eq!(segunda.autorizacao.situacao(), Situacao::Duplicada);
    let protocolo = primeira.autorizacao.protocolo.and_then(|p| p.n_prot).expect("protocol");
    assert!(segunda.autorizacao.x_motivo().contains(&protocolo));
}

#[test]
fn same_number_with_another_key_gives_539() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let numero = proximo_numero();
    let primeira = Rascunho { codigo_numerico: Some("10000001".into()), ..nfce(numero) };
    let segunda = Rascunho { codigo_numerico: Some("10000002".into()), ..nfce(numero) };
    assert_eq!(emissor.emitir(&primeira).expect("first").autorizacao.c_stat(), 100);
    let retorno = emissor.emitir(&segunda).expect("second").autorizacao;
    assert_eq!(retorno.c_stat(), 539);
    assert_eq!(retorno.situacao(), Situacao::Duplicada);
}

#[test]
fn missing_homologation_text_gives_598_or_373() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let nfe = emissor.preparar(&nfe55(proximo_numero())).expect("nfe");
    let sem_texto =
        alterar_e_reassinar(&nfe, |xml| xml.replace(TEXTO_HOMOLOGACAO_DESTINATARIO, "CLIENTE REAL"));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfe55, &sem_texto)), 598);

    let nfce_doc = emissor.preparar(&nfce(proximo_numero())).expect("nfce");
    let sem_texto = alterar_e_reassinar(&nfce_doc, |xml| xml.replace(TEXTO_HOMOLOGACAO_ITEM, "ARROZ"));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &sem_texto)), 373);
}

#[test]
fn production_environment_is_refused_with_252() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let producao = alterar_e_reassinar(&documento, |xml| xml.replace("<tpAmb>2</tpAmb>", "<tpAmb>1</tpAmb>"));
    let resposta = enviar_bruto(&server, Modelo::Nfce65, &producao);
    assert_eq!(c_stat(&resposta), 252);
    assert!(std::str::from_utf8(&resposta.body).expect("utf-8").contains("só aceita tpAmb=2"));
}

#[test]
fn production_status_query_is_refused_too() {
    let server = SefazSandbox::new();
    let dados = "<consStatServ xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><tpAmb>1</tpAmb><cUF>35</cUF><xServ>STATUS</xServ></consStatServ>";
    let pedido = request(Servico::Status, url_servico(Uf::SP, Modelo::Nfe55, Servico::Status), dados);
    assert_eq!(c_stat(&server.send(&pedido).expect("response")), 252);
}

#[test]
fn tags_out_of_order_give_225() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let trocado = alterar_e_reassinar(&documento, |xml| {
        let serie = "<serie>1</serie>";
        xml.replacen(serie, "", 1).replacen("<dhEmi>", &format!("{serie}<dhEmi>"), 1)
    });
    // <serie> now comes after <nNF>.
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &trocado)), 225);
}

#[test]
fn unknown_tag_gives_225() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let com_tag =
        alterar_e_reassinar(&documento, |xml| xml.replacen("<natOp>", "<inventado>1</inventado><natOp>", 1));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &com_tag)), 225);
}

#[test]
fn spaces_between_tags_give_588() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let com_espaco = alterar_e_reassinar(&documento, |xml| xml.replacen("<natOp>", " <natOp>", 1));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &com_espaco)), 588);
}

#[test]
fn wrong_check_digit_gives_253_and_broken_concatenation_502() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let chave = documento.chave.as_str().to_string();

    let digito: u8 = documento.chave.dv().parse().expect("digit");
    let chave_errada = format!("{}{}", &chave[..43], (digito + 1) % 10);
    let id = format!("NFe{chave_errada}");
    let xml = sem_assinatura(&documento.xml)
        .replace(&chave, &chave_errada)
        .replace(&format!("<cDV>{digito}</cDV>"), &format!("<cDV>{}</cDV>", (digito + 1) % 10));
    let assinado =
        ravenrs_nfe::assinatura::sign_enveloped(&xml, "NFe", &id, &SandboxSigner::new().expect("signer"))
            .expect("signed");
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &assinado)), 253);

    let numero = alterar_e_reassinar(&documento, |xml| {
        let atual = format!("<nNF>{}</nNF>", documento.numero);
        xml.replacen(&atual, "<nNF>999999</nNF>", 1)
    });
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &numero)), 502);
}

#[test]
fn totals_are_recalculated() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    // 2 x 10.50 = 21.00, minus 1.00, plus 0.333 x 9.99 = 3.33: vProd 24.33, vNF 23.33.
    assert!(documento.xml.contains("<vNF>23.33</vNF>"));
    assert_eq!(documento.valor_total(), d("23.33"));

    let casos: [(&str, &str, u32); 4] = [
        ("<vNF>23.33</vNF>", "<vNF>24.33</vNF>", 610),
        ("<vProd>21.00</vProd>", "<vProd>22.00</vProd>", 629),
        ("<vProd>24.33</vProd>", "<vProd>25.33</vProd>", 564),
        ("</vUnTrib><vDesc>1.00</vDesc>", "</vUnTrib>", 537),
    ];
    for (de, para, esperado) in casos {
        let alterado = alterar_e_reassinar(&documento, |xml| xml.replacen(de, para, 1));
        assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &alterado)), esperado, "{de}");
    }
}

#[test]
fn payments_are_checked() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    assert!(documento.xml.contains("<vTroco>7.67</vTroco>"));
    let troco =
        alterar_e_reassinar(&documento, |xml| xml.replace("<vTroco>7.67</vTroco>", "<vTroco>1.00</vTroco>"));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &troco)), 869);
    let pouco = alterar_e_reassinar(&documento, |xml| {
        xml.replace("<vPag>30.00</vPag>", "<vPag>1.00</vPag>").replace("<vTroco>7.67</vTroco>", "")
    });
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &pouco)), 865);
}

#[test]
fn normal_regime_without_the_new_group_gives_1115() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let crt3 = alterar_e_reassinar(&documento, |xml| xml.replace("<CRT>1</CRT>", "<CRT>3</CRT>"));
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &crt3)), 1115);
}

#[test]
fn asynchronous_batch_gives_452() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let dados = lote(1, &documento.xml).replace("<indSinc>1</indSinc>", "<indSinc>0</indSinc>");
    let pedido = request(Servico::Autorizacao, "https://x/autorizacao", &dados);
    assert_eq!(c_stat(&server.send(&pedido).expect("response")), 452);
}

#[test]
fn nfce_without_supplement_gives_394() {
    let server = SefazSandbox::new();
    let documento = emissor(&server).preparar(&nfce(proximo_numero())).expect("document");
    let sem = alterar_e_reassinar(&documento, |xml| {
        let inicio = xml.find("<infNFeSupl>").expect("start");
        let fim = xml.find("</infNFeSupl>").expect("end") + "</infNFeSupl>".len();
        format!("{}{}", &xml[..inicio], &xml[fim..])
    });
    assert_eq!(c_stat(&enviar_bruto(&server, Modelo::Nfce65, &sem)), 394);
}

#[test]
fn use_once_answers_108_and_then_goes_away() {
    let server = SefazSandbox::new();
    server.use_once(autorizacao(cenarios::paralisado_108));
    let emissor = emissor(&server);
    let rascunho = nfce(proximo_numero());
    let primeira = emissor.emitir(&rascunho).expect("first").autorizacao;
    assert_eq!(primeira.c_stat(), 108);
    assert_eq!(primeira.situacao(), Situacao::Paralisado);
    assert!(primeira.protocolo.is_none());
    assert_eq!(emissor.emitir(&rascunho).expect("second").autorizacao.c_stat(), 100);
}

#[test]
fn status_is_107_and_can_be_overridden_then_reset() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let retorno = emissor.status(Modelo::Nfce65).expect("status");
    assert!(retorno.em_operacao());
    assert_eq!(retorno.retorno.ver_aplic, VER_APLIC);
    assert!(retorno.retorno.x_motivo.starts_with(PREFIXO_MOTIVO));

    server.use_handler(status(cenarios::paralisado_108));
    assert_eq!(emissor.status(Modelo::Nfce65).expect("status").retorno.c_stat, 108);
    assert_eq!(emissor.status(Modelo::Nfce65).expect("status").retorno.c_stat, 108);

    server.reset_handlers();
    assert_eq!(emissor.status(Modelo::Nfce65).expect("status").retorno.c_stat, 107);
}

#[test]
fn newest_handler_wins_and_next_falls_through() {
    let server = SefazSandbox::new();
    server.use_handler(status(cenarios::paralisado_108));
    server.use_handler(status(|_| Resposta::Proximo));
    let emissor = emissor(&server);
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 108);
    server.use_handler(status(|pedido| pedido.responder_com(109, "Parou de vez")));
    let retorno = emissor.status(Modelo::Nfe55).expect("status").retorno;
    assert_eq!(retorno.c_stat, 109);
    assert_eq!(retorno.x_motivo, "[SANDBOX] Parou de vez");
}

#[test]
fn initial_handlers_come_back_on_reset() {
    let server = SefazSandbox::com_handlers(vec![status(cenarios::paralisado_108)]);
    let emissor = emissor(&server);
    server.use_handler(status(cenarios::em_operacao_107));
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 107);
    server.reset_handlers();
    assert_eq!(emissor.status(Modelo::Nfe55).expect("status").retorno.c_stat, 108);
}

#[test]
fn post_handler_matches_by_url() {
    let server = SefazSandbox::new();
    server.use_handler(post("homologacao.nfce.fazenda.sp.gov.br", cenarios::paralisado_108));
    let emissor = emissor(&server);
    assert_eq!(emissor.status(Modelo::Nfce65).expect("nfce").retorno.c_stat, 108);
    assert_eq!(emissor.status(Modelo::Nfe55).expect("nfe").retorno.c_stat, 107);
}

#[test]
fn every_ready_scenario_answers_its_code_in_the_right_format() {
    type Cenario = fn(&ravenrs_nfe::sandbox::SandboxRequest) -> Resposta;
    let casos: [(Cenario, u32); 10] = [
        (cenarios::autorizado_100, 100),
        (cenarios::em_operacao_107, 107),
        (cenarios::paralisado_108, 108),
        (cenarios::duplicidade_204, 204),
        (cenarios::schema_225, 225),
        (cenarios::assinatura_297, 297),
        (cenarios::duplicidade_539, 539),
        (cenarios::homologacao_598, 598),
        (cenarios::ibscbs_1115, 1115),
        (cenarios::codigo_129, 129),
    ];
    for (cenario, codigo) in casos {
        let server = SefazSandbox::new();
        server.use_once(autorizacao(cenario));
        let retorno = emissor(&server).emitir(&nfce(proximo_numero())).expect("emission").autorizacao;
        assert_eq!(retorno.c_stat(), codigo);
        assert_eq!(retorno.lote.ver_aplic, VER_APLIC);
        assert!(retorno.x_motivo().starts_with(PREFIXO_MOTIVO), "{codigo}");
        assert_eq!(retorno.nfe_proc.is_some(), codigo == 100, "{codigo}");
    }
}

#[test]
fn timeout_scenario_is_a_transport_error() {
    let server = SefazSandbox::new();
    server.use_once(status(cenarios::timeout));
    assert_eq!(emissor(&server).status(Modelo::Nfe55), Err(NfeError::Transporte(TransportError::Timeout)));
}

#[test]
fn protocol_query_follows_the_document() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let emissao = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    let chave = emissao.documento.chave;

    let retorno = emissor.consultar(&chave).expect("query");
    assert_eq!(retorno.retorno.c_stat, 100);
    assert_eq!(retorno.ch_nfe.as_deref(), Some(chave.as_str()));
    assert!(retorno.protocolo.is_some());

    let outra = emissor.preparar(&nfce(proximo_numero())).expect("other");
    assert_eq!(emissor.consultar(&outra.chave).expect("query").retorno.c_stat, 217);
}

#[test]
fn cancellation_flow() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let emissao = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    let chave = emissao.documento.chave.clone();
    let protocolo = emissao.autorizacao.protocolo.and_then(|p| p.n_prot).expect("protocol");

    let errado = emissor.cancelar(&cancelamento(&chave, "135000000000000")).expect("wrong");
    assert_eq!(errado.c_stat, 222);
    assert!(!errado.registrado());

    let retorno = emissor.cancelar(&cancelamento(&chave, &protocolo)).expect("cancel");
    assert_eq!(retorno.lote.c_stat, 128);
    assert_eq!(retorno.c_stat, 135);
    assert!(retorno.registrado());
    assert_eq!(retorno.n_prot.as_ref().map(String::len), Some(15));
    assert_eq!(retorno.ch_nfe.as_deref(), Some(chave.as_str()));

    assert_eq!(emissor.consultar(&chave).expect("query").retorno.c_stat, 101);
    // By the order of Table 5-38 of the MOC, "already cancelled" (580) comes before "duplicate" (573).
    assert_eq!(emissor.cancelar(&cancelamento(&chave, &protocolo)).expect("again").c_stat, 580);

    let outra = emissor.preparar(&nfce(proximo_numero())).expect("other");
    let inexistente = emissor.cancelar(&cancelamento(&outra.chave, &protocolo)).expect("unknown");
    assert_eq!(inexistente.c_stat, 494);
}

#[test]
fn cancellation_justification_is_checked_before_sending() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let documento = emissor.preparar(&nfce(proximo_numero())).expect("document");
    let curta = Cancelamento {
        justificativa: "curta".to_string(),
        ..cancelamento(&documento.chave, "135000000000000")
    };
    assert!(matches!(emissor.cancelar(&curta), Err(NfeError::Validacao { .. })));
    assert!(server.requests().is_empty());
}

#[test]
fn tampered_event_gives_297() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let emissao = emissor.emitir(&nfce(proximo_numero())).expect("emission");
    let chave = emissao.documento.chave.clone();
    let protocolo = emissao.autorizacao.protocolo.and_then(|p| p.n_prot).expect("protocol");
    emissor.cancelar(&cancelamento(&chave, &protocolo)).expect("cancel");
    let enviado = server.requests().last().expect("request").body().to_string();
    let inicio = enviado.find("<envEvento").expect("start");
    let fim = enviado.find("</envEvento>").expect("end") + "</envEvento>".len();
    let dados = enviado[inicio..fim].replace("Venda desfeita", "Venda alterada");
    let pedido = request(Servico::Evento, "https://x/evento", &dados);
    assert_eq!(c_stat(&server.send(&pedido).expect("response")), 297);
}

#[test]
fn requests_record_what_went_out() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    emissor.status(Modelo::Nfce65).expect("status");
    emissor.emitir(&nfce(proximo_numero())).expect("emission");
    let pedidos = server.requests();
    assert_eq!(pedidos.len(), 2);
    assert_eq!(pedidos[0].servico, Some(Servico::Status));
    assert_eq!(pedidos[1].servico, Some(Servico::Autorizacao));
    assert_eq!(pedidos[1].http.url, "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeAutorizacao4.asmx");
    assert_eq!(
        pedidos[1].http.header("content-type"),
        Some(
            "application/soap+xml;charset=utf-8;action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeAutorizacao4/nfeAutorizacaoLote\""
        )
    );
    assert!(pedidos[1].body().contains("<indSinc>1</indSinc>"));
    assert!(pedidos[1].body().contains("<soap12:Envelope"));
    server.limpar_dados();
    assert!(server.requests().is_empty());
}

#[test]
fn wrong_envelope_or_action_comes_back_as_a_fault() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let documento = emissor.preparar(&nfce(proximo_numero())).expect("document");
    let dados = lote(1, &documento.xml);

    // SOAP 1.1 namespace.
    let mut pedido = request(Servico::Autorizacao, "https://x/a", &dados);
    let corpo = String::from_utf8(pedido.body)
        .expect("utf-8")
        .replace("http://www.w3.org/2003/05/soap-envelope", "http://schemas.xmlsoap.org/soap/envelope/");
    pedido.body = corpo.into_bytes();
    let resposta = server.send(&pedido).expect("response");
    assert_eq!(resposta.status, 400);
    assert!(std::str::from_utf8(&resposta.body).expect("utf-8").contains("Fault"));

    // The action of one service with the namespace of another.
    let mut pedido = request(Servico::Autorizacao, "https://x/a", &dados);
    let corpo = String::from_utf8(pedido.body)
        .expect("utf-8")
        .replace("wsdl/NFeAutorizacao4", "wsdl/NFeStatusServico4");
    pedido.body = corpo.into_bytes();
    assert_eq!(server.send(&pedido).expect("response").status, 400);

    // Broken XML.
    let pedido = HttpRequest {
        metodo: Default::default(),
        url: "https://x/a".into(),
        headers: vec![("Content-Type".into(), Servico::Autorizacao.content_type())],
        body: b"<nao-fecha>".to_vec(),
    };
    assert_eq!(c_stat(&server.send(&pedido).expect("response")), 243);
}

fn pedido_de_inutilizacao() -> HttpRequest {
    HttpRequest {
        metodo: Default::default(),
        url: "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeinutilizacao4.asmx".into(),
        headers: vec![(
            "Content-Type".into(),
            "application/soap+xml;charset=utf-8;action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeInutilizacao4/nfeInutilizacaoNF\"".into(),
        )],
        body: b"<x/>".to_vec(),
    }
}

#[test]
fn unhandled_is_an_error_by_default() {
    let server = SefazSandbox::new();
    match server.send(&pedido_de_inutilizacao()) {
        Err(TransportError::NaoTratado { url, acao }) => {
            assert!(url.contains("nfeinutilizacao4"));
            assert!(acao.expect("action").ends_with("nfeInutilizacaoNF"));
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn empty_sandbox_leaves_everything_unhandled() {
    let server = SefazSandbox::vazio();
    assert!(matches!(
        emissor(&server).status(Modelo::Nfe55),
        Err(NfeError::Transporte(TransportError::NaoTratado { .. }))
    ));
    server.use_once(consulta(cenarios::paralisado_108));
    assert!(matches!(
        emissor(&server).status(Modelo::Nfe55),
        Err(NfeError::Transporte(TransportError::NaoTratado { .. }))
    ));
}

#[test]
fn unhandled_warn_records_and_stays_local() {
    let server = SefazSandbox::new();
    server.on_unhandled(OnUnhandled::Warn);
    let resposta = server.send(&pedido_de_inutilizacao()).expect("response");
    assert_eq!(resposta.status, 404);
    assert_eq!(server.warnings().len(), 1);
    assert!(server.warnings()[0].contains("nfeInutilizacaoNF"));
}

struct Eco;

impl Transport for Eco {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse { status: 299, headers: Vec::new(), body: b"eco".to_vec() })
    }
}

#[test]
fn unhandled_passthrough_goes_to_the_transport_given() {
    let server = SefazSandbox::new();
    server.on_unhandled(OnUnhandled::Passthrough(Arc::new(Eco)));
    let resposta = server.send(&pedido_de_inutilizacao()).expect("response");
    assert_eq!(resposta.status, 299);
    assert_eq!(resposta.body, b"eco");
}

#[test]
fn sandbox_environment_refuses_a_transport_that_reaches_the_network() {
    let resultado = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Sandbox)
        .emitente(emitente("11222333000181"))
        .signer(SandboxSigner::new().expect("signer"))
        .transport(Eco)
        .build();
    assert!(matches!(resultado, Err(NfeError::Configuracao(_))));

    let externo = ravenrs_nfe::RsaSigner::from_pem(
        include_str!("../../ravenrs-dfe/fixtures/laboratorio/externo-cert.pem"),
        include_str!("../../ravenrs-dfe/fixtures/laboratorio/externo-key.pem"),
    )
    .expect("signer");
    let homologacao = Emissor::builder(BetaAck::i_understand_this_is_experimental())
        .ambiente(Ambiente::Homologacao)
        .emitente(emitente("11222333000181"))
        .signer(externo)
        .transport(Eco)
        .build()
        .expect("emitter");
    assert_eq!(homologacao.ambiente(), Ambiente::Homologacao);
    let documento = homologacao.preparar(&nfce(proximo_numero())).expect("document");
    assert!(documento.xml.contains("<tpAmb>2</tpAmb>"));
    assert_eq!(documento.origem, ravenrs_nfe::Origem::Homologacao);
}

#[test]
fn builder_needs_its_parts() {
    let ack = BetaAck::i_understand_this_is_experimental;
    assert!(matches!(Emissor::builder(ack()).build(), Err(NfeError::Configuracao(_))));
    let invalido = Emissor::builder(ack())
        .emitente(emitente("11222333000182"))
        .signer(SandboxSigner::new().expect("signer"))
        .transport(SefazSandbox::new())
        .build();
    assert!(matches!(invalido, Err(NfeError::Validacao { .. })));
}

#[test]
fn bad_drafts_are_stopped_before_anything_goes_out() {
    let server = SefazSandbox::new();
    let emissor = emissor(&server);
    let numero = proximo_numero();
    let mut sem_itens = nfce(numero);
    sem_itens.itens.clear();
    let mut gtin = nfce(numero);
    gtin.itens[0].gtin = Some("7891000315508".into());
    let mut data = nfce(numero);
    data.emitido_em = "ontem".into();
    let sem_destinatario = Rascunho { destinatario: None, ..nfe55(numero) };
    let mut sem_pagamento = nfce(numero);
    sem_pagamento.pagamentos.clear();
    let mut numero_zero = nfce(numero);
    numero_zero.numero = 0;
    for rascunho in [sem_itens, gtin, data, sem_destinatario, sem_pagamento, numero_zero] {
        assert!(matches!(emissor.emitir(&rascunho), Err(NfeError::Validacao { .. })));
    }
    assert!(server.requests().is_empty());
}

#[test]
fn observation_goes_after_the_beta_sentence_and_is_escaped() {
    let server = SefazSandbox::new();
    let rascunho = nfce(proximo_numero()).com_observacao("Entrega <amanha> & fim");
    let documento = emissor(&server).preparar(&rascunho).expect("document");
    assert!(documento.xml.contains("SEM VALOR FISCAL. Entrega &lt;amanha&gt; &amp; fim</infCpl>"));
    assert_eq!(emissor(&server).autorizar(&documento).expect("sent").c_stat(), 100);
}

#[test]
fn sandbox_works_across_threads() {
    let server = SefazSandbox::new();
    let emissor = Arc::new(emissor(&server));
    let inicio = proximo_numero() + 100_000;
    let mut threads = Vec::new();
    for deslocamento in 0..16 {
        let emissor = Arc::clone(&emissor);
        threads.push(std::thread::spawn(move || {
            emissor.emitir(&nfce(inicio + deslocamento)).expect("emission").autorizacao
        }));
    }
    let mut protocolos: Vec<String> = threads
        .into_iter()
        .map(|thread| {
            let retorno = thread.join().expect("thread");
            assert_eq!(retorno.c_stat(), 100);
            retorno.protocolo.and_then(|p| p.n_prot).expect("protocol")
        })
        .collect();
    protocolos.sort();
    protocolos.dedup();
    assert_eq!(protocolos.len(), 16);
    assert_eq!(server.requests().len(), 16);
}
