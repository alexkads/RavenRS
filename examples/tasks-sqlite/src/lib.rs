//! A task list built with ravenrs-app.
use ravenrs_app::BusinessObject;
use ravenrs_app_ui::{App, Language};

#[derive(BusinessObject)]
#[object(caption = "Tarefas", navigation = "Organização", icon = "list")]
struct Task {
    #[field(key)]
    id: i64,
    #[field(caption = "Título", default_property, required)]
    title: String,
    #[field(caption = "Concluída")]
    done: bool,
}

/// Navigation, lists and forms backed by an in-memory object space.
pub fn app() -> ravenrs::prelude::AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match database("tasks.sqlite") {
            Ok(database) => tasks().storage(move || database.space()).into_view(),
            Err(error) => Box::new(ravenrs::prelude::Text::new(error)),
        }
    }
    #[cfg(target_arch = "wasm32")]
    tasks().into_view()
}

/// The application before it is mounted.
pub fn tasks() -> App {
    App::new("Minhas tarefas").language(Language::BrazilianPortuguese).entity::<Task>()
}

/// Opens the persistent task database on the desktop.
#[cfg(not(target_arch = "wasm32"))]
pub fn database(file: &str) -> Result<ravenrs_app_sqlite::SqliteDatabase, String> {
    use ravenrs_app::BusinessObject;
    use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
    let connection = rusqlite::Connection::open(file).map_err(|e| e.to_string())?;
    Ok(SqliteDatabase::builder(std::rc::Rc::new(connection))
        .managed(Task::entity_info(), TableMap::new("tasks"))
        .map_err(|e| e.0)?
        .build())
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let view = match ravenrs::web::IndexedDbStorage::open("tasks").await {
                Ok(storage) => super::tasks().keep_in(storage).into_view(),
                Err(_) => super::tasks().into_view(),
            };
            match ravenrs::web::start(view, ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    /// Unmounts the app and releases its listeners and GPU resources.
    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}
