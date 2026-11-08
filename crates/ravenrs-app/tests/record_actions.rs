//! Actions declared as data: they set fields of the records selected, check the rules and keep
//! all or none.
#![allow(clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    ActionInput, Application, EntityInfo, FieldInfo, FieldKind, Frame, FrameContext, MemoryDatabase,
    ObjectSpace, RecordAction, Rule, Selection, Value, ViewInfo, ViewKind,
};

fn task() -> EntityInfo {
    EntityInfo::new("Task")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("state", FieldKind::Text))
        .field(FieldInfo::new("cost", FieldKind::Integer).default_value(0.0))
        .field(FieldInfo::new("finished", FieldKind::Date))
        .rule(Rule::range("cost", None, Some(100.0)))
        .action(
            RecordAction::new("Concluir")
                .id("Finish")
                .enabled("[state] <> 'done'")
                .set("state = 'done'; finished = Today(); cost = [cost] * 2")
                .confirm("Concluir as tarefas?"),
        )
}

fn database(costs: &[f64]) -> MemoryDatabase {
    let database = MemoryDatabase::new(vec![task()]).expect("valid");
    let mut space = database.space();
    for (i, cost) in costs.iter().enumerate() {
        let mut record = space.new_object("Task").expect("known");
        record.set("id", i as f64 + 1.0);
        record.set("state", "open");
        record.set("cost", *cost);
        space.save(record).expect("waits");
    }
    space.commit().expect("keeps");
    database
}

fn frame_over(database: &MemoryDatabase) -> Frame {
    let application = Application::builder("Test").entity(task()).build().expect("valid");
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(database.space()));
    let frame = application.new_frame(FrameContext::Main, space);
    frame.set_view(Some(ViewInfo::new("Task_ListView", ViewKind::List, Some("Task"))));
    frame
}

fn select_all(frame: &Frame, database: &MemoryDatabase) {
    let records = database.space().query("Task", &ravenrs_app::Query::all()).expect("reads");
    frame.set_selected(records);
}

fn field(database: &MemoryDatabase, id: f64, name: &str) -> Value {
    database.space().find("Task", &Value::Number(id)).expect("reads").expect("there").get(name)
}

#[test]
fn it_sets_every_record_selected_from_what_each_held() {
    let database = database(&[10.0, 20.0]);
    let frame = frame_over(&database);
    let finish = frame.action("Finish").expect("it comes by itself");
    assert_eq!(finish.caption(), "Concluir");
    assert_eq!(finish.confirmation_text(), Some("Concluir as tarefas?"));
    assert!(!finish.is_enabled(), "nothing is selected");
    select_all(&frame, &database);
    assert!(finish.is_enabled());
    frame.execute("Finish", ActionInput::None).expect("runs");
    assert_eq!(field(&database, 1.0, "state"), Value::Text("done".into()));
    assert_eq!(field(&database, 1.0, "cost"), Value::Number(20.0));
    assert_eq!(field(&database, 2.0, "cost"), Value::Number(40.0));
    assert!(matches!(field(&database, 2.0, "finished"), Value::Date(_)));
    select_all(&frame, &database);
    assert!(!finish.is_enabled(), "what is done is not done again");
}

#[test]
fn a_record_that_breaks_a_rule_keeps_every_record_as_it_was() {
    let database = database(&[10.0, 60.0]);
    let frame = frame_over(&database);
    select_all(&frame, &database);
    let refused = frame.execute("Finish", ActionInput::None);
    assert!(refused.is_err(), "the second would cost 120");
    assert_eq!(field(&database, 1.0, "state"), Value::Text("open".into()), "the first is kept as it was");
    assert_eq!(field(&database, 1.0, "cost"), Value::Number(10.0));
}

#[test]
fn what_is_wrong_with_a_declaration_comes_out_when_it_is_put_together() {
    let bad = [
        RecordAction::new("A").set("nothing = 1"),
        RecordAction::new("A").set("state = (("),
        RecordAction::new("A").set(""),
        RecordAction::new("A").set("state = 'a'").enabled("[state] ="),
    ];
    for action in bad {
        let entity = EntityInfo::new("T")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("state", FieldKind::Text))
            .action(action);
        assert!(entity.check().is_err());
        assert!(Application::builder("Test").entity(entity).build().is_err());
    }
    let built = Application::builder("Test").entity(task()).module(Assigns).build();
    assert!(built.is_err_and(|e| e.0.contains("the action x")));
}

/// A module with an action that names a field its entity has not.
#[derive(Debug)]
struct Assigns;

struct AssignsController;

impl ravenrs_app::ViewController for AssignsController {
    fn id(&self) -> &str {
        "Assigns"
    }

    fn actions(&self) -> Vec<ravenrs_app::Action> {
        vec![ravenrs_app::Action::simple("x", "x").assign(&task(), "missing = 1")]
    }
}

impl ravenrs_app::Module for Assigns {
    fn name(&self) -> &str {
        "Assigns"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ravenrs_app::ViewController>> {
        vec![Box::new(AssignsController)]
    }
}

#[test]
fn one_record_or_any_number_is_up_to_the_declaration() {
    let only_one = RecordAction::new("Um").selection(Selection::RequireSingle).set("state = 'x'");
    let entity = task().action(only_one);
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let application = Application::builder("Test").entity(entity).build().expect("valid");
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(database.space()));
    let frame = application.new_frame(FrameContext::Main, space);
    frame.set_view(Some(ViewInfo::new("Task_ListView", ViewKind::List, Some("Task"))));
    let one = frame.action("Task.Um").expect("its id follows from the caption");
    let mut record = database.space().new_object("Task").expect("known");
    record.set("id", 1.0);
    frame.set_selected(vec![record.clone(), record]);
    assert!(!one.is_enabled());
}
