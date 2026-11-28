//! Keys the application itself hands out, with no one signed in to ask: `issue_key` and
//! `revoke_key`, checked by the same service as any other key.
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{MemoryDatabase, ObjectSpace, Role, SpaceMaker, UserDirectory, security_entities};
use ravenrs_app_server::{ApiRequest, ApiService, issue_key, revoke_key};

fn roles() -> Vec<Role> {
    vec![Role::new("admin").administrative(), Role::new("agent")]
}

fn space() -> SpaceMaker {
    let database = MemoryDatabase::new(security_entities()).expect("entities");
    let space: SpaceMaker = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(space.clone(), roles()).expect("directory");
    directory.create_first_administrator("ana", "uma-senha-de-teste").expect("first");
    space
}

#[test]
fn an_issued_key_acts_as_its_user_with_only_the_roles_it_keeps_until_it_is_revoked() {
    let space = space();
    let issued = issue_key(&space, roles(), "ana", "agente linux-1", &["admin"], Some(30)).expect("made");
    assert!(issued.key.starts_with("rvk_"));
    let mut service = ApiService::new(space.clone(), roles()).expect("service");
    let me = service.handle(ApiRequest::new("GET", "/api/me").bearer(&issued.key));
    assert_eq!(me.status, 200, "{:?}", me.body);
    revoke_key(&space, &issued.id).expect("revoked");
    revoke_key(&space, &issued.id).expect("a key that is gone is not an error");
    let mut service = ApiService::new(space, roles()).expect("service");
    let after = service.handle(ApiRequest::new("GET", "/api/me").bearer(&issued.key));
    assert_eq!(after.status, 401);
}

#[test]
fn a_key_for_no_user_a_role_the_user_lacks_or_no_name_is_refused() {
    let space = space();
    let refused = |user: &str, name: &str, keep: &[&str], days: Option<i64>| {
        issue_key(&space, roles(), user, name, keep, days).expect_err("refused").status()
    };
    assert_eq!(refused("bia", "x", &[], None), 400);
    assert_eq!(refused("ana", "x", &["agent"], None), 400);
    assert_eq!(refused("ana", " ", &[], None), 400);
    assert_eq!(refused("ana", "x", &[], Some(0)), 400);
}
