//! **BETA, proof of concept.** A supermarket checkout: everything it does comes from RavenRS crates,
//! and this is only how they are put together, with some sample records and the sandbox of the SEFAZ.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ravenrs_app::{ObjectSpace, Query};
use ravenrs_app_nfe::engine::BetaAck;
use ravenrs_app_nfe::{NFE_EMITENTE, NfeService};
use ravenrs_app_pos::{CONFIGURACAO, PRODUTO, Pos, entidades};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use ravenrs_app_ui::App;
use ravenrs_widgets::{Alert, AnyView, Tone};

/// Where the records are kept, unless `RAVEN_PDV` says otherwise.
pub fn arquivo_padrao() -> PathBuf {
    let casa = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map_or_else(PathBuf::new, PathBuf::from);
    casa.join("mini-pdv-beta.sqlite")
}

/// What it sells at first: `(code, GTIN, description, cents, NCM)`.
const PRODUTOS: [(&str, &str, &str, f64, &str); 7] = [
    ("1", "7891000053508", "Arroz branco tipo 1, 5 kg", 2790.0, "10063021"),
    ("2", "7891000244104", "Feijão carioca 1 kg", 879.0, "07133399"),
    ("3", "7891000315507", "Café torrado 500 g", 1849.0, "09012100"),
    ("4", "7891000100202", "Açúcar refinado 1 kg", 499.0, "17019900"),
    ("5", "7891000100301", "Óleo de soja 900 ml", 739.0, "15079011"),
    ("6", "7891000100400", "Sabão em barra", 450.0, "34011900"),
    ("7", "7891000100103", "Leite integral 1 L", 549.0, "04012010"),
];

/// The sample records, the first time only.
fn semear(space: &mut dyn ObjectSpace) -> Result<(), String> {
    let erro = |error: ravenrs_app::SpaceError| error.to_string();
    if space.query(PRODUTO, &Query::all()).map_err(erro)?.is_empty() {
        for (codigo, gtin, descricao, preco, ncm) in PRODUTOS {
            let mut produto = space.new_object(PRODUTO).map_err(erro)?;
            produto.set("codigo", codigo.to_owned());
            produto.set("gtin", gtin.to_owned());
            produto.set("descricao", descricao.to_owned());
            produto.set("preco", preco);
            produto.set("ncm", ncm.to_owned());
            space.save(produto).map_err(erro)?;
        }
    }
    if space.query(NFE_EMITENTE, &Query::all()).map_err(erro)?.is_empty() {
        let mut emitente = space.new_object(NFE_EMITENTE).map_err(erro)?;
        for (campo, valor) in [
            ("razao_social", "MERCADO EXEMPLO LTDA"),
            ("nome_fantasia", "Mercado Exemplo"),
            ("cnpj", "11222333000181"),
            ("inscricao_estadual", "110042490114"),
            ("logradouro", "Rua de Exemplo"),
            ("numero", "100"),
            ("bairro", "Centro"),
            ("codigo_municipio", "3550308"),
            ("municipio", "Sao Paulo"),
            ("uf", "SP"),
            ("cep", "01001000"),
        ] {
            emitente.set(campo, valor.to_owned());
        }
        space.save(emitente).map_err(erro)?;
    }
    if space.query(CONFIGURACAO, &Query::all()).map_err(erro)?.is_empty() {
        let mut configuracao = space.new_object(CONFIGURACAO).map_err(erro)?;
        configuracao.set("operador", "Maria".to_owned());
        space.save(configuracao).map_err(erro)?;
    }
    space.commit().map_err(|error| error.to_string())
}

/// The application over this file, or in memory with none, with its point of sale.
pub fn app_com_pos(arquivo: Option<&Path>) -> Result<(AnyView, Pos), String> {
    let service = NfeService::sandbox(BetaAck::i_understand_this_is_experimental()).map_err(|e| e.to_string())?;
    let pos = Pos::new(service);
    let mut app = pos.instalar(App::new("Mini-PDV")).seed(semear);
    if let Some(arquivo) = arquivo {
        let conexao = rusqlite::Connection::open(arquivo).map_err(|e| e.to_string())?;
        let mut builder = SqliteDatabase::builder(Rc::new(conexao));
        for entidade in entidades() {
            let tabela = TableMap::new(entidade.name.to_lowercase());
            builder = builder.managed(entidade, tabela).map_err(|error| error.0)?;
        }
        let database = builder.build();
        app = app.storage(move || database.space());
    }
    app.build().map(|view| (view, pos)).map_err(|error| error.0)
}

/// The window. What goes wrong on the way shows on it.
pub fn app(arquivo: Option<&Path>) -> AnyView {
    match app_com_pos(arquivo) {
        Ok((view, _)) => view,
        Err(why) => Box::new(Alert::new("Mini-PDV", why).tone(Tone::Danger)),
    }
}
