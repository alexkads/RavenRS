//! Webhooks pushed in by a provider: signature, organization, one effect per event and the newest
//! fact winning, over the synthetic fixture of two organizations with the same local keys.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests fail with context")]

use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, SpaceMaker, Value};
use ravenrs_app_server::{
    ApiRequest, ApiService, ORGANIZATION_HEADER, SIGNATURE_HEADER, WEBHOOK_EVENT, Webhook, sign,
    webhook_event_entity,
};
use ravenrs_app_testkit::{Clock, Fixture, Organization};
use serde_json::json;

const T1_SECRET: &str = "segredo-sintetico-do-T1";
const T2_SECRET: &str = "segredo-sintetico-do-T2";

/// A charge at a fake provider, which the webhook marks paid or refunded.
fn charge_entity() -> EntityInfo {
    EntityInfo::new("Charge")
        .field(FieldInfo::new("id", FieldKind::Text).key())
        .field(FieldInfo::new("status", FieldKind::Text))
}

fn service_of(organization: Organization) -> (ApiService, Rc<Organization>) {
    let organization = Rc::new(organization);
    let source = organization.clone();
    let maker: SpaceMaker = Rc::new(move || source.raw());
    let hook = Webhook::new(|cx, data| {
        let mut space = cx.space.borrow_mut();
        let mut charge = match space.find("Charge", &Value::Text(cx.subject.to_owned()))? {
            Some(charge) => charge,
            None => {
                let mut charge = space.new_object("Charge")?;
                charge.set("id", cx.subject);
                charge
            }
        };
        charge.set("status", data["status"].as_str().unwrap_or_default());
        space.save(charge)?;
        Ok(())
    })
    .secret(organization.name(), if organization.name() == "T1" { T1_SECRET } else { T2_SECRET });
    let service = ApiService::new(maker, Vec::new()).unwrap().webhook("fakepay", hook);
    (service, organization)
}

fn fixture() -> (ApiService, Rc<Organization>, ApiService, Rc<Organization>) {
    let fixture = Fixture::memory(vec![charge_entity(), webhook_event_entity()]).unwrap();
    let (s1, t1) = service_of(fixture.t1);
    let (s2, t2) = service_of(fixture.t2);
    (s1, t1, s2, t2)
}

fn event(id: &str, at: i64, status: &str) -> Vec<u8> {
    json!({ "id": id, "subject": "charge_9", "at": at, "data": { "status": status } })
        .to_string()
        .into_bytes()
}

fn signed(organization: &str, secret: &str, body: Vec<u8>) -> ApiRequest {
    ApiRequest::new("POST", "/api/hooks/fakepay")
        .header(ORGANIZATION_HEADER, organization)
        .header(SIGNATURE_HEADER, sign(secret.as_bytes(), &body))
        .bytes(body)
}

fn status(organization: &Organization) -> Option<String> {
    let found = organization.raw().find("Charge", &Value::Text("charge_9".into())).unwrap()?;
    match found.get("status") {
        Value::Text(status) => Some(status),
        _ => None,
    }
}

fn events(organization: &Organization) -> usize {
    organization.raw().query(WEBHOOK_EVENT, &Query::all()).unwrap().len()
}

#[test]
fn a_bad_or_missing_signature_is_refused_with_no_effect_and_no_detail() {
    let (mut s1, t1, ..) = fixture();
    let body = event("evt_1", 1, "paid");
    let missing =
        ApiRequest::new("POST", "/api/hooks/fakepay").header(ORGANIZATION_HEADER, "T1").bytes(body.clone());
    let wrong = signed("T1", "not the secret", body.clone());
    let mut tampered = signed("T1", T1_SECRET, body);
    tampered.body = event("evt_1", 1, "refunded");
    for request in [missing, wrong, tampered] {
        let answer = s1.handle(request);
        assert_eq!(answer.status, 401);
        assert_eq!(
            answer.body["message"], "The notification was not accepted.",
            "the same words for every case"
        );
    }
    assert_eq!(status(&t1), None);
    assert_eq!(events(&t1), 0);
    assert_eq!(s1.handle(signed("T1", T1_SECRET, event("evt_1", 1, "paid"))).status, 200);
    assert_eq!(status(&t1).as_deref(), Some("paid"), "no token needed, only the signature");
}

#[test]
fn an_event_of_t2_at_the_address_of_t1_is_refused() {
    let (mut s1, t1, mut s2, t2) = fixture();
    let body = event("evt_1", 1, "paid");
    assert_eq!(s1.handle(signed("T2", T2_SECRET, body.clone())).status, 401, "T1 has no secret for T2");
    assert_eq!(
        s1.handle(signed("T1", T2_SECRET, body.clone())).status,
        401,
        "nor takes T2's secret as its own"
    );
    assert_eq!(status(&t1), None);
    assert_eq!(s2.handle(signed("T2", T2_SECRET, body)).status, 200);
    assert_eq!(status(&t2).as_deref(), Some("paid"));
    assert_eq!(status(&t1), None, "the same key in T1 is untouched");
}

#[test]
fn the_same_event_twice_has_one_effect() {
    let (mut s1, t1, ..) = fixture();
    let first = s1.handle(signed("T1", T1_SECRET, event("evt_1", 1, "paid")));
    assert_eq!(first.body["applied"], true);
    // The provider resends it, the same id, though it says something else now.
    let again = s1.handle(signed("T1", T1_SECRET, event("evt_1", 1, "refunded")));
    assert_eq!((again.status, again.body["duplicate"].clone()), (200, json!(true)));
    assert_eq!(status(&t1).as_deref(), Some("paid"));
    assert_eq!(events(&t1), 1);
}

#[test]
fn an_older_event_after_a_newer_one_leaves_the_newer_state() {
    let (mut s1, t1, ..) = fixture();
    let clock = Clock::at(1_731_600_000);
    let paid_at = clock.now() * 1000;
    clock.advance(60);
    let refunded_at = clock.now() * 1000;
    assert_eq!(
        s1.handle(signed("T1", T1_SECRET, event("evt_2", refunded_at, "refunded"))).body["applied"],
        true
    );
    let late = s1.handle(signed("T1", T1_SECRET, event("evt_1", paid_at, "paid")));
    assert_eq!((late.status, late.body["applied"].clone()), (200, json!(false)), "kept, with no effect");
    assert_eq!(status(&t1).as_deref(), Some("refunded"));
    assert_eq!(events(&t1), 2);
}

#[test]
fn the_secret_is_in_no_record_answer_or_debug_print() {
    let (mut s1, t1, ..) = fixture();
    let answer = s1.handle(signed("T1", T1_SECRET, event("evt_1", 1, "paid")));
    assert!(!answer.body.to_string().contains(T1_SECRET));
    let kept = t1.raw().query(WEBHOOK_EVENT, &Query::all()).unwrap();
    assert!(!format!("{kept:?}").contains(T1_SECRET) && !format!("{kept:?}").contains("sha256="));
    assert!(!format!("{:?}", Webhook::new(|_, _| Ok(())).secret("T1", T1_SECRET)).contains(T1_SECRET));
    let listed = s1.handle(ApiRequest::new("GET", "/api/data/WebhookEvent"));
    assert_ne!(listed.status, 200, "the events are not reachable through the data API");
}

#[test]
fn over_real_http_a_signed_notification_needs_no_token() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    runtime.spawn(ravenrs_app_server::serve(listener, || {
        let fixture = Fixture::memory(vec![charge_entity(), webhook_event_entity()])?;
        Ok(service_of(fixture.t1).0)
    }));
    let url = format!("http://{address}/api/hooks/fakepay");
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let body = event("evt_1", 1, "paid");
    let unsigned = agent.post(&url).header("X-RavenRS-Organization", "T1").send(&body[..]).unwrap();
    assert_eq!(unsigned.status(), 401);
    let mut answer = agent
        .post(&url)
        .header("X-RavenRS-Organization", "T1")
        .header("X-RavenRS-Signature", sign(T1_SECRET.as_bytes(), &body))
        .send(&body[..])
        .unwrap();
    assert_eq!(answer.status(), 200);
    let said: serde_json::Value = serde_json::from_str(&answer.body_mut().read_to_string().unwrap()).unwrap();
    assert_eq!(said["applied"], true);
    let data = agent.get(format!("http://{address}/api/data/Charge")).call().unwrap();
    assert_eq!(data.status(), 401, "the rest of the API still asks for a token");
    runtime.shutdown_background();
}
