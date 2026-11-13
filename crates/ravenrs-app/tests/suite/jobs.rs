//! Actions whose work runs away from the screen, with no window.
#![allow(clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::time::{Duration, Instant};

use ravenrs_app::*;

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Bill")
            .navigation("Finance")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("status", FieldKind::Text)),
    ]
}

struct Holder(Vec<Action>);

impl WindowController for Holder {
    fn id(&self) -> &str {
        "holder"
    }
    fn actions(&self) -> Vec<Action> {
        self.0.clone()
    }
}

struct Setup {
    db: MemoryDatabase,
    frame: Frame,
    told: Rc<RefCell<Vec<(String, MessageKind)>>>,
    woken: Arc<AtomicUsize>,
}

fn setup(actions: Vec<Action>) -> Setup {
    let db = MemoryDatabase::new(entities()).expect("valid");
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(db.space()));
    let frame = Frame::new(FrameContext::Main, space, None, Vec::new(), vec![Box::new(Holder(actions))]);
    frame.set_entities(Rc::new(entities()));
    let told = Rc::new(RefCell::new(Vec::new()));
    let kept = told.clone();
    frame.set_messenger(move |message| kept.borrow_mut().push((message.text.clone(), message.kind)));
    let woken = Arc::new(AtomicUsize::new(0));
    let counting = woken.clone();
    frame.set_job_waker(Arc::new(move || {
        counting.fetch_add(1, Ordering::SeqCst);
    }));
    Setup { db, frame, told, woken }
}

/// Keeps what the work handed back until it ends, as the screen does each time it is woken.
fn until_done(frame: &Frame) -> bool {
    let start = Instant::now();
    let mut changed = false;
    while !frame.jobs().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(5), "the work never ended");
        changed |= frame.apply_jobs();
        std::thread::sleep(Duration::from_millis(2));
    }
    changed
}

fn bill(id: f64) -> Object {
    let mut bill = Object::from_values("Bill", Default::default());
    bill.set("id", id);
    bill.set("status", "Open");
    bill
}

#[test]
fn what_the_work_hands_back_is_kept_on_the_screens_thread() {
    let screen = std::thread::current().id();
    let ran_elsewhere = Arc::new(AtomicBool::new(false));
    let seen = ran_elsewhere.clone();
    let pay = Action::background("pay", "Pagar", move |job| {
        seen.store(std::thread::current().id() != screen, Ordering::SeqCst);
        for (done, record) in job.selected().iter().enumerate() {
            let mut record = record.clone();
            record.set("status", "Paid");
            job.save(record);
            job.progress((done + 1) as f32 / 2.0, "pagando");
        }
        Ok(())
    })
    .selection(Selection::RequireMultiple);
    let s = setup(vec![pay]);
    let (one, two) = (bill(1.0), bill(2.0));
    {
        let mut space = s.db.space();
        space.save(one.clone()).expect("saves");
        space.save(two.clone()).expect("saves");
        space.commit().expect("commits");
    }
    s.frame.set_selected(vec![one, two]);
    let started = s.frame.execute("pay", ActionInput::None).expect("starts");
    assert!(matches!(started, ActionResult::Started(_)));
    assert!(until_done(&s.frame), "records changed");
    assert!(ran_elsewhere.load(Ordering::SeqCst), "the work ran on a thread of its own");
    assert!(s.woken.load(Ordering::SeqCst) > 0, "the screen was woken");
    let kept = s.db.space().find("Bill", &Value::Number(2.0)).expect("reads").expect("there");
    assert_eq!(kept.get("status"), Value::Text("Paid".into()), "it was committed");
}

#[test]
fn the_action_is_off_while_its_work_runs_and_says_how_far_it_got() {
    let gate = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel::<()>();
    let rx = Arc::new(std::sync::Mutex::new(rx));
    let (g, r) = (gate.clone(), rx.clone());
    let scan = Action::background("scan", "Varrer", move |job| {
        job.progress(0.5, "metade");
        g.wait();
        let _ = r.lock().expect("lock").recv();
        Ok(())
    });
    let s = setup(vec![scan]);
    s.frame.execute("scan", ActionInput::None).expect("starts");
    gate.wait();
    let action = s.frame.action("scan").expect("there");
    assert!(!action.is_enabled(), "it does not run twice at once");
    assert!(matches!(s.frame.execute("scan", ActionInput::None), Err(ActionError::Disabled(_))));
    let job = s.frame.jobs().pop().expect("running");
    assert_eq!(job.caption(), "Varrer");
    assert_eq!(job.progress(), (Some(0.5), "metade".to_owned()));
    tx.send(()).expect("lets it end");
    until_done(&s.frame);
    assert!(action.is_enabled(), "it is on again once the work ended");
}

#[test]
fn cancelling_asks_the_work_to_stop_and_what_it_handed_back_stays() {
    let gate = Arc::new(Barrier::new(2));
    let g = gate.clone();
    let count = Action::background("count", "Contar", move |job| {
        job.save(bill(7.0));
        g.wait();
        while !job.cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        job.tell("parou", MessageKind::Warning);
        Ok(())
    });
    let s = setup(vec![count]);
    s.frame.execute("count", ActionInput::None).expect("starts");
    gate.wait();
    let job = s.frame.jobs().pop().expect("running");
    job.cancel();
    assert!(job.is_cancelled());
    until_done(&s.frame);
    assert!(s.db.space().find("Bill", &Value::Number(7.0)).expect("reads").is_some());
    assert_eq!(s.told.borrow().as_slice(), [("parou".to_owned(), MessageKind::Warning)]);
}

#[test]
fn a_failure_or_a_panic_is_told_and_the_action_turns_on_again() {
    let fail = Action::background("fail", "Falhar", |_| Err("sem espaço".to_owned()));
    #[allow(clippy::panic)]
    let crash = Action::background("crash", "Quebrar", |_| panic!("boom"));
    let s = setup(vec![fail, crash]);
    s.frame.execute("fail", ActionInput::None).expect("starts");
    s.frame.execute("crash", ActionInput::None).expect("starts");
    until_done(&s.frame);
    let told: Vec<String> = s.told.borrow().iter().map(|(text, _)| text.clone()).collect();
    assert!(told.contains(&"sem espaço".to_owned()), "{told:?}");
    assert_eq!(told.len(), 2, "the panic is told too: {told:?}");
    assert!(s.frame.action("crash").expect("there").is_enabled());
}

#[test]
fn removing_goes_by_the_key_and_a_bad_change_keeps_nothing() {
    let remove = Action::background("remove", "Remover", |job| {
        job.delete(bill(1.0));
        Ok(())
    });
    let broken = Action::background("broken", "Quebrado", |job| {
        job.save(bill(9.0));
        job.save(Object::from_values("Nothing", Default::default()));
        Ok(())
    });
    let s = setup(vec![remove, broken]);
    {
        let mut space = s.db.space();
        space.save(bill(1.0)).expect("saves");
        space.commit().expect("commits");
    }
    s.frame.execute("remove", ActionInput::None).expect("starts");
    assert!(until_done(&s.frame));
    assert!(s.db.space().find("Bill", &Value::Number(1.0)).expect("reads").is_none());
    s.frame.execute("broken", ActionInput::None).expect("starts");
    assert!(!until_done(&s.frame), "nothing changed");
    assert!(s.db.space().find("Bill", &Value::Number(9.0)).expect("reads").is_none());
    assert_eq!(s.told.borrow().len(), 1, "the error was told");
}

#[test]
fn it_says_it_ran_once_the_work_ended() {
    let said = Rc::new(RefCell::new(0));
    let kept = said.clone();
    let tidy =
        Action::background("tidy", "Arrumar", |_| Ok(())).on_executed(move |_| *kept.borrow_mut() += 1);
    let s = setup(vec![tidy]);
    s.frame.execute("tidy", ActionInput::None).expect("starts");
    assert_eq!(*said.borrow(), 0, "not before it ends");
    until_done(&s.frame);
    assert_eq!(*said.borrow(), 1);
}
