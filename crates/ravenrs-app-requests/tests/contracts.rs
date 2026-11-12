//! Scenarios C02, C06 and C07 of docs/APP-CONTRACTS.md, on the synthetic fixture.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use ravenrs_app::{Access, Identity, ObjectSpace, Operation, Query, Role, SpaceError, Value};
use ravenrs_app_requests::{
    APPROVED, DECISION, Decided, Decision, Dispatcher, OUTBOX, OutboxModule, Outcome, REQUEST,
    RequestsModule, Step, forbid_self_decision,
};
use ravenrs_app_testkit::{Behavior, Clock, FakeSink, Fixture, Organization, PERSON, roles};

use Operation::{Create, Read, Write};

fn module() -> RequestsModule {
    RequestsModule::new(vec![
        Step::new("review", "Review", "Reviewers"),
        Step::new("approval", "Approval", "Approvers"),
    ])
    .expect("steps")
    .policy(forbid_self_decision())
}

/// Someone who makes and decides requests: reads and changes them, records decisions and their
/// events. A new record is also written, so creating takes `Write` too.
fn member(role: &str) -> Role {
    Role::new(role)
        .type_permission(REQUEST, [Read, Write, Create], Access::Allow)
        .type_permission(DECISION, [Read, Write, Create], Access::Allow)
        .type_permission(OUTBOX, [Read, Write, Create], Access::Allow)
}

fn person(name: &str, role: &str) -> Identity {
    Identity::new(name, name, vec![member(role)])
}

fn fixture() -> Fixture {
    Fixture::memory(module().declared_entities()).expect("fixture")
}

fn decision(id: &str, version: i64, outcome: Outcome) -> Decision {
    Decision { id: id.to_owned(), request: 1, version, outcome, reason: format!("reason of {id}") }
}

fn count(organization: &Organization, entity: &str) -> usize {
    organization.raw().query(entity, &Query::all()).expect("reads").len()
}

fn state(organization: &Organization) -> Value {
    organization.raw().find(REQUEST, &Value::Number(1.0)).expect("reads").expect("there").get("state")
}

fn submit(organization: &Organization, by: &str) {
    let requester = person(by, "Requesters");
    module()
        .submit(&mut organization.space_as(requester), &person(by, "Requesters"), 1, "New chairs", "U1")
        .expect("submitted");
}

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ravenrs-testkit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("folder");
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// C02: what a profile reaches, and organizations that share keys.

#[test]
fn a_registrar_reads_only_the_people_of_their_unit_without_the_reserved_note() {
    let fixture = fixture();
    let space = fixture.t1.space_as(Identity::new("op", "op", vec![roles::registrar("U1")]));
    let people = space.query(PERSON, &Query::all()).expect("reads");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].get("name"), Value::Text("P1".to_owned()));
    assert_ne!(people[0].get("note"), Value::Text("reserved note of P1".to_owned()));
    let other = space.find(PERSON, &Value::Number(2.0));
    assert!(matches!(other, Ok(None) | Err(SpaceError::Denied(_))), "got {other:?}");
}

#[test]
fn a_registrar_cannot_create_a_person_in_another_unit() {
    let fixture = fixture();
    let mut space = fixture.t1.space_as(Identity::new("op", "op", vec![roles::registrar("U1")]));
    let mut intruder = space.new_object(PERSON).expect("entity");
    intruder.set("id", 9.0);
    intruder.set("unit", "U2");
    intruder.set("name", "Intruder");
    let result = space.save(intruder).and_then(|()| space.commit());
    assert!(matches!(result, Err(SpaceError::Denied(_))), "got {result:?}");
    assert_eq!(count(&fixture.t1, PERSON), 2);
}

#[test]
fn the_same_key_in_two_organizations_never_mixes() {
    let fixture = fixture();
    let admin = || Identity::new("admin", "admin", vec![roles::administrator()]);
    let in_t1 =
        fixture.t1.space_as(admin()).find(PERSON, &Value::Number(1.0)).expect("reads").expect("there");
    let in_t2 =
        fixture.t2.space_as(admin()).find(PERSON, &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(in_t1.get("name"), Value::Text("P1".to_owned()));
    assert_eq!(in_t2.get("name"), Value::Text("P1 of T2".to_owned()));
    assert_eq!(count(&fixture.t2, PERSON), 1);
}

#[test]
fn a_worker_reaches_only_what_its_job_was_granted() {
    let fixture = fixture();
    let space = fixture.t1.space_as(Identity::new("worker", "worker", vec![roles::worker(&[OUTBOX])]));
    let people = space.query(PERSON, &Query::all());
    assert!(matches!(&people, Ok(found) if found.is_empty()) || matches!(people, Err(SpaceError::Denied(_))));
}

// C06: decisions that follow the application's rules and survive a restart.

#[test]
fn whoever_made_a_request_does_not_decide_it() {
    let fixture = fixture();
    submit(&fixture.t1, "ana");
    let ana = person("ana", "Reviewers");
    let result = module().decide(
        &mut fixture.t1.space_as(ana.clone()),
        "T1",
        &ana,
        &decision("d1", 1, Outcome::Approve),
        0,
    );
    assert!(matches!(result, Err(SpaceError::Denied(_))), "got {result:?}");
    assert_eq!(state(&fixture.t1), Value::Text("review".to_owned()));
}

#[test]
fn only_the_role_of_the_step_decides_it() {
    let fixture = fixture();
    submit(&fixture.t1, "ana");
    let bia = person("bia", "Approvers");
    let result = module().decide(
        &mut fixture.t1.space_as(bia.clone()),
        "T1",
        &bia,
        &decision("d1", 1, Outcome::Approve),
        0,
    );
    assert!(matches!(result, Err(SpaceError::Denied(_))), "got {result:?}");
}

#[test]
fn a_decision_on_an_old_version_is_a_conflict() {
    let fixture = fixture();
    submit(&fixture.t1, "ana");
    let bia = person("bia", "Reviewers");
    let first = module().decide(
        &mut fixture.t1.space_as(bia.clone()),
        "T1",
        &bia,
        &decision("d1", 1, Outcome::Approve),
        0,
    );
    assert_eq!(first.expect("kept"), Decided::Recorded { state: "approval".to_owned() });
    let cid = person("cid", "Approvers");
    let stale = module().decide(
        &mut fixture.t1.space_as(cid.clone()),
        "T1",
        &cid,
        &decision("d2", 1, Outcome::Reject),
        0,
    );
    assert!(matches!(stale, Err(SpaceError::Conflict(_))), "got {stale:?}");
    assert_eq!(state(&fixture.t1), Value::Text("approval".to_owned()));
}

#[test]
fn reopening_the_files_continues_on_the_right_step_without_repeating_a_decision() {
    let folder = Folder::new("restart");
    let clock = Clock::default();
    {
        let fixture = Fixture::sqlite(&folder.0, module().declared_entities()).expect("fixture");
        submit(&fixture.t1, "ana");
        let bia = person("bia", "Reviewers");
        let kept = module().decide(
            &mut fixture.t1.space_as(bia.clone()),
            "T1",
            &bia,
            &decision("d1", 1, Outcome::Approve),
            clock.now(),
        );
        assert_eq!(kept.expect("kept"), Decided::Recorded { state: "approval".to_owned() });
    }
    clock.advance(3600);
    let fixture = Fixture::sqlite(&folder.0, module().declared_entities()).expect("reopened");
    assert_eq!(state(&fixture.t1), Value::Text("approval".to_owned()));
    let bia = person("bia", "Reviewers");
    let again = module().decide(
        &mut fixture.t1.space_as(bia.clone()),
        "T1",
        &bia,
        &decision("d1", 1, Outcome::Approve),
        clock.now(),
    );
    assert_eq!(again.expect("answered"), Decided::AlreadyRecorded);
    assert_eq!(count(&fixture.t1, DECISION), 1);
    let cid = person("cid", "Approvers");
    let last = module().decide(
        &mut fixture.t1.space_as(cid.clone()),
        "T1",
        &cid,
        &decision("d2", 2, Outcome::Approve),
        clock.now(),
    );
    assert_eq!(last.expect("kept"), Decided::Recorded { state: APPROVED.to_owned() });
    assert_eq!(count(&fixture.t1, OUTBOX), 2);
}

// C07: events leave only after their commit, and never twice.

#[test]
fn a_refused_decision_leaves_no_event() {
    let fixture = fixture();
    submit(&fixture.t1, "ana");
    let ana = person("ana", "Reviewers");
    let _ = module().decide(
        &mut fixture.t1.space_as(ana.clone()),
        "T1",
        &ana,
        &decision("d1", 1, Outcome::Approve),
        0,
    );
    assert_eq!(count(&fixture.t1, OUTBOX), 0);
    assert_eq!(count(&fixture.t1, DECISION), 0);
}

fn decided(organization: &Organization) {
    submit(organization, "ana");
    let bia = person("bia", "Reviewers");
    module()
        .decide(
            &mut organization.space_as(bia.clone()),
            organization.name(),
            &bia,
            &decision("d1", 1, Outcome::Approve),
            0,
        )
        .expect("kept");
}

fn worker(organization: &Organization) -> impl ObjectSpace {
    organization.space_as(Identity::new("worker", "worker", vec![roles::worker(&[OUTBOX])]))
}

#[test]
fn an_answer_lost_on_the_way_back_is_checked_before_sending_again() {
    let fixture = fixture();
    decided(&fixture.t1);
    let sink = FakeSink::new();
    sink.then(Behavior::LoseReply);
    let dispatcher = Dispatcher::new("T1", 3);
    let first = dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs");
    assert_eq!((first.unknown, sink.effects(), sink.sends()), (1, 1, 1));
    let second = dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs");
    assert_eq!(second.confirmed, 1);
    assert_eq!((sink.effects(), sink.sends()), (1, 1), "nothing was sent again");
    let third = dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs");
    assert_eq!(third, Default::default(), "nothing is left to send");
}

#[test]
fn a_destination_that_stays_down_ends_in_failed_after_the_limit() {
    let fixture = fixture();
    decided(&fixture.t1);
    let sink = FakeSink::new();
    sink.then(Behavior::Down).then(Behavior::Down).then(Behavior::Down);
    let dispatcher = Dispatcher::new("T1", 3);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs").retry, 1);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs").retry, 1);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs").failed, 1);
    assert_eq!(sink.effects(), 0);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs"), Default::default());
}

#[test]
fn the_same_local_key_in_two_organizations_makes_two_effects() {
    let fixture = fixture();
    decided(&fixture.t1);
    decided(&fixture.t2);
    let sink = FakeSink::new();
    Dispatcher::new("T1", 3).run(&mut worker(&fixture.t1), &sink).expect("runs");
    Dispatcher::new("T2", 3).run(&mut worker(&fixture.t2), &sink).expect("runs");
    assert_eq!(sink.keys(), vec!["T1/decision/d1".to_owned(), "T2/decision/d1".to_owned()]);
}

#[test]
fn a_package_needs_at_least_one_step() {
    assert!(RequestsModule::new(Vec::new()).is_err());
}

#[test]
fn a_destination_that_cannot_tell_leaves_a_lost_answer_to_reconcile() {
    let fixture = fixture();
    decided(&fixture.t1);
    let sink = FakeSink::without_status();
    sink.then(Behavior::LoseReply);
    let dispatcher = Dispatcher::new("T1", 3);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs").unknown, 1);
    assert_eq!(dispatcher.run(&mut worker(&fixture.t1), &sink).expect("runs").unknown, 1);
    assert_eq!(sink.sends(), 1, "nothing is sent blindly again");
}

#[test]
fn a_refused_event_fails_at_once() {
    let fixture = fixture();
    decided(&fixture.t1);
    let sink = FakeSink::new();
    sink.then(Behavior::Refuse);
    let report = Dispatcher::new("T1", 3).run(&mut worker(&fixture.t1), &sink).expect("runs");
    assert_eq!((report.failed, sink.effects()), (1, 0));
}

#[test]
fn the_package_says_its_version_and_its_job() {
    use ravenrs_app::{Application, Module};
    let manifest = module().manifest();
    assert!(manifest.version.is_some());
    assert_eq!(manifest.jobs, vec![ravenrs_app_requests::DISPATCH_JOB.to_owned()]);
    let without_events = Application::builder("App").module(module()).build();
    assert!(without_events.is_err_and(|e| e.0.contains("Outbox")), "events come from the framework's module");
    let app = Application::builder("App").module(module()).module(OutboxModule).build().expect("fits");
    let names: Vec<String> = app.manifests().into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["Outbox", "Requests"]);
}
