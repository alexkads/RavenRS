//! The stories of the window of an application made with `ravenrs-app`, for RavenComponentBook
//! (feature `book`): see `ravenrs_widgets::book`.

use ravenrs_app::{Application, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace};
use ravenrs_widgets::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets::{AnyView, Labels, Localized, Text};

use crate::{AppShell, EditorRegistry, SystemModule, Words};

/// Two lists, banks and accounts, with a few records, as the gallery's example has them.
fn render(args: &Args) -> AnyView {
    let bank = EntityInfo::new("Bank")
        .caption("Bancos")
        .navigation("Cadastros")
        .icon("landmark")
        .field(FieldInfo::new("code", FieldKind::Text).caption("Código").typed_key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property());
    let account = EntityInfo::new("Account")
        .caption("Contas")
        .navigation("Cadastros")
        .icon("wallet")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(
            FieldInfo::new(
                "kind",
                FieldKind::Enum(vec![
                    EnumOption::new("checking", "Corrente"),
                    EnumOption::new("savings", "Poupança"),
                ]),
            )
            .caption("Tipo")
            .default_value("checking"),
        )
        .field(FieldInfo::new("bank", FieldKind::Reference("Bank".into())).caption("Banco"))
        .field(FieldInfo::new("opening", FieldKind::Money).caption("Saldo inicial").default_value(0.0))
        .field(FieldInfo::new("opened", FieldKind::Date).caption("Aberta em"))
        .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativa").default_value(true))
        .field(FieldInfo::new("notes", FieldKind::LongText).caption("Observações"));
    let entities = vec![account, bank];
    let Ok(database) = MemoryDatabase::new(entities.clone()) else {
        return Box::new(Text::new("No database"));
    };
    let mut setup = database.space();
    for (code, name) in [("001", "Banco do Brasil"), ("341", "Itaú")] {
        if let Ok(mut object) = setup.new_object("Bank") {
            object.set("code", code);
            object.set("name", name);
            let _ = setup.save(object);
        }
    }
    for (name, cents) in [("Conta corrente", 125_050.0), ("Poupança", 980_000.0), ("Carteira", 12_000.0)] {
        if let Ok(mut object) = setup.new_object("Account") {
            object.set("name", name);
            object.set("opening", cents);
            object.set("bank", "341");
            let _ = setup.save(object);
        }
    }
    let _ = setup.commit();
    let builder = Application::builder(args.text("name")).module(SystemModule::with_words(Words::pt_br()));
    let Ok(application) =
        entities.into_iter().fold(builder, |builder, entity| builder.entity(entity)).build()
    else {
        return Box::new(Text::new("No application"));
    };
    let mut shell = AppShell::with_options(
        application,
        move || database.space(),
        Words::pt_br(),
        EditorRegistry::default(),
    );
    if !args.text("subtitle").is_empty() {
        shell = shell.brand_subtitle(args.text("subtitle"));
    }
    if !args.bool("theme_menu") {
        shell = shell.without_theme_menu();
    }
    Box::new(Localized::new(Labels::pt_br(), shell))
}

impl ComponentBook for AppShell {
    fn book() -> Meta {
        Meta::new("Application/AppShell", render)
            .summary("The window of an application made from its model: navigation by module, lists, forms and the theme menu.")
            .arg(ArgType::text("name", "Raven Contas").description("The application's name, at the top of the navigation."))
            .arg(ArgType::text("subtitle", "").description("A line under the name; empty, none."))
            .arg(ArgType::bool("theme_menu", true).description("The theme menu at the foot of the navigation."))
            .story("Lists", Args::new())
            .story("With subtitle", Args::new().with("subtitle", "Finanças da família"))
            .story("No theme menu", Args::new().with("theme_menu", false))
    }
}

/// The stories of this crate, in the order the book lists them.
pub fn stories() -> Vec<Meta> {
    vec![AppShell::book()]
}
