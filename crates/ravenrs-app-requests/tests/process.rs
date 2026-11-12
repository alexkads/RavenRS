//! Scenario C06 of docs/APP-CONTRACTS.md: tasks per step, inboxes by role, deadlines, a rule in an
//! expression and delegations that end.
#![allow(clippy::expect_used)]

use ravenrs_app::{Access, Identity, ObjectSpace, Operation, Query, Role, SpaceError, Value};
use ravenrs_app_requests::{
    DECISION, DELEGATION, Decided, Decision, OUTBOX, Outcome, REQUEST, RequestsModule, Step, TASK,
    task_reader,
};
use ravenrs_app_testkit::{Clock, Fixture, Organization};

use Operation::{Create, Read, Write};

const HOUR: i64 = 3600;

fn module() -> RequestsModule {
    RequestsModule::new(vec![
        Step::new("review", "Review", "Reviewers").due_in(24 * HOUR),
        Step::new("approval", "Approval", "Approvers")
            .when("[requester] <> ?CurrentUserName")
            .due_in(48 * HOUR),
    ])
    .expect("steps")
}

/// Works on requests, decisions, events and delegations, and reads only the tasks of its own role.
fn member(role: &str) -> Role {
    task_reader(
        Role::new(role)
            .type_permission(REQUEST, [Read, Write, Create], Access::Allow)
            .type_permission(DECISION, [Read, Write, Create], Access::Allow)
            .type_permission(OUTBOX, [Read, Write, Create], Access::Allow)
            .type_permission(DELEGATION, [Read, Write, Create], Access::Allow)
            .type_permission(TASK, [Write, Create], Access::Allow),
    )
}

fn person(name: &str, role: &str) -> Identity {
    Identity::new(name, name, vec![member(role)])
}

fn fixture() -> Fixture {
    Fixture::memory(module().declared_entities()).expect("fixture")
}

fn submit(organization: &Organization, id: i64, by: &Identity, at: i64) {
    module()
        .submit(&mut organization.space_as(by.clone()), by, id, "New chairs", "U1", at)
        .expect("submitted");
}

fn decide(
    organization: &Organization,
    who: &Identity,
    id: &str,
    request: i64,
    version: i64,
    at: i64,
) -> Result<Decided, SpaceError> {
    let decision =
        Decision { id: id.to_owned(), request, version, outcome: Outcome::Approve, reason: String::new() };
    module().decide(&mut organization.space_as(who.clone()), organization.name(), who, &decision, at)
}

fn inbox(organization: &Organization, who: &Identity) -> Vec<String> {
    organization
        .space_as(who.clone())
        .query(TASK, &Query::all().sort("id", true))
        .expect("reads")
        .into_iter()
        .filter(|task| task.get("state") == Value::Text("open".to_owned()))
        .map(|task| match task.get("id") {
            Value::Text(id) => id,
            _ => String::new(),
        })
        .collect()
}

#[test]
fn each_role_sees_only_the_tasks_it_decides() {
    let fixture = fixture();
    let clock = Clock::default();
    let ana = person("ana", "Requesters");
    let bia = person("bia", "Reviewers");
    let cid = person("cid", "Approvers");
    submit(&fixture.t1, 1, &ana, clock.now());
    assert_eq!(inbox(&fixture.t1, &bia), ["1/review"]);
    assert!(inbox(&fixture.t1, &cid).is_empty());
    assert!(inbox(&fixture.t1, &ana).is_empty());

    decide(&fixture.t1, &bia, "d1", 1, 1, clock.now()).expect("reviewed");
    assert!(inbox(&fixture.t1, &bia).is_empty(), "the decided task closes");
    assert_eq!(inbox(&fixture.t1, &cid), ["1/approval"]);
}

#[test]
fn a_task_is_due_by_its_step() {
    let fixture = fixture();
    let clock = Clock::default();
    let ana = person("ana", "Requesters");
    submit(&fixture.t1, 1, &ana, clock.now());
    let task =
        fixture.t1.raw().find(TASK, &Value::Text("1/review".to_owned())).expect("reads").expect("there");
    assert_eq!(task.get("due"), Value::Number((clock.now() + 24 * HOUR) as f64));
    clock.advance(HOUR);
    decide(&fixture.t1, &person("bia", "Reviewers"), "d1", 1, 1, clock.now()).expect("reviewed");
    let next =
        fixture.t1.raw().find(TASK, &Value::Text("1/approval".to_owned())).expect("reads").expect("there");
    assert_eq!(next.get("due"), Value::Number((clock.now() + 48 * HOUR) as f64));
}

#[test]
fn the_rule_of_a_step_is_an_expression_of_the_application() {
    let fixture = fixture();
    let clock = Clock::default();
    // Dan both asks and approves: the step's rule refuses him on his own request.
    let dan = Identity::new("dan", "dan", vec![member("Approvers"), member("Requesters")]);
    submit(&fixture.t1, 1, &dan, clock.now());
    decide(&fixture.t1, &person("bia", "Reviewers"), "d1", 1, 1, clock.now()).expect("reviewed");
    let own = decide(&fixture.t1, &dan, "d2", 1, 2, clock.now());
    assert!(matches!(own, Err(SpaceError::Denied(_))), "got {own:?}");
    let other = decide(&fixture.t1, &person("cid", "Approvers"), "d3", 1, 2, clock.now());
    assert!(other.is_ok(), "got {other:?}");
}

#[test]
fn a_delegation_lends_the_role_until_its_end() {
    let fixture = fixture();
    let clock = Clock::default();
    let ana = person("ana", "Requesters");
    let bia = person("bia", "Reviewers");
    let cid = person("cid", "Approvers");
    let eva = person("eva", "Requesters");
    for request in [1, 2] {
        submit(&fixture.t1, request, &ana, clock.now());
        decide(&fixture.t1, &bia, &format!("r{request}"), request, 1, clock.now()).expect("reviewed");
    }
    let lend = module().delegate(
        &mut fixture.t1.space_as(cid.clone()),
        &cid,
        "away",
        "eva",
        "Approvers",
        clock.now() + 2 * HOUR,
        clock.now(),
    );
    lend.expect("lent");

    // Signing in again, eva gets the roles lent to her.
    let mut eva = eva;
    let lent = module().delegated_roles(&fixture.t1.space_as(eva.clone()), &eva, clock.now()).expect("reads");
    assert_eq!(lent.len(), 1);
    eva.roles.extend(lent);
    let space = fixture.t1.space_as(eva.clone());
    assert_eq!(module().tasks_for(&space, &eva, clock.now()).expect("reads").len(), 2);
    clock.advance(HOUR);
    assert!(
        decide(&fixture.t1, &eva, "e1", 1, 2, clock.now()).is_ok(),
        "eva decides while the delegation runs"
    );
    clock.advance(2 * HOUR);
    let late = decide(&fixture.t1, &eva, "e2", 2, 2, clock.now());
    assert!(matches!(late, Err(SpaceError::Denied(_))), "got {late:?}");
    assert!(module().tasks_for(&space, &eva, clock.now()).expect("reads").is_empty(), "eva loses the task");
    assert!(decide(&fixture.t1, &cid, "c2", 2, 2, clock.now()).is_ok(), "cid keeps it");
}

#[test]
fn only_a_member_lends_a_role_and_only_into_the_future() {
    let fixture = fixture();
    let clock = Clock::default();
    let ana = person("ana", "Requesters");
    let cid = person("cid", "Approvers");
    let foreign = module().delegate(
        &mut fixture.t1.space_as(ana.clone()),
        &ana,
        "x",
        "eva",
        "Approvers",
        clock.now() + HOUR,
        clock.now(),
    );
    assert!(matches!(foreign, Err(SpaceError::Denied(_))));
    let past = module().delegate(
        &mut fixture.t1.space_as(cid.clone()),
        &cid,
        "y",
        "eva",
        "Approvers",
        clock.now(),
        clock.now(),
    );
    assert!(matches!(past, Err(SpaceError::Refused(_))));
    assert!(fixture.t1.raw().query(DELEGATION, &Query::all()).expect("reads").is_empty());
}

#[test]
fn the_generated_list_of_tasks_shows_the_open_ones() {
    use ravenrs_app::{Application, OutboxModule};
    let app = Application::builder("App").module(OutboxModule).module(module()).build().expect("fits");
    let tasks = app.entities().iter().find(|entity| entity.name == TASK).expect("declared");
    assert_eq!(tasks.list_criteria.as_deref(), Some("[state] = 'open'"));
    let model = format!("{:?}", app.model());
    assert!(model.contains(TASK), "the model has a list of tasks");
}
