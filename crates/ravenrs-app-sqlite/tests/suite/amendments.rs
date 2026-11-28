//! Sealed records and amendments in SQLite, as in memory (#856).
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{Amendment, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Seal, SpaceError, Value};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use rusqlite::Connection;

fn database() -> SqliteDatabase {
    built(false)
}

fn built(guarded: bool) -> SqliteDatabase {
    let lesson = EntityInfo::new("Lesson")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("status", FieldKind::Text))
        .field(FieldInfo::new("present", FieldKind::Integer))
        .sealed(Seal::when("[status] = 'closed'"));
    let correction = EntityInfo::new("LessonCorrection")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
        .field(FieldInfo::new("present", FieldKind::Integer))
        .field(FieldInfo::new("reason", FieldKind::LongText))
        .field(FieldInfo::new("previous", FieldKind::LongText))
        .amends(
            Amendment::correction("Lesson", "lesson_id")
                .copies(["present"])
                .reason("reason", 10)
                .before("previous"),
        );
    let builder = SqliteDatabase::builder(Rc::new(Connection::open_in_memory().expect("a database")))
        .managed(lesson, TableMap::new("lessons"))
        .expect("valid")
        .managed(correction, TableMap::new("lesson_corrections"))
        .expect("valid");
    if guarded { builder.sealed_guards().expect("the guards").build() } else { builder.build() }
}

#[test]
fn a_closed_lesson_changes_only_through_a_justified_correction_that_never_changes() {
    let database = database();
    let mut space = database.space();
    let mut lesson = space.new_object("Lesson").expect("known");
    let key = lesson.get("id");
    lesson.set("status", "closed");
    lesson.set("present", 3.0);
    space.insert(lesson).expect("waits");
    space.commit().expect("keeps");

    let mut direct = space.find("Lesson", &key).expect("reads").expect("there");
    direct.set("present", 9.0);
    space.save(direct).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(why)) if why.contains("closed")));
    space.rollback();

    let mut correction = space.new_object("LessonCorrection").expect("known");
    correction.set("lesson_id", key.clone());
    correction.set("present", 5.0);
    correction.set("reason", "curta");
    space.insert(correction.clone()).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(why)) if why.contains("at least 10")));
    space.rollback();

    correction.set("reason", "lista de presença errada");
    space.insert(correction).expect("waits");
    space.commit().expect("one commit corrects");
    let fresh = database.space();
    let after = fresh.find("Lesson", &key).expect("reads").expect("there");
    assert_eq!(after.integer("present"), Some(5));
    let mut kept = fresh.query("LessonCorrection", &Query::all()).expect("reads").remove(0);
    assert!(kept.text("previous").contains("\"present\":3"));

    let mut space = database.space();
    kept.set("reason", Value::Text("trocando o motivo depois".into()));
    space.save(kept).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(why)) if why.contains("does not change")));
}

#[test]
fn with_the_guards_the_database_itself_refuses_a_closed_lesson_written_directly() {
    let database = built(true);
    let mut space = database.space();
    for (id, status) in [(1.0, "closed"), (2.0, "open")] {
        let mut lesson = space.new_object("Lesson").expect("known");
        lesson.set("id", id);
        lesson.set("status", status);
        lesson.set("present", 3.0);
        space.insert(lesson).expect("waits");
    }
    space.commit().expect("keeps");
    let sql = database.backend().connection();
    // Whoever writes by other ways than the framework is refused by the database.
    let changed = sql.execute("UPDATE lessons SET present = 9 WHERE id = 1", []);
    assert!(changed.is_err_and(|e| e.to_string().contains("sealed")), "a direct update went through");
    assert!(sql.execute("DELETE FROM lessons WHERE id = 1", []).is_err(), "a direct delete went through");
    assert_eq!(sql.execute("UPDATE lessons SET present = 4 WHERE id = 2", []).expect("open"), 1);

    // The framework's own correction goes through, in one commit, and leaves no mark behind.
    let mut correction = space.new_object("LessonCorrection").expect("known");
    correction.set("lesson_id", 1.0);
    correction.set("present", 5.0);
    correction.set("reason", "lista de presença errada");
    space.insert(correction).expect("waits");
    space.commit().expect("the correction goes through the guard");
    let after = database.space().find("Lesson", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(after.integer("present"), Some(5));
    let marks: i64 =
        sql.query_row("SELECT COUNT(*) FROM raven_amending", [], |row| row.get(0)).expect("counts");
    assert_eq!(marks, 0, "the commit took its marks away");
    assert!(sql.execute("UPDATE lessons SET present = 9 WHERE id = 1", []).is_err(), "still guarded after");
}
