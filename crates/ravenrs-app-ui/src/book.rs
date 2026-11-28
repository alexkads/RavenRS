//! The stories of the window of an application made with `ravenrs-app`, for RavenComponentBook
//! (feature `book`): see `ravenrs_widgets::book`.

use ravenrs_app::{
    Application, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace, Rule,
};
use ravenrs_widgets::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets::{AnyView, Labels, Localized, Text};

use crate::{AppShell, EditorRegistry, EntityForm, FormText, SystemModule, Words};

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
    let section = args.text("section");
    if !section.is_empty() {
        shell = shell.sections(std::iter::once(("Cadastros".to_owned(), section)).collect());
    }
    shell = shell
        .compact_navigation(args.bool("compact"))
        .navigation_search(args.bool("search"))
        .navigation_collapse(args.bool("collapse"));
    Box::new(Localized::new(Labels::pt_br(), shell))
}

impl ComponentBook for AppShell {
    fn book() -> Meta {
        Meta::new("Application/AppShell", render)
            .summary("The window of an application made from its model: navigation by module, lists, forms and the theme menu.")
            .arg(ArgType::text("name", "Raven Contas").description("The application's name, at the top of the navigation."))
            .arg(ArgType::text("subtitle", "").description("A line under the name; empty, none."))
            .arg(ArgType::bool("theme_menu", true).description("The theme menu at the foot of the navigation."))
            .arg(ArgType::text("section", "").description(
                "The section that gathers the group Cadastros, which then is one item with its lists in a submenu; empty, none.",
            ))
            .story("Lists", Args::new())
            .story("With subtitle", Args::new().with("subtitle", "Finanças da família"))
            .story("No theme menu", Args::new().with("theme_menu", false))
            .arg(ArgType::bool("compact", false).description("Denser items, 32 high with callout text, as in Raven Finanças."))
            .arg(ArgType::bool("search", false).description("A first item, Procurar comando, that opens the command palette (⌘K)."))
            .arg(ArgType::bool("collapse", false).description("An item at the foot, Recolher menu, that folds the menu to its icons (⌘B)."))
            .story("In sections", Args::new().with("section", "Finanças"))
            .story("Compact", Args::new().with("compact", true))
            .story(
                "Compact, with search and collapse",
                Args::new().with("compact", true).with("search", true).with("collapse", true).with("section", "Finanças"),
            )
    }
}

/// The entities of a transaction as Raven Finanças has them: a kind on a segmented control, a
/// description across the form, an amount, a date and three references.
fn ledger() -> Vec<EntityInfo> {
    let named = |entity: &str, caption: &str| {
        EntityInfo::new(entity)
            .caption(caption)
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
    };
    let reference = |name: &str, target: &str, caption: &str| {
        FieldInfo::new(name, FieldKind::Reference(target.into())).caption(caption).editor("Select")
    };
    let transaction = EntityInfo::new("Transaction")
        .caption("Lançamentos")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(
            FieldInfo::new(
                "flow",
                FieldKind::Enum(vec![
                    EnumOption::new("expense", "Despesa"),
                    EnumOption::new("income", "Receita"),
                ]),
            )
            .caption("Tipo")
            .editor("Segmented")
            .default_value("expense")
            .wide(),
        )
        .field(
            FieldInfo::new("description", FieldKind::Text)
                .caption("Descrição")
                .placeholder("Ex.: Supermercado")
                .autofocus()
                .wide(),
        )
        .field(FieldInfo::new("amount", FieldKind::Money).caption("Valor").placeholder("0,00"))
        .field(FieldInfo::new("date", FieldKind::Date).caption("Data"))
        .field(reference("account", "Account", "Conta"))
        .field(reference("category", "Category", "Categoria"))
        .field(reference("member", "Member", "De quem").none_caption("Família toda"))
        .rule(Rule::required("description"))
        .rule(Rule::required("amount"))
        .rule(Rule::required("date"));
    vec![transaction, named("Account", "Contas"), named("Category", "Categorias"), named("Member", "Família")]
}

/// Every kind of field a model has, each with the editor the registry gives it.
fn every_kind() -> Vec<EntityInfo> {
    let field = |name: &str, kind: FieldKind, caption: &str| FieldInfo::new(name, kind).caption(caption);
    vec![
        EntityInfo::new("Sample")
            .caption("Campos")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(field("text", FieldKind::Text, "Text").placeholder("A line"))
            .field(field("number", FieldKind::Integer, "Integer"))
            .field(field("decimal", FieldKind::Decimal(2), "Decimal (2)"))
            .field(field("money", FieldKind::Money, "Money").placeholder("0,00"))
            .field(field("date", FieldKind::Date, "Date"))
            .field(field("moment", FieldKind::DateTime, "Date and time"))
            .field(
                field(
                    "choice",
                    FieldKind::Enum(vec![EnumOption::new("a", "First"), EnumOption::new("b", "Second")]),
                    "Enum",
                )
                .default_value("a"),
            )
            .field(
                field(
                    "segmented",
                    FieldKind::Enum(vec![EnumOption::new("a", "First"), EnumOption::new("b", "Second")]),
                    "Enum, segmented",
                )
                .editor("Segmented")
                .default_value("a"),
            )
            .field(field("color", FieldKind::Color, "Color"))
            .field(field("active", FieldKind::Boolean, "Boolean").default_value(true))
            .field(field("notes", FieldKind::LongText, "Long text").wide()),
    ]
}

/// A form of the first entity of `entities`, opened on a new record, with what the story asks.
fn form(entities: Vec<EntityInfo>, args: &Args) -> AnyView {
    let entity = entities.first().map(|e| e.name.clone()).unwrap_or_default();
    let Ok(database) = MemoryDatabase::new(entities.clone()) else {
        return Box::new(Text::new("The entities are declared right above."));
    };
    let mut setup = database.space();
    for (target, names) in [
        ("Account", ["Conta corrente", "Poupança", "Carteira"]),
        ("Category", ["Mercado", "Moradia", "Transporte"]),
        ("Member", ["Ana", "Bruno", "Lia"]),
    ] {
        for name in names {
            if let Ok(mut object) = setup.new_object(target) {
                object.set("name", name);
                let _ = setup.save(object);
            }
        }
    }
    let _ = setup.commit();
    let Ok(form) = EntityForm::new(
        &entity,
        entities,
        move || database.space(),
        Words::pt_br(),
        EditorRegistry::default(),
    ) else {
        return Box::new(Text::new("The form could not be made."));
    };
    let form = form.width(args.number("width") as f32);
    let mut text = FormText::new(args.text("title"), args.text("save"));
    if !args.text("description").is_empty() {
        text = text.description(args.text("description"));
    }
    form.start_new(text, |_| {});
    if args.bool("show_errors") {
        // Saving with nothing filled in shows what the rules ask.
        form.save();
    }
    Box::new(Localized::new(Labels::pt_br(), form))
}

impl ComponentBook for EntityForm {
    fn book() -> Meta {
        Meta::new("Application/EntityForm", |args: &Args| form(ledger(), args))
            .summary("A form made from an entity of the model, in a window: each field with the editor its kind asks for, the rules checked on save.")
            .arg(ArgType::text("title", "Novo lançamento"))
            .arg(ArgType::text("save", "Salvar lançamento").description("The button that keeps the record; it carries a check."))
            .arg(ArgType::text("description", "").description("A line under the title; empty, none."))
            .arg(ArgType::number("width", 440.0, 320.0, 880.0, 1.0).description("The width of the window; two columns of fields, `wide()` ones across."))
            .arg(ArgType::bool("show_errors", false).description("Saved with the fields empty, so the rules show their messages."))
            .story("New record", Args::new())
            .story("With description", Args::new().with("description", "Despesas entram no mês da data."))
            .story("Wide", Args::new().with("width", 600.0))
            .story("Rules not met", Args::new().with("show_errors", true))
            .story_with(
                "Every kind of field",
                Args::new().with("title", "Campos").with("save", "Salvar"),
                |args| form(every_kind(), args),
            )
    }
}

/// The stories of this crate, in the order the book lists them.
pub fn stories() -> Vec<Meta> {
    vec![AppShell::book(), EntityForm::book()]
}
