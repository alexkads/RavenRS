//! Access for a while, with a reason and a deadline, and revoking a user (#857).
#![allow(clippy::expect_used)]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_app::{
    ACCESS_EXPIRED, AccessError, AuditMode, AuditSettings, AuditTrail, AuthError, Authenticator, Credentials,
    EntityInfo, FieldInfo, FieldKind, Identity, MemoryDatabase, ObjectSpace, Operation, PermissionPolicy,
    Query, Role, RoleMerge, SECURITY_API_KEY, SECURITY_AUDIT_ENTRY, SECURITY_TEMPORARY_ACCESS, SECURITY_USER,
    SecuredObjectSpace, SecurityContext, SpaceMaker, TemporaryAccessPolicy, UserDirectory, Value,
    audit_entity, security_entities,
};
use ravenrs_criteria::DateTime;

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![
        EntityInfo::new("Note")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("text", FieldKind::Text)),
        audit_entity(),
    ];
    all.extend(security_entities());
    all
}

struct World {
    database: MemoryDatabase,
    directory: UserDirectory,
    trail: Rc<AuditTrail>,
    admin: Identity,
    hours: Rc<Cell<u32>>,
}

fn world() -> World {
    let database = MemoryDatabase::new(entities()).expect("entities fit");
    let raw = database.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
    let support = Role::new("Support").policy(PermissionPolicy::ReadOnlyAll);
    let directory = UserDirectory::new(maker, vec![support]).expect("roles");
    let admin = directory.create_first_administrator("ana", "correct horse").expect("created");
    let mut space = database.space();
    let mut note = space.new_object("Note").expect("note");
    note.set("text", "oi");
    space.insert(note).expect("kept");
    space.commit().expect("kept");
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "ana".to_owned()));
    World { database, directory, trail, admin, hours: Rc::new(Cell::new(0)) }
}

impl World {
    fn context(&self, identity: Identity) -> Rc<SecurityContext> {
        Rc::new(
            SecurityContext::new(identity, self.directory.clone(), entities(), RoleMerge::AnyRole)
                .with_audit_trail(self.trail.clone()),
        )
    }

    /// A policy whose clock is now plus the hours the test moves it by.
    fn policy(&self) -> TemporaryAccessPolicy {
        let hours = self.hours.clone();
        TemporaryAccessPolicy::new("Support").max_hours(72).min_reason(10).name_prefix("suporte-").clock(
            move || {
                let now = ravenrs_app::local_now();
                let seconds = i64::from(now.seconds) + i64::from(hours.get()) * 3600;
                DateTime {
                    days: now.days + seconds.div_euclid(86_400),
                    seconds: seconds.rem_euclid(86_400) as u32,
                }
            },
        )
    }

    fn count(&self, entity: &str) -> usize {
        self.database.space().query(entity, &Query::all()).expect("reads").len()
    }
}

#[test]
fn an_administrator_grants_with_a_reason_and_a_deadline_in_one_commit() {
    let world = world();
    let admin = world.context(world.admin.clone());
    let policy = world.policy();
    assert_eq!(admin.grant_temporary_access(&policy, "curto dem", 2), Err(AccessError::ReasonTooShort(10)));
    assert_eq!(admin.grant_temporary_access(&policy, "suporte do caixa", 0), Err(AccessError::Hours(72)));
    assert_eq!(admin.grant_temporary_access(&policy, "suporte do caixa", 73), Err(AccessError::Hours(72)));
    assert_eq!(world.count(SECURITY_TEMPORARY_ACCESS), 0, "a refused grant wrote nothing");
    let users = world.count(SECURITY_USER);

    let granted = admin.grant_temporary_access(&policy, "suporte do caixa", 2).expect("granted");
    assert!(granted.user_name.starts_with("suporte-"));
    assert!(!format!("{granted:?}").contains(&granted.password), "the password shows in Debug");
    assert_eq!(world.count(SECURITY_USER), users + 1);
    let access =
        world.database.space().query(SECURITY_TEMPORARY_ACCESS, &Query::all()).expect("reads").remove(0);
    assert_eq!(access.text("granted_by"), "ana");
    assert_eq!(access.get("user_id"), granted.user_key);
    assert_eq!(access.get("expires_at"), Value::Date(granted.expires_at));
    let now = ravenrs_app::local_now();
    let ahead = (granted.expires_at.days - now.days) * 86_400 + i64::from(granted.expires_at.seconds)
        - i64::from(now.seconds);
    assert!((7_190..=7_210).contains(&ahead), "two hours ahead: {ahead}");

    let support = world
        .directory
        .authenticate(&Credentials::new(&granted.user_name, &granted.password))
        .expect("signs in");
    assert!(support.roles.iter().any(|role| role.name() == "Support"));
    let trail = world.database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads");
    assert!(!trail.is_empty(), "the grant left no trail");
    assert!(
        trail.iter().all(|entry| !format!("{entry:?}").contains(&granted.password)),
        "the password went to the trail"
    );
}

#[test]
fn only_an_administrator_grants_and_revokes() {
    let world = world();
    let reader =
        world.context(Identity::new("bia", "bia", vec![Role::new("r").policy(PermissionPolicy::AllowAll)]));
    let policy = world.policy();
    assert_eq!(reader.grant_temporary_access(&policy, "suporte do caixa", 2), Err(AccessError::NotAllowed));
    assert_eq!(reader.revoke_user(&world.admin.key), Err(AccessError::NotAllowed));
    let admin = world.context(world.admin.clone());
    let wrong = TemporaryAccessPolicy::new("Administrators");
    assert!(matches!(admin.grant_temporary_access(&wrong, "suporte do caixa", 2), Err(AccessError::Role(_))));
}

#[test]
fn past_the_deadline_the_user_cannot_sign_in_and_an_open_session_loses_everything() {
    let world = world();
    let admin = world.context(world.admin.clone());
    let policy = world.policy();
    let granted = admin.grant_temporary_access(&policy, "suporte do caixa", 2).expect("granted");
    let identity =
        world.directory.authenticate(&Credentials::new(&granted.user_name, &granted.password)).expect("in");
    let session = world.context(identity);
    let space = SecuredObjectSpace::new(Box::new(world.database.space()), session.clone());
    assert_eq!(space.query("Note", &Query::all()).expect("reads").len(), 1);

    world.hours.set(3);
    assert_eq!(world.directory.end_expired(policy.now()).expect("ends"), 1);
    let refused = world.directory.authenticate(&Credentials::new(&granted.user_name, &granted.password));
    assert_eq!(refused.map(|_| ()), Err(AuthError::Rejected));
    let access =
        world.database.space().query(SECURITY_TEMPORARY_ACCESS, &Query::all()).expect("reads").remove(0);
    assert_eq!(access.text("ended_reason"), ACCESS_EXPIRED);
    session.reload().expect("reads again");
    assert!(!session.with(|p| p.can(Operation::Read, "Note", None)), "an expired session still reads");
    // An ended access does not change.
    let mut space = world.database.space();
    let mut ended = access.clone();
    ended.set("ended_reason", "outra coisa");
    assert!(space.save(ended).and_then(|()| space.commit()).is_err(), "an ended access was changed");
}

#[test]
fn revoking_turns_off_the_user_their_keys_and_their_access_but_never_oneself() {
    let world = world();
    let admin = world.context(world.admin.clone());
    let policy = world.policy();
    let granted = admin.grant_temporary_access(&policy, "suporte do caixa", 48).expect("granted");
    let mut space = world.database.space();
    let mut key = space.new_object(SECURITY_API_KEY).expect("key");
    key.set("name", "integração");
    key.set("user_id", granted.user_key.clone());
    key.set("active", true);
    space.insert(key).expect("kept");
    space.commit().expect("kept");

    assert_eq!(admin.revoke_user(&world.admin.key), Err(AccessError::OwnUser));
    admin.revoke_user(&granted.user_key).expect("revoked");
    let user = world.database.space().find(SECURITY_USER, &granted.user_key).expect("reads").expect("there");
    assert_eq!(user.get("active"), Value::Bool(false));
    let keys = world.database.space().query(SECURITY_API_KEY, &Query::all()).expect("reads");
    assert!(keys.iter().all(|key| key.get("active") == Value::Bool(false)), "a key stayed active");
    let access =
        world.database.space().query(SECURITY_TEMPORARY_ACCESS, &Query::all()).expect("reads").remove(0);
    assert_eq!(access.text("ended_reason"), "Revoked by ana");
    let refused = world.directory.authenticate(&Credentials::new(&granted.user_name, &granted.password));
    assert_eq!(refused.map(|_| ()), Err(AuthError::Rejected));
}

#[test]
fn the_last_administrator_is_not_revoked() {
    let world = world();
    let other = world.context(Identity::new("root", "root", vec![Role::new("r").administrative()]));
    assert_eq!(other.revoke_user(&world.admin.key), Err(AccessError::LastAdministrator));
    let user = world.database.space().find(SECURITY_USER, &world.admin.key).expect("reads").expect("there");
    assert_eq!(user.get("active"), Value::Bool(true));
}
