//! Records declared as Rust structs.
#![allow(clippy::expect_used)]

use ravenrs_app::*;

#[derive(BusinessEnum, Clone, Debug, PartialEq)]
enum Kind {
    #[option(caption = "Corrente", code = "c")]
    Checking,
    Savings,
}

#[derive(BusinessObject, Clone, Debug, PartialEq)]
#[object(caption = "Contas", navigation = "Cadastros", icon = "wallet")]
struct Account {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property)]
    name: String,
    kind: Kind,
    #[field(money, default = 100.0)]
    opening: i64,
    #[field(money, calculated = "[opening] * 2")]
    double: i64,
    #[field(long, no_list)]
    notes: Option<String>,
    #[field(reference = "Bank", caption = "Banco")]
    bank: Option<String>,
    active: bool,
    #[field(decimals = 3, read_only, no_detail)]
    rate: f64,
}

#[derive(BusinessObject, Clone, Debug, PartialEq)]
struct Bank {
    #[field(typed_key, caption = "Código")]
    code: String,
    #[field(lookup)]
    name: String,
}

fn account(name: &str) -> Account {
    Account {
        id: 0,
        name: name.into(),
        kind: Kind::Savings,
        opening: 10,
        double: 0,
        notes: None,
        bank: Some("341".into()),
        active: true,
        rate: 1.5,
    }
}

#[test]
fn the_attributes_say_what_the_entity_is() {
    let entity = Account::entity_info();
    entity.check().expect("valid");
    assert_eq!((entity.name.as_str(), entity.caption.as_str()), ("Account", "Contas"));
    assert_eq!(entity.navigation.as_deref(), Some("Cadastros"));
    assert_eq!(entity.icon.as_deref(), Some("wallet"));
    let field = |name: &str| entity.field_named(name).expect("the field").clone();
    assert!(field("id").is_key);
    assert_eq!(field("id").kind, FieldKind::Integer);
    assert_eq!(field("name").caption, "Nome");
    assert_eq!(entity.default_property().map(|f| f.name.as_str()), Some("name"));
    assert_eq!(field("opening").kind, FieldKind::Money);
    assert_eq!(field("opening").default_value, Value::Number(100.0));
    assert!(field("double").read_only && field("double").calculated.is_some());
    assert_eq!(field("notes").kind, FieldKind::LongText);
    assert!(!field("notes").in_list);
    assert_eq!(field("bank").kind, FieldKind::Reference("Bank".into()));
    assert_eq!(field("active").kind, FieldKind::Boolean);
    assert_eq!(field("rate").kind, FieldKind::Decimal(3));
    assert!(field("rate").read_only && !field("rate").in_detail);
    let FieldKind::Enum(options) = field("kind").kind else { panic!("options") };
    assert_eq!(options[0], EnumOption::new("c", "Corrente"));
    assert_eq!(options[1], EnumOption::new("Savings", "Savings"));
    assert_eq!(field("kind").default_value, Value::Text("c".into()), "a new record starts on the first");
    let bank = Bank::entity_info();
    assert!(bank.field_named("code").expect("the field").is_key);
    assert!(!bank.field_named("code").expect("the field").read_only, "typed by people");
    assert!(bank.field_named("name").expect("the field").in_lookup);
}

#[test]
fn a_struct_goes_to_a_record_and_back() {
    let first = account("Carteira");
    let object = first.to_object();
    assert_eq!(object.entity(), "Account");
    assert_eq!(object.get("kind"), Value::Text("Savings".into()));
    assert_eq!(object.get("notes"), Value::Null);
    assert_eq!(Account::from_object(&object), first);
}

#[test]
fn what_cannot_be_read_gives_the_empty_value() {
    let empty = Object::from_values("Account", Default::default());
    let read = Account::from_object(&empty);
    assert_eq!((read.id, read.name.as_str(), read.active), (0, "", false));
    assert_eq!(read.kind, Kind::Checking, "the first option");
    assert_eq!(read.bank, None);
}

#[test]
fn structs_go_through_a_space() {
    let database = MemoryDatabase::new(vec![Account::entity_info(), Bank::entity_info()]).expect("valid");
    let mut space = database.space();
    space.add(&account("A")).expect("adds");
    space.add(&account("B")).expect("adds");
    let bank = Bank { code: "341".into(), name: "Itaú".into() };
    space.add(&bank).expect("adds");
    assert!(space.add(&bank).is_err(), "a typed key is not taken over");
    space.commit().expect("commits");
    let read: Vec<Account> = space.read(&Query::all().sort("name", false)).expect("reads");
    assert_eq!(read.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["B", "A"]);
    assert_ne!(read[0].id, read[1].id, "numbered as they came");
    assert_eq!(read[0].double, 20, "calculated");
}

#[test]
fn a_struct_goes_in_an_application() {
    let application =
        Application::builder("A").entity_of::<Account>().entity_of::<Bank>().build().expect("valid");
    assert!(application.model().at(&["Views", "Account_ListView"]).is_some());
}

#[test]
fn wrong_attributes_point_at_what_is_wrong() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}

#[derive(BusinessObject, Clone, Debug, PartialEq)]
#[object(caption = "Chamadas", empty_text = "Nenhuma chamada ainda: abra uma aula para começar.")]
struct Attendance {
    #[field(key)]
    id: i64,
    #[field(default_property)]
    class: String,
}

#[test]
fn what_an_empty_list_says_comes_from_the_struct_and_reaches_the_model() {
    let entity = Attendance::entity_info();
    assert_eq!(entity.empty_text.as_deref(), Some("Nenhuma chamada ainda: abra uma aula para começar."));
    let application = Application::builder("School").entity(entity).build().expect("valid");
    let model = application.model();
    let view = model.at(&["Views", "Attendance_ListView"]).expect("the list view");
    assert_eq!(view.text("EmptyText"), Some("Nenhuma chamada ainda: abra uma aula para começar."));
    assert_eq!(Account::entity_info().empty_text, None);
}
