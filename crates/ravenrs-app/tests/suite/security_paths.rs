//! Permissions that read through references, such as "a teacher records the attendance of their
//! own class": they hold for a record not yet kept, and nothing put on the record stands in for
//! what storage says.
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{
    Access, EntityInfo, FieldInfo, FieldKind, Identity, MemoryDatabase, Object, ObjectSpace, Operation,
    PermissionPolicy, Query, Role, RoleMerge, SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory,
    Value, security_entities,
};

fn teacher() -> EntityInfo {
    EntityInfo::new("Teacher")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("login", FieldKind::Text))
}

fn class() -> EntityInfo {
    EntityInfo::new("Class")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("teacher_id", FieldKind::Reference("Teacher".into())))
}

fn attendance() -> EntityInfo {
    EntityInfo::new("Attendance")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("class_id", FieldKind::Reference("Class".into())))
        .field(FieldInfo::new("present", FieldKind::Integer))
}

const OWN: &str = "[class_id.teacher_id.login] = ?CurrentUserName";

/// Ana teaches class 1 and Bia class 2. A teacher reads classes, and reads, records and changes
/// only the attendance of their own; teachers themselves they cannot read at all.
struct School {
    database: MemoryDatabase,
    context: Rc<SecurityContext>,
}

impl School {
    fn new(login: &str) -> Self {
        let mut entities = vec![teacher(), class(), attendance()];
        entities.extend(security_entities());
        let database = MemoryDatabase::new(entities.clone()).expect("entities fit");
        let mut space = database.space();
        for (id, login) in [(1, "ana"), (2, "bia")] {
            let mut record = space.new_object("Teacher").expect("teacher");
            record.set("id", f64::from(id));
            record.set("login", login);
            space.save(record).expect("saved");
            let mut record = space.new_object("Class").expect("class");
            record.set("id", f64::from(id));
            record.set("name", format!("Class of {login}"));
            record.set("teacher_id", f64::from(id));
            space.save(record).expect("saved");
            let mut record = space.new_object("Attendance").expect("attendance");
            record.set("id", f64::from(id));
            record.set("class_id", f64::from(id));
            record.set("present", 10.0);
            space.save(record).expect("saved");
        }
        space.commit().expect("committed");
        let role = Role::new("Teacher")
            .policy(PermissionPolicy::DenyAll)
            .type_permission("Class", [Operation::Read], Access::Allow)
            .object_permission(
                "Attendance",
                OWN,
                [Operation::Read, Operation::Create, Operation::Write],
                Access::Allow,
            );
        let raw = database.clone();
        let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
        let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
        let context = Rc::new(SecurityContext::new(
            Identity::new(login, login, vec![role]),
            directory,
            entities,
            RoleMerge::AnyRole,
        ));
        Self { database, context }
    }

    fn space(&self) -> SecuredObjectSpace {
        SecuredObjectSpace::new(Box::new(self.database.space()), self.context.clone())
    }

    fn call(&self, space: &mut SecuredObjectSpace, class: i32) -> Object {
        let mut record = space.new_object("Attendance").expect("they may create some");
        record.set("id", 100.0 + f64::from(class));
        record.set("class_id", f64::from(class));
        record.set("present", 12.0);
        record
    }

    fn stored_attendance(&self) -> usize {
        self.database.space().query("Attendance", &Query::all()).expect("reads").len()
    }
}

#[test]
fn a_teacher_records_the_attendance_of_their_own_class() {
    let school = School::new("ana");
    let mut space = school.space();
    let record = school.call(&mut space, 1);
    space.save(record).expect("their own class");
    space.commit().expect("kept");
    assert_eq!(school.stored_attendance(), 3);
}

#[test]
fn and_not_of_another_class() {
    let school = School::new("ana");
    let mut space = school.space();
    let record = school.call(&mut space, 2);
    assert!(space.save(record).is_err(), "Bia's class");
    assert!(space.commit().is_ok());
    assert_eq!(school.stored_attendance(), 2);
}

#[test]
fn what_is_put_on_the_record_does_not_stand_in_for_storage() {
    let school = School::new("ana");
    let mut space = school.space();
    let mut record = school.call(&mut space, 2);
    record.set("class_id.teacher_id.login", "ana");
    assert!(space.save(record).is_err(), "a forged path is replaced by what storage says");
    // Nor does it get kept, where something else might read it.
    let mut record = school.call(&mut space, 1);
    record.set("class_id.teacher_id.login", "bia");
    space.save(record).expect("her own class, whatever the record says");
    space.commit().expect("kept");
    let kept =
        school.database.space().find("Attendance", &Value::Number(101.0)).expect("reads").expect("there");
    assert!(!kept.values().keys().any(|name| name.contains('.')), "{:?}", kept.values());
    let mut record = school.call(&mut space, 99);
    record.set("class_id.teacher_id.login", "ana");
    assert!(space.save(record).is_err(), "a class that does not exist leads nowhere");
}

#[test]
fn moving_their_record_to_another_class_is_refused() {
    let school = School::new("ana");
    let mut space = school.space();
    let mut own = space.find("Attendance", &Value::Number(1.0)).expect("reads").expect("hers");
    own.set("present", 11.0);
    space.save(own.clone()).expect("a change that stays in her class");
    own.set("class_id", 2.0);
    assert!(space.save(own).is_err(), "it would land in Bia's class");
}

#[test]
fn each_reads_only_the_attendance_of_their_classes() {
    for (login, mine) in [("ana", 1.0), ("bia", 2.0)] {
        let school = School::new(login);
        let found = school.space().query("Attendance", &Query::all()).expect("reads");
        let classes: Vec<Value> = found.iter().map(|record| record.get("class_id")).collect();
        assert_eq!(classes, [Value::Number(mine)], "{login}");
        assert!(school.space().find("Attendance", &Value::Number(3.0 - mine)).expect("reads").is_none());
        // Teachers they cannot read, though the rule goes through them.
        assert!(school.space().query("Teacher", &Query::all()).is_err_or_empty());
    }
}

trait ErrOrEmpty {
    fn is_err_or_empty(&self) -> bool;
}

impl ErrOrEmpty for Result<Vec<Object>, ravenrs_app::SpaceError> {
    fn is_err_or_empty(&self) -> bool {
        self.as_ref().map_or(true, Vec::is_empty)
    }
}
