//! The feature center: every feature of the application framework, one small demonstration each,
//! with what to try on its own list. People sign in as ana, bia or caio, with [`seed::PASSWORD`].
//!
//! It is also how to know it all still works: `tests/tour.rs` goes through every demonstration, as
//! each of the three, in memory and in SQLite.

use ravenrs::prelude::{AnyView, ThemeEditor};
use ravenrs_app::{
    Access, Application, AuditMode, AuditSettings, DashboardInfo, Diff, EntityInfo, MemoryDatabase,
    ModelLayer, ObjectSpace, Operation, PermissionPolicy, Role, audit_entity, register_function,
    security_entities,
};
use ravenrs_app_library::LibraryModule;
use ravenrs_app_report::{
    AnalyticsModule, ReportModule, ReportWords, StoredReportsModule, analytics_entities, report_entity,
};
use ravenrs_app_ui::{App, CloneModule, Language, SecuredApp, Session, Words};

/// The demonstration of themes, which is the token editor itself.
const THEME_EDITOR: &str = "ThemeEditor";

pub mod actions;
pub mod model;
pub mod seed;

fn library() -> LibraryModule {
    LibraryModule::new().navigation("Registros prontos").person().event().country().address()
}

/// A function of the application, for a filter on a list to use. Expressions are checked when the
/// model is put together, so it comes before that.
fn register_is_even() {
    if !ravenrs_app::environment().has_function("IsEven") {
        register_function("IsEven", |values| match values.first() {
            Some(ravenrs_app::Value::Number(number)) => Ok(ravenrs_app::Value::Bool(number % 2.0 == 0.0)),
            _ => Ok(ravenrs_app::Value::Bool(false)),
        });
    }
}

/// The entities storage has to know: the center's, the modules', and the fields the model adds.
pub fn storage_entities() -> Result<Vec<EntityInfo>, String> {
    register_is_even();
    let mut builder = Application::builder("Centro").module(library()).module(actions::CenterModule);
    for entity in model::entities() {
        builder = builder.entity(entity);
    }
    let mut application = builder.build().map_err(|e| e.0)?;
    let difference = Diff::parse(model::CUSTOM_FIELDS).map_err(|e| e.0)?;
    application.layers_mut().put(&ModelLayer::Administrator, difference);
    application.refresh_model().map_err(|e| e.0)?;
    let mut entities = application.entities().to_vec();
    entities.extend(security_entities());
    entities.push(audit_entity());
    entities.push(report_entity());
    entities.extend(analytics_entities());
    Ok(entities)
}

/// Who can do what. Administrators come with every installation.
fn roles() -> Vec<Role> {
    let document = "SecuredDocument";
    vec![
        Role::new("Administradores").administrative(),
        Role::new("Vendas")
            .policy(PermissionPolicy::AllowAll)
            .member_permission(document, ["cost"], None, [Operation::Read, Operation::Write], Access::Deny)
            .object_permission(
                document,
                "[owner] <> ?CurrentUserName",
                [Operation::Write, Operation::Delete],
                Access::Deny,
            ),
        Role::new("Leitura")
            .policy(PermissionPolicy::ReadOnlyAll)
            .object_permission(document, "[owner] = 'ana'", [Operation::Read], Access::Deny)
            .navigation_permission("Auditoria", Access::Deny)
            .action_permission("DemoSimple", Access::Deny),
    ]
}

fn audit() -> AuditSettings {
    let mut settings = AuditSettings::new(AuditMode::Off)
        .entity("AuditFull", AuditMode::Full)
        .entity("AuditLight", AuditMode::Lightweight)
        .entity("AuditCreation", AuditMode::CreationOnly)
        .entity("AuditFields", AuditMode::Full)
        .without_field("AuditFields", "ignored")
        .entity("AuditOwner", AuditMode::Full)
        .entity("AuditPart", AuditMode::Full);
    for entity in security_entities() {
        settings = settings.entity(entity.name, AuditMode::Full);
    }
    settings
}

/// The application each person gets once they signed in.
fn application() -> App {
    let words = ReportWords::pt_br;
    let mut app = App::new("Raven Centro de Recursos")
        .language(Language::BrazilianPortuguese)
        .module(library())
        .module(actions::CenterModule)
        .module(CloneModule::with_words(Words::pt_br()))
        .module(ReportModule::new("Relatório da lista"))
        .module(StoredReportsModule::new(words()))
        .module(AnalyticsModule::new(words()))
        .administrator_settings(model::CUSTOM_FIELDS)
        .translation("pt-BR", ravenrs_app_library::PT_BR)
        .translation("pt-BR", ravenrs_app_report::PT_BR);
    for entity in model::entities() {
        app = app.entity_info(entity);
    }
    app.dashboard(
        DashboardInfo::new(THEME_EDITOR, "Editor de tokens")
            .navigation("Temas")
            .icon("palette")
            .control("Editor de tokens", THEME_EDITOR),
    )
    .dashboard(
        DashboardInfo::new(model::COUNTER_WINDOW, "Caixa de exemplo")
            .navigation("Balcão")
            .icon("shopping-cart")
            .control("Caixa de exemplo", model::COUNTER_WINDOW),
    )
}

/// The whole center over what `space` makes.
pub fn center<S: ObjectSpace + 'static>(space: impl Fn() -> S + 'static) -> SecuredApp {
    watched(space, |_| {})
}

/// The same, telling `seen` the session of each person who signs in, for what drives it from outside.
pub fn watched<S: ObjectSpace + 'static>(
    space: impl Fn() -> S + 'static,
    seen: impl Fn(&Session) + 'static,
) -> SecuredApp {
    let seen = std::rc::Rc::new(seen);
    register_is_even();
    let mut secured = SecuredApp::new("Raven Centro de Recursos", space, move |_person| {
        let seen = seen.clone();
        application().with_session(move |session| {
            session.register_control(THEME_EDITOR, || Box::new(ThemeEditor::new()) as AnyView);
            session.register_control(model::COUNTER_WINDOW, actions::counter_control());
            seen(session);
        })
    })
    .language(Language::BrazilianPortuguese)
    .audit(audit())
    .seed(seed::seed);
    for role in roles() {
        secured = secured.role(role);
    }
    secured
}

/// In memory, as in the browser: it starts over each time it opens.
pub fn in_memory() -> Result<SecuredApp, String> {
    let database = memory_database()?;
    Ok(center(move || database.space()))
}

/// An empty database in memory that knows it all.
pub fn memory_database() -> Result<MemoryDatabase, String> {
    MemoryDatabase::new(storage_entities()?).map_err(|e| e.0)
}

/// In SQLite: what is done here is there the next time it opens.
#[cfg(not(target_arch = "wasm32"))]
pub fn in_sqlite(file: &std::path::Path) -> Result<SecuredApp, String> {
    let database = sqlite_database(file)?;
    Ok(center(move || database.space()))
}

/// A database in this file that knows it all.
#[cfg(not(target_arch = "wasm32"))]
pub fn sqlite_database(file: &std::path::Path) -> Result<ravenrs_app_sqlite::SqliteDatabase, String> {
    let connection = rusqlite::Connection::open(file).map_err(|e| e.to_string())?;
    let mut builder = ravenrs_app_sqlite::SqliteDatabase::builder(std::rc::Rc::new(connection));
    for entity in storage_entities()? {
        let table = ravenrs_app_sqlite::TableMap::new(entity.name.to_lowercase());
        builder = builder.managed(entity, table).map_err(|e| e.0)?;
    }
    Ok(builder.build())
}

/// The counter window on its own, with a look of its own: `cargo run -p feature-center -- caixa`.
pub fn counter_window() -> AnyView {
    actions::counter_app().theme(ravenrs::prelude::Theme::raven_light()).into_view()
}

/// The window: in a file on the desktop, set by `RAVEN_CENTRO`, and in memory in the browser.
pub fn app() -> AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    let made = {
        let file = std::env::var_os("RAVEN_CENTRO")
            .map_or_else(|| std::env::temp_dir().join("raven-centro-de-recursos.db"), Into::into);
        in_sqlite(&file)
    };
    #[cfg(target_arch = "wasm32")]
    let made = in_memory();
    match made {
        Ok(center) => center.into_view(),
        Err(why) => Box::new(ravenrs::prelude::Alert::new("Raven Centro de Recursos", why)),
    }
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match ravenrs::web::start(super::app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => std::mem::forget(app),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
