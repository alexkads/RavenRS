//! Actions written in the playground's Rust run through its interpreter: declared ones and ones
//! with a closure of their own.
#![cfg(feature = "live")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionInput, ActionResult, Application, EntityInfo, FrameContext, MemoryDatabase, ObjectSpace,
    Query, Value, ViewInfo, ViewKind,
};
use ravenrs_live::{Registry, Value as Live, run};

const SOURCE: &str = r#"
fn task() -> EntityInfo {
    EntityInfo::new("Task")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("state", FieldKind::Text))
        .action(
            RecordAction::new("Concluir")
                .id("Finish")
                .enabled("[state] <> 'done'")
                .set("state = 'done'")
                .confirm("Concluir as tarefas?"),
        )
}

fn count() -> Action {
    Action::simple("Count", "Contar")
        .selection(Selection::RequireMultiple)
        .on_execute(|cx| {
            if cx.selected.len() > 1 {
                cx.records_changed();
                Ok(())
            } else {
                Err(format!("only {}", cx.selected.len()))
            }
        })
}
"#;

fn interpreted<T: 'static>(entry: &str) -> T {
    let mut registry = Registry::new();
    ravenrs_app::live_expose(&mut registry);
    match run(SOURCE, Some(entry), Rc::new(registry)).unwrap_or_else(|e| panic!("{e:?}")) {
        Live::Host(host) => host.take::<T>().expect("of its type"),
        other => panic!("{entry} gave {}", other.kind()),
    }
}

#[derive(Debug)]
struct Counting(Action);

struct Controller(Action);

impl ravenrs_app::ViewController for Controller {
    fn id(&self) -> &str {
        "Counting"
    }

    fn actions(&self) -> Vec<Action> {
        vec![self.0.clone()]
    }
}

impl ravenrs_app::Module for Counting {
    fn name(&self) -> &str {
        "Counting"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ravenrs_app::ViewController>> {
        vec![Box::new(Controller(self.0.clone()))]
    }
}

#[test]
fn declared_and_closure_actions_written_in_the_playground_run() {
    let task: EntityInfo = interpreted("task");
    let count: Action = interpreted("count");
    let database = MemoryDatabase::new(vec![task.clone()]).expect("valid");
    let mut space = database.space();
    for id in [1.0, 2.0] {
        let mut record = space.new_object("Task").expect("known");
        record.set("id", id);
        record.set("state", "open");
        space.save(record).expect("waits");
    }
    space.commit().expect("keeps");
    let application =
        Application::builder("Test").entity(task).module(Counting(count)).build().expect("valid");
    let shared: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(database.space()));
    let frame = application.new_frame(FrameContext::Main, shared);
    frame.set_view(Some(ViewInfo::new("Task_ListView", ViewKind::List, Some("Task"))));
    let all = database.space().query("Task", &Query::all()).expect("reads");

    frame.set_selected(all[..1].to_vec());
    let refused = frame.execute("Count", ActionInput::None);
    assert!(matches!(&refused, Err(ravenrs_app::ActionError::Failed(why)) if why == "only 1"), "{refused:?}");

    frame.set_selected(all.clone());
    assert_eq!(frame.execute("Count", ActionInput::None).expect("runs"), ActionResult::RecordsChanged);
    assert_eq!(frame.action("Finish").expect("there").confirmation_text(), Some("Concluir as tarefas?"));
    frame.execute("Finish", ActionInput::None).expect("runs");
    let states: Vec<Value> = database
        .space()
        .query("Task", &Query::all())
        .expect("reads")
        .iter()
        .map(|r| r.get("state"))
        .collect();
    assert_eq!(states, [Value::Text("done".into()), Value::Text("done".into())]);
    assert!(ravenrs_live::late_problems().is_empty());
}
