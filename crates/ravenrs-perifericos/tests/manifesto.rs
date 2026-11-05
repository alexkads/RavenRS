//! Manifests that must be refused, one rule at a time.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_perifericos::{ErroManifesto, Manifesto, Version, manifestos_embutidos};
use serde_json::{Value, json};

fn base() -> Value {
    json!({
        "id": "marca.modelo-1.cupom",
        "versao": "1.0.0",
        "tipo": "impressora_termica",
        "fabricante": "Marca",
        "modelos": [{ "nome": "Marca Modelo 1" }],
        "conexoes": ["usb"],
        "protocolo": { "nome": "esc_pos" },
        "situacao": { "estado": "presumido" }
    })
}

fn com(caminho: &[&str], valor: Value) -> Value {
    let mut raiz = base();
    let (ultimo, meio) = caminho.split_last().unwrap();
    let mut atual = &mut raiz;
    for parte in meio {
        if atual.get(*parte).is_none() {
            atual[*parte] = json!({});
        }
        atual = &mut atual[*parte];
    }
    atual[*ultimo] = valor;
    raiz
}

fn ler(valor: &Value) -> Result<Manifesto, ErroManifesto> {
    Manifesto::de_json(valor.to_string().as_bytes())
}

fn recusa_campo(valor: &Value, esperado: &str) {
    match ler(valor) {
        Err(ErroManifesto::Campo { campo, .. }) => assert_eq!(campo, esperado, "{valor}"),
        outro => panic!("esperava recusa em {esperado}: {outro:?} em {valor}"),
    }
}

#[test]
fn a_good_manifest_passes_and_survives_json() {
    let manifesto = ler(&base()).unwrap();
    let de_novo = Manifesto::de_json(manifesto.para_json().unwrap().as_bytes()).unwrap();
    assert_eq!(manifesto, de_novo);
    for embutido in manifestos_embutidos() {
        assert_eq!(Manifesto::de_json(embutido.para_json().unwrap().as_bytes()).unwrap(), embutido);
    }
}

#[test]
fn the_id_must_be_stable_and_clean() {
    for id in ["semPonto", "a.b", "a.b.c.d", "A.b.c", "../x.y.z", "a.b c.d", "a..c", "-a.b.c"] {
        recusa_campo(&com(&["id"], json!(id)), "id");
    }
}

#[test]
fn the_exact_model_is_mandatory_and_cannot_be_vague() {
    recusa_campo(&com(&["modelos"], json!([])), "modelos");
    let sem_modelo = {
        let mut valor = base();
        valor.as_object_mut().unwrap().remove("modelos");
        valor
    };
    assert!(matches!(ler(&sem_modelo), Err(ErroManifesto::Formato(_))));
    for vago in ["Marca", "marca", "TS", "Genérico", "todos", "qualquer"] {
        recusa_campo(&com(&["modelos"], json!([{ "nome": vago }])), "modelos.nome");
    }
    // "TSC" as a model of TSC: the case that motivated the rule.
    let tsc = com(&["fabricante"], json!("TSC"));
    let mut tsc = tsc;
    tsc["modelos"] = json!([{ "nome": "TSC" }]);
    recusa_campo(&tsc, "modelos.nome");
}

#[test]
fn nothing_can_be_a_path() {
    for caminho in ["/etc/passwd", "..\\x", "a/b", "C:\\driver.dll", "x..y"] {
        recusa_campo(&com(&["fabricante"], json!(caminho)), "fabricante");
    }
    // A sentence may say ESC/POS, and still no path or address.
    assert!(ler(&com(&["observacoes"], json!("Só em ESC/POS."))).is_ok());
    for caminho in
        ["/etc/passwd", "leia em /tmp/x", "..\\x", "C:\\driver.dll", "https://exemplo.invalid", "x..y"]
    {
        recusa_campo(&com(&["observacoes"], json!(caminho)), "observacoes");
        recusa_campo(&com(&["instalacao"], json!(caminho)), "instalacao");
    }
    let fonte = |texto: &str| com(&["situacao"], json!({ "estado": "so_pela_documentacao", "fonte": texto }));
    assert!(ler(&fonte("Manual, https://exemplo.invalid/a.pdf")).is_ok());
    for ruim in ["/etc/passwd", "..\\x", "", "a..b"] {
        recusa_campo(&fonte(ruim), "situacao.fonte");
    }
}

#[test]
fn unknown_fields_do_not_get_in() {
    for campo in ["script", "codigo", "url", "biblioteca"] {
        let valor = com(&[campo], json!("baixar e executar"));
        assert!(matches!(ler(&valor), Err(ErroManifesto::Formato(_))), "{campo}");
    }
    let dentro = com(&["ajustes", "executar"], json!("x"));
    assert!(matches!(ler(&dentro), Err(ErroManifesto::Formato(_))));
}

#[test]
fn size_and_counts_have_limits() {
    let grande = com(&["observacoes"], json!("a".repeat(20_000)));
    assert!(matches!(ler(&grande), Err(ErroManifesto::Grande { .. })));
    recusa_campo(&com(&["observacoes"], json!("a".repeat(1001))), "observacoes");
    recusa_campo(&com(&["fabricante"], json!("a".repeat(121))), "fabricante");
    let muitos: Vec<Value> = (0..33).map(|i| json!({ "nome": format!("Modelo {i}") })).collect();
    recusa_campo(&com(&["modelos"], json!(muitos)), "modelos");
    recusa_campo(&com(&["conexoes"], json!([])), "conexoes");
    recusa_campo(&com(&["fabricante"], json!("com\ncontrole")), "fabricante");
}

fn proibido(valor: &Value, esperado: &str) {
    match ler(valor) {
        Err(ErroManifesto::ComandoProibido { campo, .. }) => assert_eq!(campo, esperado),
        outro => panic!("esperava comando proibido: {outro:?}"),
    }
}

#[test]
fn commands_are_only_what_the_kind_of_device_may_send() {
    // A second command hidden after the cut, and things no slot accepts.
    proibido(&com(&["ajustes", "corte_total"], json!("1D 56 42 18 1B 40")), "ajustes.corte_total");
    proibido(&com(&["ajustes", "corte_total"], json!("1B 40")), "ajustes.corte_total");
    proibido(&com(&["ajustes", "bipe"], json!("1D 56 42 18")), "ajustes.bipe");
    proibido(&com(&["ajustes", "gaveta"], json!("1D 56 42 18")), "ajustes.gaveta");
    // The mode change that writes to the printer's memory.
    proibido(&com(&["ajustes", "comando_modo"], json!("1D F9 35 01")), "ajustes.comando_modo");
    assert!(ler(&com(&["ajustes", "comando_modo"], json!("1D F9 20 01"))).is_ok());

    // A pinpad sends no printer bytes, whatever they are.
    let mut pinpad = com(&["ajustes", "corte_total"], json!("1D 56 42 18"));
    pinpad["tipo"] = json!("pinpad");
    pinpad["protocolo"] = json!({ "nome": "abecs", "versao": "2.20" });
    proibido(&pinpad, "ajustes.corte_total");
    // A label printer gets no receipt cut either.
    let mut etiqueta = com(&["ajustes", "corte_parcial"], json!("1D 56 42 18"));
    etiqueta["tipo"] = json!("etiquetadora");
    etiqueta["protocolo"] = json!({ "nome": "zpl" });
    proibido(&etiqueta, "ajustes.corte_parcial");
}

#[test]
fn a_command_too_long_or_not_hex_does_not_parse() {
    for ruim in ["ZZ", "1D5", &"00 ".repeat(17)] {
        let valor = com(&["ajustes", "bipe"], json!(ruim));
        assert!(matches!(ler(&valor), Err(ErroManifesto::Formato(_))), "{ruim}");
    }
}

#[test]
fn values_must_fit_the_device() {
    recusa_campo(&com(&["padroes", "dpi"], json!(600)), "padroes.dpi");
    recusa_campo(&com(&["padroes", "largura_pontos"], json!(5000)), "padroes.largura_pontos");
    recusa_campo(&com(&["padroes", "baud"], json!(1234)), "padroes.baud");
    recusa_campo(&com(&["ajustes", "tamanho_bloco"], json!(0)), "ajustes.tamanho_bloco");
    recusa_campo(&com(&["ajustes", "tamanho_bloco"], json!(5000)), "ajustes.tamanho_bloco");
    recusa_campo(&com(&["protocolo"], json!({ "nome": "zpl" })), "protocolo");
    recusa_campo(&com(&["capacidades", "pix_no_pinpad"], json!(true)), "capacidades.pix_no_pinpad");

    let mut pinpad = base();
    pinpad["tipo"] = json!("pinpad");
    pinpad["protocolo"] = json!({ "nome": "abecs" });
    recusa_campo(&pinpad, "protocolo.versao");
    pinpad["protocolo"] = json!({ "nome": "abecs", "versao": "2.20" });
    pinpad["capacidades"] = json!({ "corta": true });
    recusa_campo(&pinpad, "capacidades");
}

#[test]
fn a_device_test_must_say_when_who_and_on_what() {
    let teste = |data: &str, por: &str, firmware: &str| {
        com(
            &["situacao"],
            json!({ "estado": "testado_no_aparelho", "data": data, "por": por, "firmware": firmware }),
        )
    };
    assert!(ler(&teste("2026-11-05", "Alex", "1.2")).is_ok());
    recusa_campo(&teste("ontem", "Alex", "1.2"), "situacao.data");
    recusa_campo(&teste("2026-02-30", "Alex", "1.2"), "situacao.data");
    recusa_campo(&teste("2026-11-05", " ", "1.2"), "situacao.por");
    recusa_campo(&teste("2026-11-05", "Alex", ""), "situacao.firmware");
}

#[test]
fn compatibility_compares_versions_as_numbers() {
    let manifesto = ler(&com(&["min_versao_programa"], json!("27.10.0"))).unwrap();
    assert!(!manifesto.compativel_com(&Version::new(27, 9, 9)), "9 is below 10, unlike as text");
    assert!(manifesto.compativel_com(&Version::new(27, 10, 0)));
    assert!(manifesto.compativel_com(&Version::new(28, 0, 0)));
    assert!(ler(&base()).unwrap().compativel_com(&Version::new(0, 0, 1)));
    assert!(matches!(ler(&com(&["versao"], json!("1.0"))), Err(ErroManifesto::Formato(_))));
}
