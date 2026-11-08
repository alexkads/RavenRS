//! Source read while the program runs gives the entities the derive gives.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ravenrs_app::{BusinessEnum, BusinessObject, FieldKind, entities_from_source};

/// The records of `examples/quick-app`, plus rules, appearance and choices.
const SOURCE: &str = r#"
use ravenrs_app::{BusinessEnum, BusinessObject};

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
#[object(caption = "Contas", navigation = "Cadastros", icon = "wallet", form_columns = 2, focus_order = "name, kind")]
#[rule(criteria = "[opening] >= 0", message = "Sem saldo negativo", warning)]
#[appearance(names = "name", criteria = "[kind] = 'Savings'", font = "success", bold, context = "list")]
#[variant(id = "savings", caption = "Poupanças", filter = "[kind] = 'Savings'", columns = "name, opening")]
#[list_filter(id = "big", caption = "Grandes", filter = "[opening] > 1000")]
#[action(caption = "Zerar", set = "opening = 0; notes = 'zerada'", enabled = "[opening] > 0", selection = "one", confirm = "Zerar a conta?")]
struct Account {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property, required, unique)]
    name: String,
    #[field(caption = "Tipo", default = AccountKind::Savings)]
    kind: AccountKind,
    #[field(caption = "Banco", reference = "Bank")]
    bank: Option<String>,
    #[field(caption = "Saldo inicial", money, min = 0, max = 1_000_000, default = 100)]
    opening: i64,
    #[field(caption = "Observações", long, hint = "Opcional")]
    notes: Option<String>,
}
"#;

#[derive(BusinessEnum)]
enum AccountKind {
    #[option(caption = "Corrente")]
    Checking,
    #[option(caption = "Poupança")]
    Savings,
}

#[derive(BusinessObject)]
#[object(caption = "Bancos", navigation = "Cadastros", icon = "folder")]
#[allow(dead_code)]
struct Bank {
    #[field(typed_key, caption = "Código")]
    code: String,
    #[field(caption = "Nome", default_property)]
    name: String,
}

#[derive(BusinessObject)]
#[object(
    caption = "Contas",
    navigation = "Cadastros",
    icon = "wallet",
    form_columns = 2,
    focus_order = "name, kind"
)]
#[rule(criteria = "[opening] >= 0", message = "Sem saldo negativo", warning)]
#[appearance(names = "name", criteria = "[kind] = 'Savings'", font = "success", bold, context = "list")]
#[variant(id = "savings", caption = "Poupanças", filter = "[kind] = 'Savings'", columns = "name, opening")]
#[list_filter(id = "big", caption = "Grandes", filter = "[opening] > 1000")]
#[action(
    caption = "Zerar",
    set = "opening = 0; notes = 'zerada'",
    enabled = "[opening] > 0",
    selection = "one",
    confirm = "Zerar a conta?"
)]
#[allow(dead_code)]
struct Account {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property, required, unique)]
    name: String,
    #[field(caption = "Tipo", default = "Savings")]
    kind: AccountKind,
    #[field(caption = "Banco", reference = "Bank")]
    bank: Option<String>,
    #[field(caption = "Saldo inicial", money, min = 0, max = 1_000_000, default = 100)]
    opening: i64,
    #[field(caption = "Observações", long, hint = "Opcional")]
    notes: Option<String>,
}

#[test]
fn source_gives_what_the_derive_gives() {
    let read = entities_from_source(SOURCE).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(read.len(), 2);
    assert_eq!(read[0], Bank::entity_info());
    assert_eq!(read[1], Account::entity_info());
    assert!(matches!(read[1].field_named("kind").map(|f| &f.kind), Some(FieldKind::Enum(o)) if o.len() == 2));
    assert_eq!(read[1].actions.len(), 1, "the action comes from both");
    assert_eq!(read[1].actions[0].set, "opening = 0; notes = 'zerada'");
}

#[test]
fn the_derive_s_words_come_with_their_place() {
    let errors = entities_from_source(
        "#[derive(BusinessObject)]\nstruct A {\n    #[field(colour)]\n    id: i64,\n}\n",
    )
    .unwrap_err();
    assert_eq!((errors[0].line, errors[0].column), (3, 13), "{errors:?}");
    assert!(errors[0].message.starts_with("unknown setting: use key"), "{}", errors[0].message);

    let errors =
        entities_from_source("#[derive(BusinessObject)]\nstruct A {\n    id: i64,\n}\n").unwrap_err();
    assert!(errors[0].message.contains("needs a key"), "{}", errors[0].message);

    let errors = entities_from_source(
        "#[derive(BusinessObject)]\n#[rule(criteria = \"[nope] > 1\")]\nstruct A {\n    #[field(key)]\n    id: i64,\n}\n",
    )
    .unwrap_err();
    assert_eq!(errors[0].message, "this struct has no field `nope`");
    assert_eq!(errors[0].line, 2);

    let errors = entities_from_source(
        "#[derive(BusinessObject)]\n#[action(caption = \"Go\", set = \"id = [nope]\")]\nstruct A {\n    #[field(key)]\n    id: i64,\n}\n",
    )
    .unwrap_err();
    assert_eq!(errors[0].message, "this struct has no field `nope`");
    assert_eq!(errors[0].line, 2);
}

#[test]
fn what_only_a_compiler_knows_is_refused_with_its_place() {
    let errors = entities_from_source(
        "#[derive(BusinessObject)]\nstruct A {\n    #[field(key)]\n    id: i64,\n    when: Vec<u8>,\n    #[field(default = now())]\n    n: i64,\n}\n",
    )
    .unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(errors[0].line, 5, "an unknown type");
    assert_eq!(errors[1].line, 6, "a default that calls a function");
}

#[test]
fn broken_rust_says_where() {
    let errors = entities_from_source("struct A {\n    id: i64,\n").unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].line >= 1 && !errors[0].message.is_empty(), "{errors:?}");
}
