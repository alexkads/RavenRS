//! A whole application from its records: navigation, lists, forms and commands.

use ravenrs_app::{BusinessEnum, BusinessObject, ObjectSpaceExt};
use ravenrs_app_ui::{App, Language};

#[derive(BusinessEnum)]
enum AccountKind {
    #[option(caption = "Corrente")]
    Checking,
    #[option(caption = "Poupança")]
    Savings,
}

#[derive(BusinessObject)]
#[object(caption = "Bancos", navigation = "Cadastros", icon = "folder")]
struct Bank {
    #[field(typed_key, caption = "Código")]
    code: String,
    #[field(caption = "Nome", default_property)]
    name: String,
}

#[derive(BusinessObject)]
#[object(caption = "Contas", navigation = "Cadastros", icon = "wallet")]
struct Account {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property)]
    name: String,
    #[field(caption = "Tipo")]
    kind: AccountKind,
    #[field(caption = "Banco", reference = "Bank")]
    bank: Option<String>,
    #[field(caption = "Saldo inicial", money)]
    opening: i64,
    #[field(caption = "Observações", long)]
    notes: Option<String>,
}

/// The window, with some records to start from, kept in memory.
pub fn app() -> ravenrs::prelude::AnyView {
    accounts().into_view()
}

/// The application, before it is put in a window: the browser build keeps its records.
pub fn accounts() -> App {
    App::new("Raven Contas")
        .language(Language::BrazilianPortuguese)
        .entity::<Account>()
        .entity::<Bank>()
        .seed(|space| {
            let bank = Bank { code: "341".into(), name: "Itaú".into() };
            space.add(&bank).map_err(|e| e.to_string())?;
            let bank = Some(bank.code);
            let first = Account {
                id: 0,
                name: "Conta corrente".into(),
                kind: AccountKind::Checking,
                bank,
                opening: 125_050,
                notes: None,
            };
            space.add(&first).map_err(|e| e.to_string())
        })
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            // The records stay in the browser between visits; without IndexedDB, in memory only.
            let view = match ravenrs::web::IndexedDbStorage::open("raven-contas").await {
                Ok(storage) => super::accounts().keep_in(storage).into_view(),
                Err(_) => super::app(),
            };
            match ravenrs::web::start(view, ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => std::mem::forget(app),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
