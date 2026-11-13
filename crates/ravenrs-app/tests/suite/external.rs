//! Signing in through an outside identity provider: linked by issuer and subject, never widening
//! what the local user can do, and revoked on unlinking.
#![allow(clippy::expect_used)]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_app::{
    Access, AuthError, Authenticator, Claims, Credentials, EntityInfo, ExternalSignIn, FieldInfo, FieldKind,
    IdentityProvider, MemoryDatabase, ObjectSpace, Operation, Provider, Query, Role, RoleMerge,
    SecuredObjectSpace, SecurityContext, SpaceError, SpaceMaker, UserDirectory, Value, security_entities,
};

const ISSUER: &str = "https://login.example.org";

fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("text", FieldKind::Text))
}

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![note()];
    all.extend(security_entities());
    all
}

/// Says who a name is, as a provider would after checking it; can be taken down.
struct FakeIdentities {
    down: Cell<bool>,
}

impl Provider for FakeIdentities {
    fn name(&self) -> &str {
        "fake identities"
    }
}

impl IdentityProvider for FakeIdentities {
    fn verify(&self, credentials: &Credentials) -> Result<Claims, SpaceError> {
        if self.down.get() {
            return Err(SpaceError::Temporary("out of reach".to_owned()));
        }
        match credentials.name.as_str() {
            "ana" => Ok(Claims::new(ISSUER, "sub-ana").email("same@example.org")),
            "other" => Ok(Claims::new("https://elsewhere.example.org", "sub-x").email("same@example.org")),
            _ => Err(SpaceError::Denied("unknown".to_owned())),
        }
    }
}

struct World {
    database: MemoryDatabase,
    directory: UserDirectory,
    provider: Rc<FakeIdentities>,
}

fn world() -> World {
    let database = MemoryDatabase::new(entities()).expect("entities fit");
    let maker: SpaceMaker = {
        let database = database.clone();
        Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
    };
    let readers = Role::new("Readers").type_permission("Note", [Operation::Read], Access::Allow);
    let directory = UserDirectory::new(maker, vec![readers]).expect("directory");
    directory.create_first_administrator("admin", "a long password here").expect("admin");
    let mut space = database.space();
    let mut note = space.new_object("Note").expect("entity");
    note.set("id", 1.0);
    note.set("text", "hello");
    space.save(note).expect("waits");
    space.commit().expect("kept");
    World { database, directory, provider: Rc::new(FakeIdentities { down: Cell::new(false) }) }
}

impl World {
    /// A local user named `ana` with the role Readers, and her key.
    fn local_ana(&self) -> Value {
        let mut space = self.database.space();
        let mut user = space.new_object(ravenrs_app::SECURITY_USER).expect("entity");
        user.set("name", "ana");
        let key = user.get("id");
        space.save(user).expect("waits");
        let role = space
            .query(ravenrs_app::SECURITY_ROLE, &Query::all())
            .expect("reads")
            .into_iter()
            .find(|role| role.get("name") == Value::Text("Readers".to_owned()))
            .expect("declared");
        let mut link = space.new_object(ravenrs_app::SECURITY_USER_ROLE).expect("entity");
        link.set("user_id", key.clone());
        link.set("role_id", role.get("id"));
        space.save(link).expect("waits");
        space.commit().expect("kept");
        key
    }

    fn sign_in(&self) -> ExternalSignIn {
        ExternalSignIn::new(self.directory.clone(), self.provider.clone()).issuer(ISSUER)
    }
}

#[test]
fn a_linked_identity_signs_in_with_the_local_user_and_its_roles() {
    let world = world();
    let ana = world.local_ana();
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    let identity = world.sign_in().authenticate(&Credentials::new("ana", "")).expect("in");
    assert_eq!(identity.key, ana);
    assert_eq!(identity.roles.iter().map(Role::name).collect::<Vec<_>>(), ["Readers"]);
}

#[test]
fn an_identity_with_no_link_is_never_linked_by_e_mail() {
    let world = world();
    world.local_ana();
    assert!(matches!(world.sign_in().authenticate(&Credentials::new("ana", "")), Err(AuthError::Rejected)));
    let ana = world.directory.identity_of("ana").expect("reads").expect("there").key;
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    // Same e-mail, another issuer: not her, and not accepted.
    assert!(matches!(world.sign_in().authenticate(&Credentials::new("other", "")), Err(AuthError::Rejected)));
    assert!(world.directory.linked_user("https://elsewhere.example.org", "sub-x").expect("reads").is_none());
}

#[test]
fn an_identity_linked_to_one_user_is_not_linked_to_another() {
    let world = world();
    let ana = world.local_ana();
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("again, the same");
    let admin = world.directory.identity_of("admin").expect("reads").expect("there").key;
    assert!(matches!(world.directory.link_external(&admin, ISSUER, "sub-ana"), Err(SpaceError::Conflict(_))));
}

#[test]
fn unlinking_ends_the_session_it_opened_on_the_next_reload() {
    let world = world();
    let ana = world.local_ana();
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    let identity = world.sign_in().authenticate(&Credentials::new("ana", "")).expect("in");
    let context = Rc::new(
        SecurityContext::new(identity, world.directory.clone(), entities(), RoleMerge::AnyRole)
            .signed_in_with(ISSUER, "sub-ana"),
    );
    let space = SecuredObjectSpace::new(Box::new(world.database.space()), context.clone());
    assert_eq!(space.query("Note", &Query::all()).expect("reads").len(), 1);

    world.directory.unlink_external(ISSUER, "sub-ana").expect("unlinked");
    context.reload().expect("reloads");
    let after = space.query("Note", &Query::all());
    assert!(
        matches!(&after, Ok(found) if found.is_empty()) || matches!(after, Err(SpaceError::Denied(_))),
        "got {after:?}"
    );
    assert!(matches!(world.sign_in().authenticate(&Credentials::new("ana", "")), Err(AuthError::Rejected)));
}

#[test]
fn a_provider_out_of_reach_grants_nothing_and_local_passwords_still_work() {
    let world = world();
    let ana = world.local_ana();
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    world.provider.down.set(true);
    assert!(matches!(
        world.sign_in().authenticate(&Credentials::new("ana", "")),
        Err(AuthError::Unavailable(_))
    ));
    assert!(
        world.directory.authenticate(&Credentials::new("admin", "a long password here")).is_ok(),
        "the documented way back"
    );
}

#[test]
fn an_issuer_not_accepted_is_rejected() {
    let world = world();
    let ana = world.local_ana();
    world.directory.link_external(&ana, ISSUER, "sub-ana").expect("linked");
    let none = ExternalSignIn::new(world.directory.clone(), world.provider.clone());
    assert!(matches!(none.authenticate(&Credentials::new("ana", "")), Err(AuthError::Rejected)));
}
