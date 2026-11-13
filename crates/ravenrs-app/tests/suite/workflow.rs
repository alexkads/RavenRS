//! States and the ways between them, refused where records are kept.
#![allow(clippy::expect_used)]

use ravenrs_app::{
    Action, Application, EntityInfo, EnumOption, FieldInfo, FieldKind, FrameContext, MemoryDatabase,
    ObjectSpace, Reminder, SpaceError, StateMachine, Value, check_transitions, reminders_of,
};
use std::cell::RefCell;
use std::rc::Rc;

fn machine() -> StateMachine {
    StateMachine::new("state")
        .state("draft", "Draft")
        .state("review", "In review")
        .state("done", "Approved")
        .start("draft")
        .transition("draft", "review", "Send to review")
        .transition("review", "draft", "Send back")
        .transition("review", "done", "Approve")
        .when("[cost] < 100")
}

fn task() -> EntityInfo {
    let options = vec![
        EnumOption::new("draft", "Draft"),
        EnumOption::new("review", "In review"),
        EnumOption::new("done", "Approved"),
    ];
    EntityInfo::new("Task")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("cost", FieldKind::Integer).default_value(0.0))
        .field(FieldInfo::new("state", FieldKind::Enum(options)))
        .field(FieldInfo::new("remind_at", FieldKind::DateTime))
        .state_machine(machine())
}

fn database() -> MemoryDatabase {
    let database = MemoryDatabase::new(vec![task()]).expect("valid");
    let mut space = database.space();
    let mut record = space.new_object("Task").expect("known");
    record.set("id", 1.0);
    record.set("name", "One");
    space.save(record).expect("waits");
    space.commit().expect("keeps");
    database
}

fn change(database: &MemoryDatabase, to: &str) -> Result<(), SpaceError> {
    let mut space = database.space();
    let mut record = space.find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("state", to);
    space.save(record)?;
    space.commit()
}

fn state(database: &MemoryDatabase) -> Value {
    database.space().find("Task", &Value::Number(1.0)).expect("reads").expect("there").get("state")
}

#[test]
fn a_new_record_starts_where_it_says_and_nowhere_else() {
    let database = database();
    assert_eq!(state(&database), Value::Text("draft".into()), "it starts there by itself");
    let mut space = database.space();
    let mut other = space.new_object("Task").expect("known");
    other.set("id", 2.0);
    other.set("state", "done");
    space.save(other).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))));
}

#[test]
fn only_the_ways_declared_go_through_with_no_screen_around() {
    let database = database();
    let refused = change(&database, "done");
    assert!(
        matches!(&refused, Err(SpaceError::Refused(why)) if why.contains("Draft") && why.contains("Approved"))
    );
    assert_eq!(state(&database), Value::Text("draft".into()));
    change(&database, "review").expect("a way");
    change(&database, "draft").expect("and back");
    change(&database, "review").expect("again");
    change(&database, "done").expect("it meets what the way asks");
    assert!(change(&database, "draft").is_err(), "there is no way back from there");
}

#[test]
fn a_way_only_opens_for_records_that_meet_it() {
    let database = database();
    change(&database, "review").expect("a way");
    let mut space = database.space();
    let mut record = space.find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("cost", 500.0);
    record.set("state", "done");
    space.save(record).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))));
}

#[test]
fn a_change_of_anything_else_keeps_going() {
    let database = database();
    let mut space = database.space();
    let mut record = space.find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("waits");
    space.commit().expect("the state did not change");
}

#[test]
fn two_steps_in_one_unit_of_work_count_as_one() {
    let database = database();
    let mut space = database.space();
    let mut record = space.find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("state", "review");
    space.save(record.clone()).expect("waits");
    record.set("state", "done");
    space.save(record).expect("waits");
    assert!(space.commit().is_err(), "from draft to done there is no way");
}

#[test]
fn what_is_wrong_with_a_machine_comes_out_when_it_is_declared() {
    let bad = [
        StateMachine::new("nothing").state("a", "A"),
        StateMachine::new("state"),
        StateMachine::new("state").state("a", "A").start("b"),
        StateMachine::new("state").state("a", "A").transition("a", "b", "Go"),
        StateMachine::new("state").state("a", "A").transition("a", "a", "Go").when("(("),
    ];
    for machine in bad {
        let entity = EntityInfo::new("T")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("state", FieldKind::Text))
            .state_machine(machine);
        assert!(entity.check().is_err());
    }
}

#[test]
fn a_new_record_and_a_field_on_forms_are_taken_care_of() {
    let entity = task();
    let field = entity.field_named("state").expect("there");
    assert!(field.read_only);
    assert_eq!(field.default_value, Value::Text("draft".into()));
    let record = entity.new_object();
    assert!(check_transitions(&entity, None, &record).is_ok());
}

fn frame_over(database: &MemoryDatabase) -> ravenrs_app::Frame {
    let application = Application::builder("Test").entity(task()).build().expect("valid");
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(database.space()));
    let frame = application.new_frame(FrameContext::Main, space);
    frame.set_view(Some(ravenrs_app::ViewInfo::new(
        "Task_ListView",
        ravenrs_app::ViewKind::List,
        Some("Task"),
    )));
    frame
}

#[test]
fn each_way_gets_an_action_that_opens_by_the_state_of_what_is_selected() {
    let database = database();
    let frame = frame_over(&database);
    let action = |id: &str| -> Action { frame.action(id).expect("there") };
    let record = database.space().find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    frame.set_selected(vec![record]);
    let send = action("Task.state.draft.review");
    let approve = action("Task.state.review.done");
    assert!(send.is_enabled() && send.is_active());
    assert!(!approve.is_enabled(), "not from where it stands");
    frame.execute("Task.state.draft.review", ravenrs_app::ActionInput::None).expect("runs");
    assert_eq!(state(&database), Value::Text("review".into()));
    // Selected as it is now, the action it no longer fits does not run.
    let record = database.space().find("Task", &Value::Number(1.0)).expect("reads").expect("there");
    frame.set_selected(vec![record]);
    assert!(!send.is_enabled());
    assert!(frame.execute("Task.state.draft.review", ravenrs_app::ActionInput::None).is_err());
    assert!(approve.is_enabled());
}

#[test]
fn a_reminder_goes_by_the_records_with_a_time_and_whom_it_is_for() {
    let entity = task().reminder(Reminder::new("remind_at").criteria("[name] = 'One'"));
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let mut space = database.space();
    for (id, name, at) in [(1.0, "One", true), (2.0, "Two", true), (3.0, "One", false)] {
        let mut record = space.new_object("Task").expect("known");
        record.set("id", id);
        record.set("name", name);
        if at {
            record.set("remind_at", Value::Date(ravenrs_app::local_now()));
        }
        space.save(record).expect("waits");
    }
    space.commit().expect("keeps");
    let found = reminders_of(&database.space(), &entity, &ravenrs_app::environment());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("id"), Value::Number(1.0));
    let not_a_date = EntityInfo::new("T")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("when", FieldKind::Text))
        .reminder(Reminder::new("when"));
    assert!(not_a_date.check().is_err());
}
