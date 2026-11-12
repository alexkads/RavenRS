//! Installing, activating, going back, and keeping it all through a restart.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_perifericos::{
    AcaoHistorico, Armazem, ArmazemArquivo, ArmazemMemoria, Dispositivo, ErroArmazem, ErroRegistro, Evento,
    Manifesto, Registro, Situacao, TipoPeriferico, Version,
};
use serde_json::json;

const ID: &str = "marca.modelo-1.cupom";

fn manifesto(versao: &str) -> Manifesto {
    let valor = json!({
        "id": ID,
        "versao": versao,
        "tipo": "impressora_termica",
        "fabricante": "Marca",
        "modelos": [{ "nome": "Marca Modelo 1" }],
        "conexoes": ["usb"],
        "protocolo": { "nome": "esc_pos" },
        "situacao": { "estado": "presumido" }
    });
    Manifesto::de_json(valor.to_string().as_bytes()).unwrap()
}

fn v(texto: &str) -> Version {
    Version::parse(texto).unwrap()
}

fn programa() -> Version {
    v("27.1.13")
}

#[test]
fn the_first_version_is_active_and_later_ones_wait() {
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"));
    registro.instalar(manifesto("1.1.0")).unwrap();
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"), "nothing changes by itself");
    assert_eq!(registro.versoes(ID).len(), 2);
    registro.ativar(ID, &v("1.1.0")).unwrap();
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.1.0"));
}

#[test]
fn going_back_follows_what_was_in_use() {
    let mut registro = Registro::vazio(programa());
    for versao in ["1.0.0", "1.1.0", "1.2.0"] {
        registro.instalar(manifesto(versao)).unwrap();
    }
    registro.ativar(ID, &v("1.2.0")).unwrap();
    registro.ativar(ID, &v("1.1.0")).unwrap();
    assert_eq!(registro.voltar(ID).unwrap(), v("1.2.0"));
    assert_eq!(registro.voltar(ID).unwrap(), v("1.0.0"));
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"));
    assert_eq!(registro.voltar(ID), Err(ErroRegistro::SemAnterior(ID.to_owned())));
    assert_eq!(registro.voltar("x.y.z"), Err(ErroRegistro::DriverDesconhecido("x.y.z".to_owned())));
}

#[test]
fn history_says_what_happened() {
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.instalar(manifesto("1.1.0")).unwrap();
    registro.ativar(ID, &v("1.1.0")).unwrap();
    registro.voltar(ID).unwrap();
    let evento = |acao, versao| Evento { acao, versao: v(versao) };
    assert_eq!(
        registro.historico(ID),
        [
            evento(AcaoHistorico::Instalado, "1.0.0"),
            evento(AcaoHistorico::Ativado, "1.0.0"),
            evento(AcaoHistorico::Instalado, "1.1.0"),
            evento(AcaoHistorico::Ativado, "1.1.0"),
            evento(AcaoHistorico::Voltou, "1.0.0"),
        ]
    );
}

#[test]
fn a_driver_for_a_newer_program_is_refused_clearly() {
    let mut registro = Registro::vazio(v("27.1.13"));
    let mut novo = manifesto("2.0.0");
    novo.min_versao_programa = Some(v("28.0.0"));
    let erro = registro.instalar(novo.clone()).unwrap_err();
    assert!(matches!(erro, ErroRegistro::ProgramaAntigo { .. }), "{erro:?}");
    let texto = erro.to_string();
    assert!(texto.contains("28.0.0") && texto.contains("27.1.13") && texto.contains(ID), "{texto}");
    assert!(registro.ativo(ID).is_none());

    let mut depois = Registro::vazio(v("28.0.0"));
    depois.instalar(novo).unwrap();
    assert_eq!(depois.ativo(ID).unwrap().versao, v("2.0.0"));
}

#[test]
fn an_older_program_cannot_activate_what_it_cannot_run() {
    let armazem = ArmazemMemoria::default();
    let mut novo = Registro::vazio(v("28.0.0"));
    novo.instalar(manifesto("1.0.0")).unwrap();
    let mut exigente = manifesto("2.0.0");
    exigente.min_versao_programa = Some(v("28.0.0"));
    novo.instalar(exigente).unwrap();
    novo.salvar(&armazem).unwrap();
    let mut antigo = Registro::carregar(v("27.0.0"), &armazem).unwrap();
    assert!(matches!(antigo.ativar(ID, &v("2.0.0")), Err(ErroRegistro::ProgramaAntigo { .. })));
    assert_eq!(antigo.ativo(ID).unwrap().versao, v("1.0.0"));
}

#[test]
fn what_cannot_be_activated_is_named() {
    let mut registro = Registro::vazio(programa());
    assert_eq!(registro.ativar(ID, &v("1.0.0")), Err(ErroRegistro::DriverDesconhecido(ID.to_owned())));
    registro.instalar(manifesto("1.0.0")).unwrap();
    assert_eq!(
        registro.ativar(ID, &v("9.0.0")),
        Err(ErroRegistro::VersaoNaoInstalada { id: ID.to_owned(), versao: v("9.0.0") })
    );
}

#[test]
fn a_version_never_changes_its_contents() {
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.instalar(manifesto("1.0.0")).unwrap();
    assert_eq!(registro.versoes(ID).len(), 1, "the same again is no change");
    let mut outro = manifesto("1.0.0");
    outro.fabricante = "Outra".to_owned();
    assert_eq!(
        registro.instalar(outro),
        Err(ErroRegistro::VersaoConflitante { id: ID.to_owned(), versao: v("1.0.0") })
    );
}

#[test]
fn an_invalid_manifest_is_not_installed_even_if_built_in_code() {
    let mut registro = Registro::vazio(programa());
    let mut ruim = manifesto("1.0.0");
    ruim.modelos.clear();
    assert!(matches!(registro.instalar(ruim), Err(ErroRegistro::ManifestoInvalido(_))));
}

#[test]
fn the_built_in_registry_has_exact_models_and_honest_status() {
    let registro = Registro::novo(programa());
    let bematech = registro.ativo("bematech.mp-4200-th.cupom").unwrap();
    assert_eq!(bematech.nome_do_modelo(), "Bematech MP-4200 TH");
    assert!(matches!(bematech.situacao, Situacao::SoPelaDocumentacao { .. }));
    assert!(matches!(registro.ativo("elgin.l42-pro.etiqueta").unwrap().situacao, Situacao::Presumido));
    assert_eq!(registro.ativo("generico.hid-teclado.leitor").unwrap().tipo, TipoPeriferico::LeitorCodigo);
    for id in ["gertec.ppc930.pinpad", "ingenico.lane-3000.pinpad"] {
        let pinpad = registro.ativo(id).unwrap();
        assert_eq!(pinpad.tipo, TipoPeriferico::Pinpad);
        assert!(!pinpad.capacidades.pix_no_pinpad, "{id}: não confirmado");
    }
    assert_eq!(registro.ativos().count(), 12);
}

#[test]
fn a_device_runs_on_its_pinned_version_or_the_active_one() {
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.instalar(manifesto("1.1.0")).unwrap();
    registro.ativar(ID, &v("1.1.0")).unwrap();
    let mut caixa = Dispositivo {
        apelido: "Caixa 1".to_owned(),
        driver_id: ID.to_owned(),
        versao_fixada: None,
        conexao: "192.168.0.50".to_owned(),
    };
    assert_eq!(registro.driver_de(&caixa).unwrap().versao, v("1.1.0"));
    caixa.versao_fixada = Some(v("1.0.0"));
    assert_eq!(registro.driver_de(&caixa).unwrap().versao, v("1.0.0"));
    caixa.versao_fixada = Some(v("9.9.9"));
    assert!(matches!(registro.driver_de(&caixa), Err(ErroRegistro::VersaoNaoInstalada { .. })));
    caixa.driver_id = "x.y.z".to_owned();
    caixa.versao_fixada = None;
    assert!(matches!(registro.driver_de(&caixa), Err(ErroRegistro::DriverDesconhecido(_))));

    let json = serde_json::to_string(&caixa).unwrap();
    assert_eq!(serde_json::from_str::<Dispositivo>(&json).unwrap(), caixa);
}

fn pasta_temporaria(nome: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ravenrs-perifericos-{}-{nome}", std::process::id()))
}

#[test]
fn everything_survives_a_restart_in_a_file() {
    let pasta = pasta_temporaria("reinicio");
    let armazem = ArmazemArquivo::novo(pasta.join("sub").join("drivers.json"));
    assert_eq!(armazem.ler().unwrap(), None);

    let mut registro = Registro::novo(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.instalar(manifesto("1.1.0")).unwrap();
    registro.ativar(ID, &v("1.1.0")).unwrap();
    registro.ativar("bematech.mp-4200-th.cupom", &v("1.0.0")).unwrap();
    registro.salvar(&armazem).unwrap();

    let mut lido = Registro::carregar(programa(), &armazem).unwrap();
    assert_eq!(lido, registro);
    assert_eq!(lido.voltar(ID).unwrap(), v("1.0.0"), "going back still works after a restart");

    let sobras: Vec<_> = std::fs::read_dir(pasta.join("sub"))
        .unwrap()
        .map(|entrada| entrada.unwrap().file_name())
        .filter(|nome| nome.to_string_lossy().ends_with(".tmp"))
        .collect();
    let _ = std::fs::remove_dir_all(&pasta);
    assert!(sobras.is_empty(), "{sobras:?}");
}

#[test]
fn a_failed_write_leaves_the_old_file_whole() {
    let pasta = pasta_temporaria("falha");
    std::fs::create_dir_all(&pasta).unwrap();
    // The destination is a folder, so the final step of the write fails.
    let destino = pasta.join("drivers.json");
    std::fs::create_dir(&destino).unwrap();
    let armazem = ArmazemArquivo::novo(&destino);
    let falhou = armazem.gravar(b"novo");
    let sobras = std::fs::read_dir(&pasta).unwrap().count();
    let _ = std::fs::remove_dir_all(&pasta);
    assert!(matches!(falhou, Err(ErroArmazem::Acesso(_))), "{falhou:?}");
    assert_eq!(sobras, 1, "the temporary file does not stay");
}

#[test]
fn writing_replaces_and_nothing_else_remains() {
    let pasta = pasta_temporaria("troca");
    let armazem = ArmazemArquivo::novo(pasta.join("drivers.json"));
    armazem.gravar(b"um").unwrap();
    armazem.gravar(b"dois").unwrap();
    let lido = armazem.ler().unwrap();
    let entradas = std::fs::read_dir(&pasta).unwrap().count();
    let _ = std::fs::remove_dir_all(&pasta);
    assert_eq!(lido.as_deref(), Some(&b"dois"[..]));
    assert_eq!(entradas, 1);
}

#[test]
fn a_program_update_brings_new_drivers_without_changing_the_active_ones() {
    let armazem = ArmazemMemoria::default();
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.salvar(&armazem).unwrap();
    let carregado = Registro::carregar(programa(), &armazem).unwrap();
    assert_eq!(carregado.ativo(ID).unwrap().versao, v("1.0.0"));
    assert!(carregado.ativo("epson.tm-t20ii.cupom").is_some(), "embedded drivers come along");
}

#[test]
fn a_tampered_store_is_refused() {
    let cases =
        [(&b"nao e json"[..], "json"), (br#"{"formato":99,"ultima_sequencia":0,"drivers":{}}"#, "formato")];
    for (bytes, nome) in cases {
        let armazem = ArmazemMemoria::default();
        armazem.gravar(bytes).unwrap();
        assert!(matches!(Registro::carregar(programa(), &armazem), Err(ErroArmazem::Conteudo(_))), "{nome}");
    }

    // A forbidden command written straight into the file.
    let armazem = ArmazemMemoria::default();
    let mut registro = Registro::vazio(programa());
    registro.instalar(manifesto("1.0.0")).unwrap();
    registro.salvar(&armazem).unwrap();
    let texto = String::from_utf8(armazem.ler().unwrap().unwrap()).unwrap();
    let adulterado = texto.replace("\"Marca Modelo 1\"", "\"Marca\"");
    assert_ne!(texto, adulterado);
    armazem.gravar(adulterado.as_bytes()).unwrap();
    assert!(matches!(Registro::carregar(programa(), &armazem), Err(ErroArmazem::Conteudo(_))));
}
