//! Sealed records change only by an amendment, in the same commit (#856).
#![allow(clippy::expect_used)]

use ravenrs_app::{
    Amendment, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace, Seal, SpaceError, Value,
};

fn database() -> MemoryDatabase {
    MemoryDatabase::new(vec![
        EntityInfo::new("Lesson")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("status", FieldKind::Text))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .field(FieldInfo::new("corrected", FieldKind::Boolean))
            .sealed(Seal::when("[status] = 'closed'")),
        EntityInfo::new("LessonCorrection")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .field(FieldInfo::new("previous", FieldKind::LongText))
            .amends(
                Amendment::correction("Lesson", "lesson_id")
                    .copies(["present"])
                    .reason("reason", 10)
                    .before("previous")
                    .marks("corrected"),
            ),
        EntityInfo::new("Enrollment")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("class", FieldKind::Text))
            .field(FieldInfo::new("ended", FieldKind::Boolean))
            .sealed(Seal::when("[ended] = true")),
        EntityInfo::new("Transfer")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("enrollment_id", FieldKind::Reference("Enrollment".into())))
            .field(FieldInfo::new("to_class", FieldKind::Text))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .amends(
                Amendment::closing("Enrollment", "enrollment_id").reason("reason", 10).marks("ended").apply(
                    |transfer, _origin, space| {
                        if transfer.text("to_class").is_empty() {
                            return Err("say the class it goes to".into());
                        }
                        let mut next = space.new_object("Enrollment").map_err(|e| e.to_string())?;
                        next.set("class", transfer.get("to_class"));
                        next.set("ended", false);
                        space.insert(next).map_err(|e| e.to_string())
                    },
                ),
            ),
    ])
    .expect("a valid model")
}

fn lesson(space: &mut dyn ObjectSpace, status: &str) -> Value {
    let mut lesson = space.new_object("Lesson").expect("the entity");
    lesson.set("status", status);
    lesson.set("present", 3.0);
    lesson.set("corrected", false);
    let key = lesson.get("id");
    space.insert(lesson).expect("waits");
    space.commit().expect("commits");
    key
}

fn correction(space: &mut dyn ObjectSpace, lesson: &Value, reason: &str) -> Result<(), SpaceError> {
    let mut correction = space.new_object("LessonCorrection")?;
    correction.set("lesson_id", lesson.clone());
    correction.set("present", 5.0);
    correction.set("reason", reason);
    space.insert(correction)?;
    space.commit()
}

fn refusal(result: Result<(), SpaceError>) -> String {
    match result {
        Err(SpaceError::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_closed_record_is_refused_when_changed_or_deleted_directly() {
    let db = database();
    let mut space = db.space();
    let key = lesson(&mut space, "closed");
    let mut stored = space.find("Lesson", &key).expect("reads").expect("there");
    stored.set("present", 9.0);
    space.save(stored).expect("waits");
    let why = refusal(space.commit());
    assert!(why.contains("closed: use LessonCorrection"), "{why}");
    space.rollback();
    space.delete("Lesson", &key).expect("waits");
    assert!(refusal(space.commit()).contains("closed"));
}

#[test]
fn an_open_record_is_changed_as_usual() {
    let db = database();
    let mut space = db.space();
    let key = lesson(&mut space, "open");
    let mut stored = space.find("Lesson", &key).expect("reads").expect("there");
    stored.set("status", "closed");
    space.save(stored).expect("waits");
    space.commit().expect("closing it is an ordinary change");
}

#[test]
fn a_correction_with_its_reason_changes_the_closed_record_and_keeps_what_was_before() {
    let db = database();
    let mut space = db.space();
    let key = lesson(&mut space, "closed");
    let why = refusal(correction(&mut space, &key, "nove char"));
    assert!(why.contains("at least 10"), "{why}");
    space.rollback();
    correction(&mut space, &key, "lista errada").expect("a correction with ten characters");
    let after = space.find("Lesson", &key).expect("reads").expect("there");
    assert_eq!(after.integer("present"), Some(5));
    assert_eq!(after.get("corrected"), Value::Bool(true));
    let kept = space.query("LessonCorrection", &ravenrs_app::Query::all()).expect("reads");
    let previous = kept[0].text("previous");
    assert!(previous.contains("\"present\":3"), "{previous}");
    assert!(previous.contains("\"corrected\":false"), "{previous}");
}

#[test]
fn a_correction_of_an_open_record_is_refused() {
    let db = database();
    let mut space = db.space();
    let key = lesson(&mut space, "open");
    let why = refusal(correction(&mut space, &key, "lista errada"));
    assert!(why.contains("correct it on its own form"), "{why}");
}

#[test]
fn a_kept_correction_never_changes() {
    let db = database();
    let mut space = db.space();
    let key = lesson(&mut space, "closed");
    correction(&mut space, &key, "lista errada").expect("corrects");
    let mut kept = space.query("LessonCorrection", &ravenrs_app::Query::all()).expect("reads").remove(0);
    kept.set("reason", "outro motivo qualquer");
    space.save(kept.clone()).expect("waits");
    assert!(refusal(space.commit()).contains("does not change"));
    space.rollback();
    space.delete("LessonCorrection", &kept.get("id")).expect("waits");
    assert!(refusal(space.commit()).contains("does not change"));
}

#[test]
fn a_closing_creates_where_it_goes_and_seals_the_origin_in_one_commit_and_only_once() {
    let db = database();
    let mut space = db.space();
    let mut origin = space.new_object("Enrollment").expect("the entity");
    origin.set("class", "A");
    origin.set("ended", false);
    let key = origin.get("id");
    space.insert(origin).expect("waits");
    space.commit().expect("commits");

    let transfer = |space: &mut dyn ObjectSpace, to: &str| -> Result<(), SpaceError> {
        let mut transfer = space.new_object("Transfer")?;
        transfer.set("enrollment_id", key.clone());
        transfer.set("to_class", to);
        transfer.set("reason", "mudou de turma");
        space.insert(transfer)?;
        space.commit()
    };

    // The apply fails: nothing it did, nor the closing, stays.
    assert!(matches!(transfer(&mut space, ""), Err(SpaceError::Refused(_))));
    space.rollback();
    assert_eq!(space.query("Enrollment", &ravenrs_app::Query::all()).expect("reads").len(), 1);
    assert!(space.changes().is_empty());

    transfer(&mut space, "B").expect("transfers");
    let all = space.query("Enrollment", &ravenrs_app::Query::all()).expect("reads");
    assert_eq!(all.len(), 2);
    let origin = space.find("Enrollment", &key).expect("reads").expect("there");
    assert_eq!(origin.get("ended"), Value::Bool(true));

    let why = refusal(transfer(&mut space, "C"));
    assert!(why.contains("closed already"), "{why}");
}

#[test]
fn the_model_refuses_an_amendment_that_does_not_fit_its_target() {
    let unsealed = MemoryDatabase::new(vec![
        EntityInfo::new("Lesson").field(FieldInfo::new("id", FieldKind::Integer).key()),
        EntityInfo::new("LessonCorrection")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
            .amends(Amendment::correction("Lesson", "lesson_id")),
    ]);
    assert!(unsealed.is_err_and(|e| e.0.contains("not sealed")));
    let wrong_reference = EntityInfo::new("LessonCorrection")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("lesson_id", FieldKind::Integer))
        .amends(Amendment::correction("Lesson", "lesson_id"));
    assert!(wrong_reference.check().is_err_and(|e| e.0.contains("does not refer to Lesson")));
}

#[test]
fn a_full_audit_has_the_correction_and_the_change_to_its_target_in_the_same_commit() {
    use ravenrs_app::{
        AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace, Query, SECURITY_AUDIT_ENTRY,
    };
    use std::rc::Rc;
    let mut entities = database().space().entities();
    entities.push(ravenrs_app::audit_entity());
    let db = MemoryDatabase::new(entities).expect("a valid model");
    let key = lesson(&mut db.space(), "closed");
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "ana".to_owned()));
    let mut audited = AuditedObjectSpace::new(Box::new(db.space()), trail);
    correction(&mut audited, &key, "lista errada").expect("corrects");
    let lines = db.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads");
    assert!(
        lines
            .iter()
            .any(|line| line.text("change") == "created" && line.text("target_entity") == "LessonCorrection")
    );
    let changed = lines
        .iter()
        .find(|line| line.text("target_entity") == "Lesson" && line.text("field") == "present")
        .expect("the change to the lesson is in the trail");
    assert_eq!(changed.text("old_value"), "3");
    assert_eq!(changed.text("new_value"), "5");
}

#[test]
fn a_role_that_creates_corrections_and_only_reads_the_target_corrects_and_one_that_cannot_create_is_denied() {
    use ravenrs_app::{
        Access, Identity, Operation, Role, RoleMerge, SecuredObjectSpace, SecurityContext, SpaceMaker,
        UserDirectory,
    };
    use std::rc::Rc;
    let mut entities = database().space().entities();
    entities.extend(ravenrs_app::security_entities());
    let db = MemoryDatabase::new(entities.clone()).expect("a valid model");
    let key = lesson(&mut db.space(), "closed");
    let maker: SpaceMaker = {
        let db = db.clone();
        Rc::new(move || Box::new(db.space()) as Box<dyn ObjectSpace>)
    };
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    let secured = |role: Role| {
        let identity = Identity::new("ana", "ana", vec![role]);
        let context =
            Rc::new(SecurityContext::new(identity, directory.clone(), entities.clone(), RoleMerge::AnyRole));
        SecuredObjectSpace::new(Box::new(db.space()), context)
    };
    let corrects =
        Role::new("corrects").type_permission("Lesson", [Operation::Read], Access::Allow).type_permission(
            "LessonCorrection",
            [Operation::Read, Operation::Create, Operation::Write],
            Access::Allow,
        );
    correction(&mut secured(corrects), &key, "lista errada").expect("corrects without Write on the lesson");
    let reads = Role::new("reads")
        .type_permission("Lesson", [Operation::Read], Access::Allow)
        .type_permission("LessonCorrection", [Operation::Read], Access::Allow);
    assert!(matches!(correction(&mut secured(reads), &key, "lista errada"), Err(SpaceError::Denied(_))));
}
