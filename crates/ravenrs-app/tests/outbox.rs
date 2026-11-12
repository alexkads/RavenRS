//! Events kept with the change that raised them, one per organization and key.
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{
    Access, Emitted, EntityInfo, Event, FieldInfo, FieldKind, Identity, MemoryDatabase, OUTBOX_EVENT,
    ObjectSpace, ObjectSpaceExt, Operation, Query, Role, RoleMerge, SecuredObjectSpace, SecurityContext,
    SpaceError, SpaceEvent, SpaceMaker, UserDirectory, Value, outbox_entity, security_entities,
};

fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("text", FieldKind::Text))
}

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![note(), outbox_entity()];
    all.extend(security_entities());
    all
}

fn database() -> MemoryDatabase {
    MemoryDatabase::new(entities()).expect("entities fit")
}

fn events(database: &MemoryDatabase) -> Vec<String> {
    database
        .space()
        .query(OUTBOX_EVENT, &Query::all())
        .expect("reads")
        .iter()
        .map(|event| match event.get("id") {
            Value::Text(id) => id,
            _ => String::new(),
        })
        .collect()
}

fn note_with(space: &mut dyn ObjectSpace, id: f64) {
    let mut record = space.new_object("Note").expect("entity");
    record.set("id", id);
    record.set("text", "written");
    space.insert(record).expect("waits");
}

#[test]
fn the_event_is_kept_with_its_change() {
    let database = database();
    let mut space = database.space();
    note_with(&mut space, 1.0);
    assert_eq!(space.emit(&Event::new("T1", "note.written", "note/1", "1")).expect("added"), Emitted::Added);
    assert_eq!(events(&database), Vec::<String>::new(), "nothing is kept before the commit");
    space.commit().expect("kept");
    assert_eq!(events(&database), ["T1/note/1"]);
    let kept = database
        .space()
        .find(OUTBOX_EVENT, &Value::Text("T1/note/1".to_owned()))
        .expect("reads")
        .expect("there");
    assert_eq!(kept.get("state"), Value::Text("pending".to_owned()));
    assert_eq!(kept.get("kind"), Value::Text("note.written".to_owned()));
}

#[test]
fn a_refused_commit_keeps_no_event() {
    let database = database();
    let mut space = database.space();
    space.on_event(Box::new(|event| match event {
        SpaceEvent::Committing(_) => Err("a rule refused the note".to_owned()),
        _ => Ok(()),
    }));
    note_with(&mut space, 1.0);
    space.emit(&Event::new("T1", "note.written", "note/1", "1")).expect("added");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))));
    space.rollback();
    assert!(events(&database).is_empty());
    assert!(database.space().find("Note", &Value::Number(1.0)).expect("reads").is_none());
}

#[test]
fn the_same_key_twice_in_one_organization_is_one_event() {
    let database = database();
    let mut space = database.space();
    let event = Event::new("T1", "note.written", "note/1", "1");
    assert_eq!(space.emit(&event).expect("added"), Emitted::Added);
    assert_eq!(
        space.emit(&event).expect("answered"),
        Emitted::AlreadyThere,
        "waiting in the same unit of work"
    );
    space.commit().expect("kept");
    let mut later = database.space();
    assert_eq!(later.emit(&event).expect("answered"), Emitted::AlreadyThere, "already kept");
    later.commit().expect("nothing to keep");
    assert_eq!(events(&database).len(), 1);
}

#[test]
fn the_same_key_in_two_organizations_is_two_events() {
    let database = database();
    let mut space = database.space();
    space.emit(&Event::new("T1", "note.written", "note/1", "1")).expect("added");
    space.emit(&Event::new("T2", "note.written", "note/1", "1")).expect("added");
    space.commit().expect("kept");
    assert_eq!(events(&database), ["T1/note/1", "T2/note/1"]);
}

#[test]
fn reading_the_events_takes_permission_on_them() {
    let database = database();
    let mut space = database.space();
    space.emit(&Event::new("T1", "note.written", "note/1", "1")).expect("added");
    space.commit().expect("kept");

    let maker: SpaceMaker = {
        let database = database.clone();
        Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
    };
    let directory = UserDirectory::new(maker, Vec::new()).expect("directory");
    let as_role = |role: Role| {
        let context = SecurityContext::new(
            Identity::new("op", "op", vec![role]),
            directory.clone(),
            entities(),
            RoleMerge::AnyRole,
        );
        SecuredObjectSpace::new(Box::new(database.space()), Rc::new(context))
    };
    let stranger = as_role(Role::new("Notes").type_permission("Note", [Operation::Read], Access::Allow));
    let seen = stranger.query(OUTBOX_EVENT, &Query::all());
    assert!(
        matches!(&seen, Ok(found) if found.is_empty()) || matches!(seen, Err(SpaceError::Denied(_))),
        "got {seen:?}"
    );
    let operator =
        as_role(Role::new("Operator").type_permission(OUTBOX_EVENT, [Operation::Read], Access::Allow));
    assert_eq!(operator.query(OUTBOX_EVENT, &Query::all()).expect("reads").len(), 1);
}
