//! The summary of what is overdue and what falls due, as each person may see it (#860).
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{
    Access, DigestPreference, DigestWords, DueDigest, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryDatabase, MemorySettings, Object, ObjectSpace, Operation, PermissionPolicy, Permissions, Role,
    RoleMerge, SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory, Value, digest_of,
    digest_preference, due_digest_for, security_entities, set_digest_preference,
};
use ravenrs_criteria::DateTime;

fn day(day: u32) -> DateTime {
    DateTime::from_ymd(2026, 11, day).expect("a date")
}

fn invoice() -> EntityInfo {
    EntityInfo::new("Invoice")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("description", FieldKind::Text))
        .field(FieldInfo::new("due_on", FieldKind::Date))
        .field(FieldInfo::new("amount", FieldKind::Money))
        .field(FieldInfo::new("status", FieldKind::Text))
        .due_digest(
            DueDigest::new("due_on")
                .pending("[status] = 'open'")
                .label("description")
                .amount("amount")
                .words("conta", "contas")
                .named(3)
                .hidden_amounts("Os valores ficam com a Tesouraria."),
        )
}

fn record(id: f64, name: &str, due: u32, cents: f64, status: &str) -> Object {
    let mut object = invoice().new_object();
    object.set("id", id);
    object.set("description", name);
    object.set("due_on", Value::Date(day(due)));
    object.set("amount", cents);
    object.set("status", status);
    object
}

fn pt() -> DigestWords {
    DigestWords::portuguese()
}

#[test]
fn one_overdue_names_it_with_the_total() {
    let records = [record(1.0, "Luz", 20, 48_000.0, "open"), record(2.0, "Água", 25, 9_000.0, "open")];
    let digest = digest_of(&invoice(), &records, day(21), 3, true, &pt()).expect("a digest");
    assert_eq!(digest.title, "1 conta vencida");
    assert_eq!(digest.body, "Luz (R$ 480,00). Total: R$ 480,00.");
    assert_eq!(digest.keys, vec![Value::Number(1.0)]);
}

#[test]
fn seven_days_ahead_takes_the_coming_one_in_order() {
    let records = [record(2.0, "Água", 25, 9_000.0, "open"), record(1.0, "Luz", 20, 48_000.0, "open")];
    let digest = digest_of(&invoice(), &records, day(21), 7, true, &pt()).expect("a digest");
    assert_eq!(digest.title, "1 conta vencida e 1 a vencer");
    assert_eq!(digest.keys, vec![Value::Number(1.0), Value::Number(2.0)]);
    assert_eq!((digest.overdue, digest.coming), (1, 1));
    assert!(digest.body.ends_with("Total: R$ 570,00."), "{}", digest.body);
}

#[test]
fn five_named_three_and_more_two_and_nothing_pending_is_nothing() {
    let records: Vec<Object> =
        (1..=5).map(|n| record(f64::from(n), &format!("Conta {n}"), 15 + n, 1_000.0, "open")).collect();
    let digest = digest_of(&invoice(), &records, day(18), 7, true, &pt()).expect("a digest");
    assert_eq!(digest.title, "2 contas vencidas e 3 a vencer");
    assert!(
        digest.body.starts_with("Conta 1 (R$ 10,00), Conta 2 (R$ 10,00), Conta 3 (R$ 10,00), mais 2."),
        "{}",
        digest.body
    );
    let paid = [record(1.0, "Luz", 20, 48_000.0, "paid"), record(2.0, "Gás", 21, 1_000.0, "cancelled")];
    assert_eq!(digest_of(&invoice(), &paid, day(21), 7, true, &pt()), None);
    assert_eq!(digest_of(&invoice(), &[], day(21), 7, true, &pt()), None);
}

#[test]
fn without_amounts_no_money_shows_and_an_empty_amount_takes_the_total_out() {
    let records = [record(1.0, "Luz", 20, 48_000.0, "open"), record(2.0, "Água", 22, 9_000.0, "open")];
    let hidden = digest_of(&invoice(), &records, day(21), 3, false, &pt()).expect("a digest");
    assert_eq!(hidden.title, "1 conta vencida e 1 a vencer");
    assert!(!hidden.title.contains("R$") && !hidden.body.contains("R$"), "{}", hidden.body);
    assert_eq!(hidden.body, "Luz, Água. Os valores ficam com a Tesouraria.");
    let mut emptied = records.clone();
    emptied[1].set("amount", Value::Null);
    let partial = digest_of(&invoice(), &emptied, day(21), 3, true, &pt()).expect("a digest");
    assert_eq!(partial.body, "Luz (R$ 480,00), Água.");
}

#[test]
fn a_field_that_is_not_there_or_not_money_is_refused() {
    let missing = EntityInfo::new("Invoice")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .due_digest(DueDigest::new("nao_existe"));
    let error = missing.check().expect_err("refused");
    assert!(error.0.contains("nao_existe"), "{error:?}");
    let not_money = invoice().due_digest(DueDigest::new("due_on").amount("description"));
    assert!(not_money.check().is_err());
}

struct World {
    database: MemoryDatabase,
    directory: UserDirectory,
}

fn world() -> World {
    let mut entities = vec![invoice()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).expect("entities fit");
    let raw = database.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    let mut space = database.space();
    for object in [
        record(1.0, "Luz", 20, 48_000.0, "open"),
        record(2.0, "Água", 25, 9_000.0, "open"),
        record(3.0, "Gás", 19, 7_000.0, "paid"),
    ] {
        space.insert(object).expect("kept");
    }
    space.commit().expect("kept");
    World { database, directory }
}

fn digest_as(world: &World, role: Role, preference: DigestPreference) -> Option<ravenrs_app::Digest> {
    let mut entities = vec![invoice()];
    entities.extend(security_entities());
    let context = Rc::new(SecurityContext::new(
        Identity::new("bia", "bia", vec![role.clone()]),
        world.directory.clone(),
        entities.clone(),
        RoleMerge::AnyRole,
    ));
    let space = SecuredObjectSpace::new(Box::new(world.database.space()), context);
    let permissions = Permissions::new(Identity::new("bia", "bia", vec![role]), entities, RoleMerge::AnyRole);
    due_digest_for(&space, &invoice(), &permissions, day(21), preference, &pt())
}

#[test]
fn the_digest_follows_the_entity_and_the_field_permission() {
    let world = world();
    let reader = Role::new("Tesouraria").policy(PermissionPolicy::ReadOnlyAll);
    let full = digest_as(&world, reader, DigestPreference::default()).expect("a digest");
    assert_eq!(full.body, "Luz (R$ 480,00). Total: R$ 480,00.");

    let no_amount = Role::new("Secretaria").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Invoice",
        ["amount"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    let hidden = digest_as(&world, no_amount, DigestPreference::default()).expect("a digest");
    assert_eq!(hidden.title, "1 conta vencida");
    assert!(!hidden.body.contains("R$"), "{}", hidden.body);

    let nothing = Role::new("Visitante").policy(PermissionPolicy::DenyAll);
    assert_eq!(digest_as(&world, nothing, DigestPreference::default()), None);

    let reader = Role::new("Tesouraria").policy(PermissionPolicy::ReadOnlyAll);
    assert_eq!(digest_as(&world, reader.clone(), DigestPreference::new(false, 3)), None, "turned off");
    let ahead = digest_as(&world, reader, DigestPreference::new(true, 7)).expect("a digest");
    assert_eq!(ahead.title, "1 conta vencida e 1 a vencer");
}

#[test]
fn each_person_keeps_only_their_own_preference() {
    let store = MemorySettings::default();
    assert_eq!(digest_preference(&store, "ana", "Invoice"), DigestPreference::default());
    set_digest_preference(&store, "ana", "ana", "Invoice", DigestPreference::new(true, 7)).expect("kept");
    let refused = set_digest_preference(&store, "bia", "ana", "Invoice", DigestPreference::new(false, 0));
    assert!(refused.is_err(), "someone changed another person's preference");
    assert_eq!(digest_preference(&store, "ana", "Invoice"), DigestPreference::new(true, 7));
    assert!(set_digest_preference(&store, "ana", "ana", "Invoice", DigestPreference::new(true, 5)).is_err());
}
