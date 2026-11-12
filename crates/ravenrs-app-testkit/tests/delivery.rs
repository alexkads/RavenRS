//! C07 of `docs/APP-CONTRACTS.md`, delivery side: events leave with limited tries, a stop between
//! the send and the answer is reconciled before anything is sent again, and only those granted the
//! work send or send again.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use ravenrs_app::{
    ActionInput, Application, Dispatcher, Event, EventSink, FrameContext, Identity, OUTBOX_EVENT,
    OUTBOX_REPROCESS, ObjectSpace, ObjectSpaceExt, OutboxModule, Provider, Query, SpaceError, Value,
    ViewInfo, ViewKind,
};
use ravenrs_app_testkit::{Behavior, Clock, FakeSink, Fixture, Organization, roles};

const ID: &str = "T1/note/1";

fn fixture_with_event(clock: &Clock) -> Fixture {
    let fixture = Fixture::memory(Vec::new()).expect("fixture");
    let mut space = fixture.t1.raw();
    space.emit(&Event::new("T1", "note.written", "note/1", "{}").at(clock.date_time())).expect("added");
    space.commit().expect("kept");
    fixture
}

fn worker(organization: &Organization) -> impl ObjectSpace {
    organization.space_as(Identity::new("worker", "worker", vec![roles::events_worker()]))
}

fn field(organization: &Organization, name: &str) -> Value {
    organization
        .raw()
        .find(OUTBOX_EVENT, &Value::Text(ID.to_owned()))
        .expect("reads")
        .expect("there")
        .get(name)
}

fn state(organization: &Organization) -> String {
    match field(organization, "state") {
        Value::Text(state) => state,
        other => format!("{other:?}"),
    }
}

/// A destination in front of which the worker stops after the send, as a process that dies would.
struct StopsAfter(FakeSink);

impl Provider for StopsAfter {
    fn name(&self) -> &str {
        "stops after the send"
    }

    fn capabilities(&self) -> Vec<String> {
        self.0.capabilities()
    }
}

impl EventSink for StopsAfter {
    fn send(&self, key: &str, payload: &str) -> Result<(), SpaceError> {
        let _ = self.0.send(key, payload);
        panic!("the worker stopped before the answer");
    }
}

#[test]
fn a_worker_that_stops_after_the_send_leaves_it_to_be_reconciled() {
    let clock = Clock::default();
    let fixture = fixture_with_event(&clock);
    let sink = FakeSink::new();
    let dispatcher = Dispatcher::new("T1", 3).lease(60);
    let stopped = catch_unwind(AssertUnwindSafe(|| {
        dispatcher.run_at(&mut worker(&fixture.t1), &StopsAfter(sink.clone()), clock.date_time())
    }));
    assert!(stopped.is_err(), "the worker stopped");
    assert_eq!(state(&fixture.t1), "sending", "kept before the send");
    assert_eq!((sink.effects(), sink.sends()), (1, 1));

    clock.advance(30);
    let held = dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");
    assert_eq!(held.waiting, 1, "another run may still be sending it");
    assert_eq!(sink.sends(), 1);

    clock.advance(31);
    let resumed = dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");
    assert_eq!(resumed.confirmed, 1, "the destination said it has it");
    assert_eq!((sink.effects(), sink.sends()), (1, 1), "nothing was sent again");
    assert_eq!(state(&fixture.t1), "confirmed");
}

#[test]
fn a_stop_before_the_destination_kept_it_sends_once_more() {
    let clock = Clock::default();
    let fixture = fixture_with_event(&clock);
    let sink = FakeSink::new();
    sink.then(Behavior::Down);
    let dispatcher = Dispatcher::new("T1", 3).lease(60);
    let _ = catch_unwind(AssertUnwindSafe(|| {
        dispatcher.run_at(&mut worker(&fixture.t1), &StopsAfter(sink.clone()), clock.date_time())
    }));
    clock.advance(61);
    let resumed = dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");
    assert_eq!(resumed.confirmed, 1);
    assert_eq!((sink.effects(), sink.sends()), (1, 2), "asked first, then sent the one it lacked");
    assert_eq!(field(&fixture.t1, "attempts"), Value::Number(2.0));
}

#[test]
fn a_destination_that_stays_down_is_tried_by_the_clock_and_then_fails() {
    let clock = Clock::default();
    let fixture = fixture_with_event(&clock);
    let sink = FakeSink::new();
    sink.then(Behavior::Down).then(Behavior::Down).then(Behavior::Down);
    let dispatcher = Dispatcher::new("T1", 3).backoff(30);
    let run =
        |clock: &Clock| dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");

    assert_eq!(run(&clock).retry, 1);
    clock.advance(29);
    assert_eq!(run(&clock).waiting, 1, "30 s after the first failure");
    clock.advance(1);
    assert_eq!(run(&clock).retry, 1);
    clock.advance(59);
    assert_eq!(run(&clock).waiting, 1, "twice as long after the second");
    clock.advance(1);
    assert_eq!(run(&clock).failed, 1);
    assert_eq!(state(&fixture.t1), "failed");
    assert!(matches!(field(&fixture.t1, "last_error"), Value::Text(error) if error.contains("out of reach")));
    assert_eq!((sink.sends(), sink.effects()), (3, 0));

    let stranger = fixture.t1.space_as(Identity::new("op", "op", vec![roles::registrar("U1")]));
    let seen = stranger.query(OUTBOX_EVENT, &Query::all());
    assert!(
        matches!(&seen, Ok(found) if found.is_empty()) || matches!(seen, Err(SpaceError::Denied(_))),
        "only those granted the events see the failure: {seen:?}"
    );
}

#[test]
fn a_worker_turned_off_is_refused_on_its_next_run() {
    let clock = Clock::default();
    let fixture = fixture_with_event(&clock);
    let sink = FakeSink::new();
    let identity = fixture.t1.add_user("worker", "Events worker").expect("added");
    let context = fixture.t1.context_of(identity);
    let dispatcher = Dispatcher::new("T1", 3);
    let report = dispatcher.run_as(&context, fixture.t1.raw(), &sink, clock.date_time()).expect("granted");
    assert_eq!(report.confirmed, 1);

    let mut space = fixture.t1.raw();
    space.emit(&Event::new("T1", "note.written", "note/2", "{}").at(clock.date_time())).expect("added");
    space.commit().expect("kept");
    fixture.t1.turn_off("worker").expect("turned off");
    let refused = dispatcher.run_as(&context, fixture.t1.raw(), &sink, clock.date_time());
    assert!(matches!(refused, Err(SpaceError::Denied(_))), "got {refused:?}");
    assert_eq!(sink.sends(), 1, "nothing more left");
}

fn reprocess_as(organization: &Organization, identity: Identity) -> Result<(), String> {
    let application = Application::builder("App").module(OutboxModule).build().map_err(|e| e.0)?;
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(organization.space_as(identity)));
    let frame = application.new_frame(FrameContext::Main, space);
    frame.set_view(Some(ViewInfo::new("OutboxEvent_ListView", ViewKind::List, Some(OUTBOX_EVENT))));
    frame.set_selected(organization.raw().query(OUTBOX_EVENT, &Query::all()).expect("reads"));
    frame.execute(OUTBOX_REPROCESS, ActionInput::None).map(|_| ()).map_err(|e| format!("{e:?}"))
}

#[test]
fn sending_again_takes_permission_and_keeps_the_key() {
    let clock = Clock::default();
    let fixture = fixture_with_event(&clock);
    let sink = FakeSink::without_status();
    sink.then(Behavior::LoseReply);
    let dispatcher = Dispatcher::new("T1", 3);
    let lost = dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");
    assert_eq!(lost.unknown, 1);
    assert_eq!(state(&fixture.t1), "unknown", "the destination cannot tell, so a person decides");

    let reader = roles::events_worker().type_permission(
        OUTBOX_EVENT,
        [ravenrs_app::Operation::Write],
        ravenrs_app::Access::Deny,
    );
    let denied = reprocess_as(&fixture.t1, Identity::new("op", "op", vec![reader]));
    assert!(
        denied.as_ref().is_err_and(|e| e.contains("not allowed")),
        "reading is not sending again: {denied:?}"
    );
    assert_eq!(state(&fixture.t1), "unknown");

    reprocess_as(&fixture.t1, Identity::new("op", "op", vec![roles::events_worker()])).expect("allowed");
    assert_eq!(state(&fixture.t1), "pending");
    clock.advance(1);
    let again = dispatcher.run_at(&mut worker(&fixture.t1), &sink, clock.date_time()).expect("runs");
    assert_eq!(again.confirmed, 1);
    assert_eq!(sink.keys(), [ID.to_owned()], "the same key");
    assert_eq!((sink.effects(), sink.sends()), (1, 2), "sent twice, one effect");
}
