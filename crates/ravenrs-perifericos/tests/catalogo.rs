//! Every way a catalog can be refused, and the path of an accepted one. The keys are made on the spot.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use ravenrs_perifericos::{
    ArmazemMemoria, Atualizacao, Catalogo, ChavePublica, ErroCatalogo, ErroManifesto, ErroRegistro,
    ErroTransporte, Registro, TransporteCatalogo, Version, assinar, chave_publica_de, verificar_atualizacoes,
};
use serde_json::{Value, json};

const ID: &str = "marca.modelo-1.cupom";
const HOJE: &str = "2026-11-05";

fn chave_nova() -> [u8; 32] {
    let mut chave = [0; 32];
    for parte in chave.chunks_mut(8) {
        let aleatorio = RandomState::new().build_hasher().finish().to_le_bytes();
        parte.copy_from_slice(&aleatorio);
    }
    chave
}

fn v(texto: &str) -> Version {
    Version::parse(texto).unwrap()
}

fn manifesto(versao: &str) -> Value {
    json!({
        "id": ID,
        "versao": versao,
        "tipo": "impressora_termica",
        "fabricante": "Marca",
        "modelos": [{ "nome": "Marca Modelo 1" }],
        "conexoes": ["usb"],
        "protocolo": { "nome": "esc_pos" },
        "situacao": { "estado": "presumido" }
    })
}

fn item(manifesto: Value, notas: &str) -> Value {
    json!({ "manifesto": manifesto, "notas": notas })
}

fn conteudo(sequencia: u64, validade: &str, drivers: Vec<Value>) -> String {
    json!({ "formato": 1, "sequencia": sequencia, "validade": validade, "drivers": drivers }).to_string()
}

struct Editor {
    privada: [u8; 32],
    publica: ChavePublica,
}

impl Editor {
    fn novo() -> Self {
        let privada = chave_nova();
        Self { privada, publica: chave_publica_de(&privada) }
    }

    fn catalogo(&self, sequencia: u64, validade: &str, drivers: Vec<Value>) -> Vec<u8> {
        assinar(&conteudo(sequencia, validade, drivers), &self.privada).unwrap().into_bytes()
    }
}

fn programa() -> Version {
    v("27.1.13")
}

fn registro_com(versao: &str) -> Registro {
    let mut registro = Registro::vazio(programa());
    registro.instalar(serde_json::from_value(manifesto(versao)).unwrap()).unwrap();
    registro
}

#[test]
fn a_good_catalog_opens() {
    let editor = Editor::novo();
    let bytes = editor.catalogo(7, "2026-12-31", vec![item(manifesto("1.1.0"), "Corrige o corte")]);
    let catalogo = Catalogo::abrir(&bytes, &editor.publica, HOJE, 0).unwrap();
    assert_eq!(catalogo.sequencia(), 7);
    assert_eq!(catalogo.validade(), "2026-12-31");
    assert_eq!(catalogo.itens()[0].notas, "Corrige o corte");
    assert_eq!(ChavePublica::de_hex(&editor.publica.para_hex()), Some(editor.publica));
}

#[test]
fn another_key_is_refused() {
    let (editor, outro) = (Editor::novo(), Editor::novo());
    let bytes = editor.catalogo(1, "2026-12-31", vec![]);
    assert_eq!(Catalogo::abrir(&bytes, &outro.publica, HOJE, 0), Err(ErroCatalogo::AssinaturaInvalida));
}

#[test]
fn changed_contents_are_refused() {
    let editor = Editor::novo();
    let bytes = editor.catalogo(1, "2026-12-31", vec![item(manifesto("1.1.0"), "x")]);
    let texto = String::from_utf8(bytes).unwrap();
    // The validity moved forward, with the signature kept.
    let adulterado = texto.replace("2026-12-31", "2099-12-31");
    assert_ne!(texto, adulterado);
    assert_eq!(
        Catalogo::abrir(adulterado.as_bytes(), &editor.publica, HOJE, 0),
        Err(ErroCatalogo::AssinaturaInvalida)
    );
}

#[test]
fn a_signature_that_is_not_one_is_refused() {
    let editor = Editor::novo();
    for assinatura in ["", "zz", "00", &"00".repeat(64)] {
        let bytes =
            json!({ "conteudo": conteudo(1, "2026-12-31", vec![]), "assinatura": assinatura }).to_string();
        assert_eq!(
            Catalogo::abrir(bytes.as_bytes(), &editor.publica, HOJE, 0),
            Err(ErroCatalogo::AssinaturaInvalida),
            "{assinatura}"
        );
    }
}

#[test]
fn an_expired_catalog_is_refused_after_its_last_day() {
    let editor = Editor::novo();
    let bytes = editor.catalogo(1, HOJE, vec![]);
    assert!(Catalogo::abrir(&bytes, &editor.publica, HOJE, 0).is_ok(), "its last day still counts");
    assert_eq!(
        Catalogo::abrir(&bytes, &editor.publica, "2026-11-06", 0),
        Err(ErroCatalogo::Vencido { validade: HOJE.to_owned(), hoje: "2026-11-06".to_owned() })
    );
}

#[test]
fn an_older_sequence_is_refused_and_the_same_is_not() {
    let editor = Editor::novo();
    let bytes = editor.catalogo(5, "2026-12-31", vec![]);
    assert!(Catalogo::abrir(&bytes, &editor.publica, HOJE, 5).is_ok());
    assert_eq!(
        Catalogo::abrir(&bytes, &editor.publica, HOJE, 6),
        Err(ErroCatalogo::SequenciaAntiga { recebida: 5, ultima: 6 })
    );
}

#[test]
fn the_registry_remembers_the_sequence_through_a_restart() {
    let editor = Editor::novo();
    let armazem = ArmazemMemoria::default();
    let mut registro = Registro::vazio(programa());
    registro.abrir_catalogo(&editor.catalogo(9, "2026-12-31", vec![]), &editor.publica, HOJE).unwrap();
    assert_eq!(registro.ultima_sequencia(), 9);
    registro.salvar(&armazem).unwrap();
    let mut depois = Registro::carregar(programa(), &armazem).unwrap();
    assert_eq!(
        depois.abrir_catalogo(&editor.catalogo(8, "2026-12-31", vec![]), &editor.publica, HOJE),
        Err(ErroCatalogo::SequenciaAntiga { recebida: 8, ultima: 9 })
    );
    assert_eq!(depois.ultima_sequencia(), 9, "a refused catalog changes nothing");
}

#[test]
fn an_invalid_driver_refuses_the_whole_catalog_even_signed() {
    let editor = Editor::novo();
    let mut sem_modelo = manifesto("1.1.0");
    sem_modelo["modelos"] = json!([]);
    let mut comando = manifesto("1.2.0");
    comando["ajustes"] = json!({ "corte_total": "1B 40" });
    let mut mal_formado = manifesto("1.3.0");
    mal_formado["script"] = json!("rm -rf");
    let casos = [(sem_modelo, "campo"), (comando, "comando"), (mal_formado, "formato")];
    for (ruim, nome) in casos {
        let bytes = editor.catalogo(1, "2026-12-31", vec![item(manifesto("1.0.1"), ""), item(ruim, "")]);
        match Catalogo::abrir(&bytes, &editor.publica, HOJE, 0) {
            Err(ErroCatalogo::ManifestoInvalido { indice: 1, erro }) => match nome {
                "campo" => assert!(matches!(erro, ErroManifesto::Campo { .. }), "{erro:?}"),
                "comando" => assert!(matches!(erro, ErroManifesto::ComandoProibido { .. }), "{erro:?}"),
                _ => assert!(matches!(erro, ErroManifesto::Formato(_)), "{erro:?}"),
            },
            outro => panic!("{nome}: {outro:?}"),
        }
    }
}

#[test]
fn shape_and_size_have_their_own_errors() {
    let editor = Editor::novo();
    let abrir = |bytes: &[u8]| Catalogo::abrir(bytes, &editor.publica, HOJE, 0);
    assert!(matches!(abrir(b"nada"), Err(ErroCatalogo::Formato(_))));
    assert!(matches!(abrir(&vec![b' '; 5 * 1024 * 1024]), Err(ErroCatalogo::Grande { .. })));
    let assinado = |texto: &str| assinar(texto, &editor.privada).unwrap().into_bytes();
    assert!(matches!(abrir(&assinado("nao e json")), Err(ErroCatalogo::Formato(_))));
    let formato_novo = json!({ "formato": 2, "sequencia": 1, "validade": "2026-12-31", "drivers": [] });
    assert!(matches!(abrir(&assinado(&formato_novo.to_string())), Err(ErroCatalogo::Formato(_))));
    assert!(matches!(abrir(&editor.catalogo(1, "fim do ano", vec![])), Err(ErroCatalogo::Formato(_))));
}

#[test]
fn an_update_is_offered_and_only_applied_when_accepted() {
    let editor = Editor::novo();
    let mut registro = registro_com("1.0.0");
    let bytes = editor.catalogo(
        2,
        "2026-12-31",
        vec![
            item(manifesto("1.1.0"), "Intermediária"),
            item(manifesto("1.2.0"), "Corrige o bipe"),
            item(manifesto("0.9.0"), "Antiga"),
        ],
    );
    let catalogo = registro.abrir_catalogo(&bytes, &editor.publica, HOJE).unwrap();
    let ofertas = verificar_atualizacoes(&catalogo, &registro);
    assert_eq!(
        ofertas,
        [Atualizacao {
            id: ID.to_owned(),
            de: Some(v("1.0.0")),
            para: v("1.2.0"),
            notas: "Corrige o bipe".to_owned(),
        }]
    );
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"), "offering changes nothing");
    assert_eq!(registro.versoes(ID).len(), 1);

    registro.aceitar(&catalogo, &ofertas[0]).unwrap();
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.2.0"));
    assert!(verificar_atualizacoes(&catalogo, &registro).is_empty());

    assert_eq!(registro.voltar(ID).unwrap(), v("1.0.0"));
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"));
}

#[test]
fn a_new_driver_is_offered_from_nothing() {
    let editor = Editor::novo();
    let registro = Registro::vazio(programa());
    let bytes = editor.catalogo(1, "2026-12-31", vec![item(manifesto("1.0.0"), "Novo")]);
    let catalogo = Catalogo::abrir(&bytes, &editor.publica, HOJE, 0).unwrap();
    let ofertas = verificar_atualizacoes(&catalogo, &registro);
    assert_eq!(ofertas.len(), 1);
    assert_eq!(ofertas[0].de, None);
}

#[test]
fn a_version_for_a_newer_program_is_not_offered_and_cannot_be_accepted() {
    let editor = Editor::novo();
    let mut exigente = manifesto("2.0.0");
    exigente["min_versao_programa"] = json!("28.0.0");
    let mut registro = registro_com("1.0.0");
    let bytes =
        editor.catalogo(1, "2026-12-31", vec![item(manifesto("1.1.0"), "Serve"), item(exigente, "Futura")]);
    let catalogo = Catalogo::abrir(&bytes, &editor.publica, HOJE, 0).unwrap();
    let ofertas = verificar_atualizacoes(&catalogo, &registro);
    assert_eq!(ofertas.len(), 1);
    assert_eq!(ofertas[0].para, v("1.1.0"), "the newest this program can run");

    let forcada = Atualizacao { id: ID.to_owned(), de: None, para: v("2.0.0"), notas: String::new() };
    assert!(matches!(registro.aceitar(&catalogo, &forcada), Err(ErroRegistro::ProgramaAntigo { .. })));
    let inexistente = Atualizacao { id: ID.to_owned(), de: None, para: v("3.0.0"), notas: String::new() };
    assert!(matches!(registro.aceitar(&catalogo, &inexistente), Err(ErroRegistro::ForaDoCatalogo { .. })));
    assert_eq!(registro.ativo(ID).unwrap().versao, v("1.0.0"));
}

struct Fixture(Result<Vec<u8>, ErroTransporte>);

impl TransporteCatalogo for Fixture {
    fn baixar(&self) -> Result<Vec<u8>, ErroTransporte> {
        self.0.clone()
    }
}

#[test]
fn a_catalog_can_come_through_any_transport() {
    let editor = Editor::novo();
    let mut registro = registro_com("1.0.0");
    let bytes = editor.catalogo(3, "2026-12-31", vec![item(manifesto("1.1.0"), "x")]);
    let catalogo = registro.buscar_catalogo(&Fixture(Ok(bytes)), &editor.publica, HOJE).unwrap();
    assert_eq!(catalogo.sequencia(), 3);
    assert_eq!(registro.ultima_sequencia(), 3);

    let falha = ErroTransporte("sem rede".to_owned());
    assert_eq!(
        registro.buscar_catalogo(&Fixture(Err(falha.clone())), &editor.publica, HOJE),
        Err(ErroCatalogo::Transporte(falha))
    );
}

#[cfg(feature = "http")]
mod http {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use ravenrs_perifericos::TransporteHttp;

    use super::*;

    /// A server on this machine that answers once.
    fn servidor(status: &'static str, corpo: Vec<u8>) -> String {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut conexao, _) = ouvinte.accept().unwrap();
            let mut pedido = [0; 2048];
            let _ = conexao.read(&mut pedido);
            let cabecalho =
                format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corpo.len());
            conexao.write_all(cabecalho.as_bytes()).unwrap();
            conexao.write_all(&corpo).unwrap();
        });
        format!("http://127.0.0.1:{porta}/catalogo.json")
    }

    #[test]
    fn a_catalog_comes_over_http_and_is_still_checked() {
        let editor = Editor::novo();
        let mut registro = registro_com("1.0.0");
        let bytes = editor.catalogo(4, "2026-12-31", vec![item(manifesto("1.1.0"), "x")]);
        let transporte = TransporteHttp::novo(servidor("200 OK", bytes.clone()));
        let catalogo = registro.buscar_catalogo(&transporte, &editor.publica, HOJE).unwrap();
        assert_eq!(catalogo.sequencia(), 4);

        let outro = Editor::novo();
        let transporte = TransporteHttp::novo(servidor("200 OK", bytes));
        assert_eq!(
            Registro::vazio(programa()).buscar_catalogo(&transporte, &outro.publica, HOJE),
            Err(ErroCatalogo::AssinaturaInvalida),
            "the address does not make it trusted"
        );
    }

    #[test]
    fn an_error_from_the_server_is_a_transport_error() {
        let editor = Editor::novo();
        let transporte = TransporteHttp::novo(servidor("404 Not Found", Vec::new()));
        let resposta = Registro::vazio(programa()).buscar_catalogo(&transporte, &editor.publica, HOJE);
        assert!(matches!(resposta, Err(ErroCatalogo::Transporte(_))), "{resposta:?}");
    }
}
