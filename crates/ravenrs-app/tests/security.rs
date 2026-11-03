//! Permissions, signing in and the audit trail, with no screen around them.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_app::{
    Access, Action, AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace, AuthError, Authenticator,
    Change, Credentials, EntityInfo, FieldInfo, FieldKind, Frame, FrameContext, Identity, MemoryDatabase,
    Object, ObjectSpace, Operation, PasswordError, PermissionPolicy, Permissions, Query, Role, RoleMerge,
    SECURITY_AUDIT_ENTRY, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SecuredObjectSpace,
    SecurityContext, SpaceError, SpaceMaker, UserDirectory, Value, ViewController, ViewInfo, ViewKind,
    audit_entity, security_entities,
};
use ravenrs_criteria::Criteria;

use Operation::{Create, Delete, Read, Write};

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("owner", FieldKind::Text))
        .field(FieldInfo::new("cost", FieldKind::Integer))
        .field(FieldInfo::new("double_cost", FieldKind::Integer).calculated("[cost] * 2"))
        .field(FieldInfo::new("token", FieldKind::Text).secret())
        .field(FieldInfo::new("lines", FieldKind::Collection("Line".to_owned())))
}

fn line() -> EntityInfo {
    let lines = FieldInfo::new("notes", FieldKind::Collection("Note".to_owned()))
        .back_reference("line_id")
        .aggregated();
    EntityInfo::new("Line")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("order_id", FieldKind::Reference("Order".to_owned())))
        .field(FieldInfo::new("text", FieldKind::Text))
        .field(lines)
}

fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("line_id", FieldKind::Reference("Line".to_owned())))
}

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![order(), line(), note(), audit_entity()];
    all.extend(security_entities());
    all
}

struct World {
    database: MemoryDatabase,
    directory: UserDirectory,
}

fn raw_maker(database: &MemoryDatabase) -> SpaceMaker {
    let database = database.clone();
    Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
}

fn world() -> World {
    let database = MemoryDatabase::new(entities()).expect("entities fit");
    let directory = UserDirectory::new(raw_maker(&database), Vec::new()).expect("roles");
    let mut space = database.space();
    for (id, owner, cost) in [(1.0, "ana", 10.0), (2.0, "bia", 20.0), (3.0, "ana", 30.0)] {
        let mut record = space.new_object("Order").expect("order");
        record.set("id", id);
        record.set("name", format!("Order {id}"));
        record.set("owner", owner);
        record.set("cost", cost);
        record.set("token", "top-secret");
        space.save(record).expect("saved");
    }
    space.commit().expect("committed");
    World { database, directory }
}

impl World {
    fn context(&self, roles: Vec<Role>) -> Rc<SecurityContext> {
        Rc::new(SecurityContext::new(
            Identity::new("ana", "ana", roles),
            self.directory.clone(),
            entities(),
            RoleMerge::AnyRole,
        ))
    }

    fn secured(&self, roles: Vec<Role>) -> SecuredObjectSpace {
        SecuredObjectSpace::new(Box::new(self.database.space()), self.context(roles))
    }

    fn stored(&self, id: f64) -> Object {
        self.database.space().find("Order", &Value::Number(id)).expect("reads").expect("there")
    }
}

fn names(found: &[Object]) -> Vec<f64> {
    found
        .iter()
        .map(|object| match object.get("id") {
            Value::Number(id) => id,
            _ => -1.0,
        })
        .collect()
}

fn denied<T: std::fmt::Debug>(result: Result<T, SpaceError>) {
    assert!(matches!(result, Err(SpaceError::Denied(_))), "expected a refusal, got {result:?}");
}

fn permissions(roles: Vec<Role>, merge: RoleMerge) -> Permissions {
    Permissions::new(Identity::new("ana", "ana", roles), entities(), merge)
}

#[test]
fn a_policy_says_what_no_permission_mentions() {
    let deny = permissions(vec![Role::new("r")], RoleMerge::AnyRole);
    assert!(!deny.can(Read, "Order", None));
    let read = permissions(vec![Role::new("r").policy(PermissionPolicy::ReadOnlyAll)], RoleMerge::AnyRole);
    assert!(read.can(Read, "Order", None));
    assert!(!read.can(Write, "Order", None));
    assert!(!read.can(Create, "Order", None));
    assert!(!read.can(Delete, "Order", None));
    let all = permissions(vec![Role::new("r").policy(PermissionPolicy::AllowAll)], RoleMerge::AnyRole);
    for operation in [Read, Write, Create, Delete] {
        assert!(all.can(operation, "Order", None));
    }
}

#[test]
fn someone_with_no_roles_can_do_nothing() {
    for merge in [RoleMerge::AnyRole, RoleMerge::AllRoles] {
        let nobody = permissions(Vec::new(), merge);
        assert!(!nobody.can(Read, "Order", None));
        assert!(!nobody.can_field(Read, "Order", "name", None));
    }
}

#[test]
fn the_levels_go_in_order_from_field_by_criteria_down_to_policy() {
    let role = Role::new("r")
        .policy(PermissionPolicy::AllowAll)
        .type_permission("Order", [Read], Access::Deny)
        .object_permission("Order", "[owner] = 'ana'", [Read], Access::Allow)
        .member_permission("Order", ["cost"], None, [Read], Access::Deny)
        .member_permission("Order", ["cost"], Some("[cost] < 15"), [Read], Access::Allow);
    let all = permissions(vec![role], RoleMerge::AnyRole);
    let world = world();
    let (first, second, third) = (world.stored(1.0), world.stored(2.0), world.stored(3.0));
    // The type denies; a record of the object permission comes first.
    assert!(all.can(Read, "Order", Some(&first)));
    assert!(!all.can(Read, "Order", Some(&second)));
    // The field by criteria comes before the field, and the field before the record.
    assert!(all.can_field(Read, "Order", "cost", Some(&first)));
    assert!(!all.can_field(Read, "Order", "cost", Some(&third)));
    // What no level mentions falls to the policy.
    assert!(all.can(Write, "Order", Some(&first)));
}

#[test]
fn a_denial_beats_an_allowance_at_the_same_level() {
    let role = Role::new("r").type_permission("Order", [Read], Access::Allow).type_permission(
        "Order",
        [Read],
        Access::Deny,
    );
    assert!(!permissions(vec![role], RoleMerge::AnyRole).can(Read, "Order", None));
}

#[test]
fn several_roles_combine_in_any_or_in_all() {
    let reads = Role::new("reads").type_permission("Order", [Read], Access::Allow);
    let denies = Role::new("denies").type_permission("Order", [Read], Access::Deny);
    let both = vec![reads, denies];
    assert!(permissions(both.clone(), RoleMerge::AnyRole).can(Read, "Order", None));
    assert!(!permissions(both, RoleMerge::AllRoles).can(Read, "Order", None));
}

#[test]
fn an_administrative_role_takes_no_denial() {
    let role = Role::new("r").administrative().type_permission("Order", [Read], Access::Deny);
    let all = permissions(vec![role], RoleMerge::AnyRole);
    assert!(all.can(Read, "Order", None));
    assert!(all.can_edit_model());
    assert!(all.can_use_action("Anything"));
}

#[test]
fn a_criteria_that_does_not_read_opens_nothing_and_closes_everything() {
    let world = world();
    let record = world.stored(1.0);
    let allows = Role::new("r").object_permission("Order", "((", [Read], Access::Allow);
    assert!(!permissions(vec![allows], RoleMerge::AnyRole).can(Read, "Order", Some(&record)));
    let denies = Role::new("r").policy(PermissionPolicy::AllowAll).object_permission(
        "Order",
        "((",
        [Read],
        Access::Deny,
    );
    assert!(!permissions(vec![denies], RoleMerge::AnyRole).can(Read, "Order", Some(&record)));
}

#[test]
fn navigation_goes_by_item_before_group_and_only_hides() {
    let role = Role::new("r")
        .navigation_permission("Sales", Access::Deny)
        .navigation_permission("Orders", Access::Allow);
    let all = permissions(vec![role], RoleMerge::AnyRole);
    assert!(all.can_navigate(Some("Sales"), "Orders"));
    assert!(!all.can_navigate(Some("Sales"), "Quotes"));
    assert!(all.can_navigate(Some("Other"), "Thing"));
}

struct Payments;

impl ViewController for Payments {
    fn id(&self) -> &str {
        "Payments"
    }
    fn actions(&self) -> Vec<Action> {
        vec![Action::simple("Pay", "Pay"), Action::simple("Remove", "Remove").requires(Delete)]
    }
}

#[test]
fn a_denied_action_goes_away_and_the_frame_says_so() {
    let world = world();
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).action_permission("Pay", Access::Deny);
    let context = world.context(vec![role]);
    assert!(!context.with(|p| p.can_use_action("Pay")));
    let space: Rc<std::cell::RefCell<dyn ObjectSpace>> =
        Rc::new(std::cell::RefCell::new(world.database.space()));
    let frame = Frame::new(FrameContext::Main, space, None, vec![Box::new(Payments)], Vec::new());
    frame.set_entities(Rc::new(entities()));
    frame.set_security(context);
    frame.set_view(Some(ViewInfo::new("Order_ListView", ViewKind::List, Some("Order"))));
    let pay = frame.action("Pay").expect("there");
    assert!(!pay.is_active());
    let remove = frame.action("Remove").expect("there");
    assert!(remove.is_active());
    assert!(!remove.is_enabled(), "they only read, so it fades");
}

#[test]
fn a_record_they_cannot_read_is_not_there() {
    let world = world();
    let space = world.secured(vec![Role::new("r").object_permission(
        "Order",
        "[owner] = ?CurrentUserName",
        [Read],
        Access::Allow,
    )]);
    assert_eq!(names(&space.query("Order", &Query::all()).expect("reads")), [1.0, 3.0]);
    assert!(space.find("Order", &Value::Number(2.0)).expect("reads").is_none());
}

#[test]
fn a_page_only_counts_what_they_can_see() {
    let world = world();
    let space = world.secured(vec![Role::new("r").object_permission(
        "Order",
        "[owner] = 'ana'",
        [Read],
        Access::Allow,
    )]);
    let page = space.query("Order", &Query::all().sort("id", false).page(1, 1)).expect("reads");
    assert_eq!(names(&page), [1.0]);
}

#[test]
fn a_field_they_cannot_read_comes_empty_and_a_filter_cannot_guess_it() {
    let world = world();
    let space = world.secured(vec![Role::new("r").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Read],
        Access::Deny,
    )]);
    let found = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(found.get("cost"), Value::Null);
    assert_eq!(found.get("name"), Value::Text("Order 1".into()));
    let guess = Query::all().filter(Criteria::parse("[cost] = 10").expect("reads"));
    assert!(space.query("Order", &guess).expect("reads").is_empty());
    // The order cannot tell it either: every empty value ties, and the key decides.
    let sorted = space.query("Order", &Query::all().sort("cost", false)).expect("reads");
    assert_eq!(names(&sorted), [1.0, 2.0, 3.0]);
}

#[test]
fn a_calculated_field_does_not_give_away_what_it_is_made_of() {
    let world = world();
    let space = world.secured(vec![Role::new("r").policy(PermissionPolicy::ReadOnlyAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Read],
        Access::Deny,
    )]);
    let found = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(found.get("double_cost"), Value::Null);
    let all =
        space.query("Order", &Query::all().filter(Criteria::parse("[double_cost] = 20").expect("reads")));
    assert!(all.expect("reads").is_empty());
}

#[test]
fn a_secret_field_is_kept_even_from_an_administrator() {
    let world = world();
    let mut space = world.secured(vec![Role::new("r").administrative()]);
    let found = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(found.get("token"), Value::Null);
    let guess = Query::all().filter(Criteria::parse("[token] = 'top-secret'").expect("reads"));
    assert!(space.query("Order", &guess).expect("reads").is_empty());
    let mut changed = found.clone();
    changed.set("token", "mine");
    denied(space.save(changed));
    // Saved back empty, it stays what it was.
    let mut named = found;
    named.set("name", "Renamed");
    space.save(named).expect("saved");
    space.commit().expect("committed");
    assert_eq!(world.stored(1.0).get("token"), Value::Text("top-secret".into()));
    assert_eq!(world.stored(1.0).get("name"), Value::Text("Renamed".into()));
}

#[test]
fn writing_back_a_field_they_cannot_read_keeps_what_is_stored() {
    let world = world();
    let mut space = world.secured(vec![Role::new("r").policy(PermissionPolicy::AllowAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Read, Write],
        Access::Deny,
    )]);
    let mut found = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    found.set("name", "Renamed");
    space.save(found.clone()).expect("saved");
    space.commit().expect("committed");
    assert_eq!(world.stored(1.0).get("cost"), Value::Number(10.0));
    found.set("cost", 999.0);
    denied(space.save(found));
}

#[test]
fn a_write_is_checked_before_and_after_the_change() {
    let world = world();
    let mut space =
        world.secured(vec![Role::new("r").policy(PermissionPolicy::ReadOnlyAll).object_permission(
            "Order",
            "[owner] = 'ana'",
            [Write],
            Access::Allow,
        )]);
    // Someone else's.
    let mut other = space.find("Order", &Value::Number(2.0)).expect("reads").expect("there");
    other.set("name", "Mine now");
    denied(space.save(other.clone()));
    // Taking it over by also writing the owner does not help.
    other.set("owner", "ana");
    denied(space.save(other));
    // Their own, but giving it away leaves what they can write.
    let mut own = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    own.set("owner", "bia");
    denied(space.save(own.clone()));
    own.set("owner", "ana");
    own.set("name", "Fine");
    space.save(own).expect("saved");
    space.commit().expect("committed");
    assert_eq!(world.stored(1.0).get("name"), Value::Text("Fine".into()));
    assert_eq!(world.stored(2.0).get("owner"), Value::Text("bia".into()));
}

#[test]
fn creating_takes_create_and_write() {
    let world = world();
    let creates = Role::new("r").type_permission("Order", [Create], Access::Allow);
    let mut space = world.secured(vec![creates]);
    let mut object = space.new_object("Order").expect("can start one");
    object.set("id", 9.0);
    denied(space.save(object.clone()));
    denied(space.insert(object));

    let writes = Role::new("r").type_permission("Order", [Write], Access::Allow);
    let mut space = world.secured(vec![writes]);
    denied(space.new_object("Order"));

    let both = Role::new("r").type_permission("Order", [Create, Write], Access::Allow);
    let mut space = world.secured(vec![both]);
    let mut object = space.new_object("Order").expect("can start one");
    object.set("id", 9.0);
    space.insert(object).expect("inserted");
    space.commit().expect("committed");
    assert_eq!(world.stored(9.0).get("id"), Value::Number(9.0));
}

#[test]
fn a_new_record_cannot_fill_a_field_they_cannot_write() {
    let world = world();
    let role = Role::new("r").type_permission("Order", [Create, Write], Access::Allow).member_permission(
        "Order",
        ["cost"],
        None,
        [Write],
        Access::Deny,
    );
    let mut space = world.secured(vec![role]);
    let mut object = space.new_object("Order").expect("can start one");
    object.set("id", 9.0);
    object.set("cost", 5.0);
    denied(space.insert(object));
}

#[test]
fn deleting_goes_by_the_record_and_save_over_a_key_is_a_write() {
    let world = world();
    let role = Role::new("r").object_permission("Order", "[owner] = 'ana'", [Read, Delete], Access::Allow);
    let mut space = world.secured(vec![role]);
    denied(space.delete("Order", &Value::Number(2.0)));
    space.delete("Order", &Value::Number(1.0)).expect("deleted");
    space.commit().expect("committed");
    assert!(world.database.space().find("Order", &Value::Number(1.0)).expect("reads").is_none());
    // Someone who only creates cannot take over a record by saving a new one with its key.
    let mut intruder = world.secured(vec![Role::new("r").type_permission("Order", [Create], Access::Allow)]);
    let mut takeover = intruder.new_object("Order").expect("can start one");
    takeover.set("id", 2.0);
    takeover.set("name", "Taken");
    denied(intruder.save(takeover));
}

#[test]
fn the_records_of_an_aggregated_collection_go_with_their_owner() {
    let role = Role::new("r")
        .member_permission("Line", ["notes"], None, [Read], Access::Allow)
        .type_permission("Line", [Write], Access::Allow);
    let all = permissions(vec![role], RoleMerge::AnyRole);
    assert!(all.can(Read, "Note", None));
    // A write on the owner, nothing on the list: it covers changing and deleting too.
    for operation in [Write, Create, Delete] {
        assert!(all.can(operation, "Note", None));
    }
    // Something explicit on the records themselves wins.
    let closed = Role::new("r").type_permission("Line", [Read], Access::Allow).type_permission(
        "Note",
        [Read],
        Access::Deny,
    );
    assert!(!permissions(vec![closed], RoleMerge::AnyRole).can(Read, "Note", None));
}

#[test]
fn what_is_not_aggregated_needs_its_own_permission() {
    let all =
        permissions(vec![Role::new("r").type_permission("Order", [Read], Access::Allow)], RoleMerge::AnyRole);
    assert!(!all.can(Read, "Line", None));
}

#[test]
fn only_an_administrator_reaches_who_can_do_what() {
    let world = world();
    let open = Role::new("r")
        .policy(PermissionPolicy::AllowAll)
        .type_permission(SECURITY_ROLE, [Read, Write, Create, Delete], Access::Allow)
        .type_permission(SECURITY_USER, [Read, Write, Create, Delete], Access::Allow);
    let mut space = world.secured(vec![open]);
    assert!(space.query(SECURITY_USER, &Query::all()).expect("reads").is_empty());
    denied(space.new_object(SECURITY_ROLE));
    denied(space.new_object(SECURITY_USER_ROLE));
}

#[test]
fn the_audit_trail_can_only_be_read_and_only_by_an_administrator() {
    let world = world();
    let mut admin = world.secured(vec![Role::new("r").administrative()]);
    denied(admin.new_object(SECURITY_AUDIT_ENTRY));
    let mut reader = world.secured(vec![Role::new("r").policy(PermissionPolicy::AllowAll)]);
    denied(reader.new_object(SECURITY_AUDIT_ENTRY));
    assert!(reader.query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads").is_empty());
}

fn signed_up(world: &World, name: &str) -> Identity {
    world.directory.create_first_administrator(name, "correct horse").expect("created")
}

#[test]
fn the_first_person_is_an_administrator_and_there_is_only_one_first() {
    let world = world();
    assert!(world.directory.is_empty().expect("reads"));
    assert_eq!(
        world.directory.create_first_administrator("ana", "short").err(),
        Some(PasswordError::TooShort)
    );
    let ana = signed_up(&world, "ana");
    assert!(ana.roles.iter().any(Role::is_administrative));
    assert!(!ana.must_change_password);
    assert_eq!(
        world.directory.create_first_administrator("eve", "correct horse").err(),
        Some(PasswordError::NotAllowed)
    );
    let hash = world
        .database
        .space()
        .query(SECURITY_USER, &Query::all())
        .expect("reads")
        .remove(0)
        .get("password_hash");
    assert!(
        matches!(&hash, Value::Text(text) if text.starts_with("$argon2id$") && !text.contains("correct"))
    );
}

#[test]
fn signing_in_says_nothing_about_who_exists() {
    let world = world();
    signed_up(&world, "ana");
    let right = world.directory.authenticate(&Credentials::new("ana", "correct horse")).expect("signs in");
    assert_eq!(right.name, "ana");
    let wrong = world.directory.authenticate(&Credentials::new("ana", "wrong one"));
    let nobody = world.directory.authenticate(&Credentials::new("nobody", "wrong one"));
    assert_eq!(wrong.as_ref().map(|_| ()), Err(&AuthError::Rejected));
    assert_eq!(nobody.as_ref().map(|_| ()), Err(&AuthError::Rejected));
}

#[test]
fn a_name_never_becomes_part_of_an_expression() {
    let world = world();
    signed_up(&world, "ana");
    let injected =
        world.directory.authenticate(&Credentials::new("x' Or 1 = 1 Or [name] = 'x", "correct horse"));
    assert!(matches!(injected, Err(AuthError::Rejected)));
}

#[test]
fn credentials_do_not_show_the_password_when_printed() {
    assert!(!format!("{:?}", Credentials::new("ana", "correct horse")).contains("horse"));
}

#[test]
fn too_many_wrong_passwords_make_an_account_wait() {
    let world = world();
    signed_up(&world, "ana");
    for _ in 0..5 {
        assert!(world.directory.authenticate(&Credentials::new("ana", "wrong one")).is_err());
    }
    // Even the right one waits now.
    assert!(matches!(
        world.directory.authenticate(&Credentials::new("ana", "correct horse")),
        Err(AuthError::Rejected)
    ));
    let user = world.database.space().query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    assert!(matches!(user.get("locked_until"), Value::Date(_)));
}

#[test]
fn someone_turned_off_cannot_sign_in() {
    let world = world();
    signed_up(&world, "ana");
    let mut space = world.database.space();
    let mut user = space.query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    user.set("active", false);
    space.save(user).expect("saved");
    space.commit().expect("committed");
    assert!(matches!(
        world.directory.authenticate(&Credentials::new("ana", "correct horse")),
        Err(AuthError::Rejected)
    ));
    assert!(world.directory.identity_of("ana").expect("reads").is_none());
}

/// A person with a role of this name, made as an administrator would.
fn add_user(world: &World, name: &str, role_name: &str, administrative: bool) -> Value {
    let mut space = world.database.space();
    let mut role = space.new_object(SECURITY_ROLE).expect("role");
    role.set("name", role_name);
    role.set("administrative", administrative);
    role.set("policy", "read_only_all");
    let role_key = role.get("id");
    space.save(role).expect("saved");
    let mut user = space.new_object(SECURITY_USER).expect("user");
    user.set("name", name);
    let key = user.get("id");
    space.save(user).expect("saved");
    let mut link = space.new_object(SECURITY_USER_ROLE).expect("link");
    link.set("user_id", key.clone());
    link.set("role_id", role_key);
    space.save(link).expect("saved");
    space.commit().expect("committed");
    key
}

fn context_of(world: &World, identity: Identity) -> Rc<SecurityContext> {
    Rc::new(SecurityContext::new(identity, world.directory.clone(), entities(), RoleMerge::AnyRole))
}

#[test]
fn passwords_change_with_the_current_one_or_by_an_administrator() {
    let world = world();
    let ana = signed_up(&world, "ana");
    let context = context_of(&world, ana);
    assert_eq!(context.change_own_password("wrong one", "brand new one"), Err(PasswordError::WrongCurrent));
    assert_eq!(context.change_own_password("correct horse", "short"), Err(PasswordError::TooShort));
    context.change_own_password("correct horse", "brand new one").expect("changed");
    assert!(world.directory.authenticate(&Credentials::new("ana", "brand new one")).is_ok());

    let bia = add_user(&world, "bia", "Readers", false);
    context.set_password_of(&bia, "given by ana").expect("set");
    let identity = world.directory.authenticate(&Credentials::new("bia", "given by ana")).expect("signs in");
    assert!(identity.must_change_password);
    let bia_context = context_of(&world, identity);
    let ana_key = context.identity().key;
    assert_eq!(bia_context.set_password_of(&ana_key, "taken over!"), Err(PasswordError::NotAllowed));
    bia_context.change_own_password("given by ana", "chosen by bia").expect("changed");
    assert!(!bia_context.identity().must_change_password);
}

#[test]
fn the_last_administrator_stays() {
    let world = world();
    let ana = signed_up(&world, "ana");
    let context = context_of(&world, ana);
    let mut space = SecuredObjectSpace::new(Box::new(world.database.space()), context.clone());
    let mut user = space.query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    user.set("active", false);
    space.save(user).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))));
    space.rollback();
    let link = space.query(SECURITY_USER_ROLE, &Query::all()).expect("reads").remove(0);
    space.delete(SECURITY_USER_ROLE, &link.get("id")).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))));
    space.rollback();
    assert!(context.with(Permissions::is_administrator));
}

#[test]
fn losing_a_role_takes_effect_on_every_space_right_away() {
    let world = world();
    let ana = signed_up(&world, "ana");
    let admin = context_of(&world, ana);
    let bia_key = add_user(&world, "bia", "Readers", false);
    let bia = context_of(&world, world.directory.identity_by_key(&bia_key).expect("reads").expect("there"));
    assert!(bia.with(|p| p.can(Read, "Order", None)));

    let mut space = SecuredObjectSpace::new(Box::new(world.database.space()), admin);
    let mut user = space.find(SECURITY_USER, &bia_key).expect("reads").expect("there");
    user.set("active", false);
    space.save(user).expect("waits");
    space.commit().expect("committed");
    bia.reload().expect("reads again");
    assert!(!bia.with(|p| p.can(Read, "Order", None)));
}

#[test]
fn permissions_kept_as_records_are_what_a_person_gets() {
    let world = world();
    let key = add_user(&world, "bia", "Readers", false);
    let mut space = world.database.space();
    let role = space
        .query(SECURITY_ROLE, &Query::all())
        .expect("reads")
        .into_iter()
        .find(|role| role.get("name") == Value::Text("Readers".into()))
        .expect("there");
    let mut member = space.new_object(ravenrs_app::SECURITY_MEMBER_PERMISSION).expect("member");
    member.set("role_id", role.get("id"));
    member.set("target_entity", "Order");
    member.set("fields", "cost, owner");
    member.set("can_read", "deny");
    space.save(member).expect("saved");
    let mut object = space.new_object(ravenrs_app::SECURITY_OBJECT_PERMISSION).expect("object");
    object.set("role_id", role.get("id"));
    object.set("target_entity", "Order");
    object.set("criteria", "[id] = 2");
    object.set("can_read", "deny");
    space.save(object).expect("saved");
    let mut navigation = space.new_object(ravenrs_app::SECURITY_NAVIGATION_PERMISSION).expect("navigation");
    navigation.set("role_id", role.get("id"));
    navigation.set("item", "Sales");
    navigation.set("access", "deny");
    space.save(navigation).expect("saved");
    let mut action = space.new_object(ravenrs_app::SECURITY_ACTION_PERMISSION).expect("action");
    action.set("role_id", role.get("id"));
    action.set("action", "Pay");
    action.set("access", "deny");
    space.save(action).expect("saved");
    let mut types = space.new_object(ravenrs_app::SECURITY_TYPE_PERMISSION).expect("type");
    types.set("role_id", role.get("id"));
    types.set("target_entity", "Line");
    types.set("can_read", "deny");
    space.save(types).expect("saved");
    space.commit().expect("committed");

    let identity = world.directory.identity_by_key(&key).expect("reads").expect("there");
    let context = context_of(&world, identity);
    let secured = SecuredObjectSpace::new(Box::new(world.database.space()), context.clone());
    let found = secured.query("Order", &Query::all()).expect("reads");
    assert_eq!(names(&found), [1.0, 3.0]);
    assert_eq!(found[0].get("cost"), Value::Null);
    assert_eq!(found[0].get("owner"), Value::Null);
    assert!(context.with(|p| !p.can_navigate(None, "Sales")));
    assert!(context.with(|p| !p.can_use_action("Pay")));
    assert!(context.with(|p| !p.can(Read, "Line", None)));
}

#[test]
fn roles_declared_in_code_get_a_record_and_add_to_it() {
    let database = MemoryDatabase::new(entities()).expect("entities fit");
    let code = Role::new("Sales").type_permission("Order", [Read], Access::Allow);
    let directory = UserDirectory::new(raw_maker(&database), vec![code.clone()]).expect("roles");
    // Opening again does not repeat it.
    let directory_again = UserDirectory::new(raw_maker(&database), vec![code]).expect("roles");
    let roles = database.space().query(SECURITY_ROLE, &Query::all()).expect("reads");
    assert_eq!(roles.len(), 1);
    let mut space = database.space();
    let mut user = space.new_object(SECURITY_USER).expect("user");
    user.set("name", "bia");
    let key = user.get("id");
    space.save(user).expect("saved");
    let mut link = space.new_object(SECURITY_USER_ROLE).expect("link");
    link.set("user_id", key);
    link.set("role_id", roles[0].get("id"));
    space.save(link).expect("saved");
    space.commit().expect("committed");
    let identity = directory_again.identity_of("bia").expect("reads").expect("there");
    let all = permissions(identity.roles, RoleMerge::AnyRole);
    assert!(all.can(Read, "Order", None));
    assert!(directory.identity_of("nobody").expect("reads").is_none());
}

fn audited(world: &World, settings: AuditSettings) -> (AuditedObjectSpace, Rc<AuditTrail>) {
    let trail = Rc::new(AuditTrail::new(settings, || "ana".to_owned()));
    (AuditedObjectSpace::new(Box::new(world.database.space()), trail.clone()), trail)
}

fn trail_of(world: &World) -> Vec<(String, String, String, Value, Value)> {
    world
        .database
        .space()
        .query(SECURITY_AUDIT_ENTRY, &Query::all())
        .expect("reads")
        .iter()
        .map(|entry| {
            let text = |name: &str| match entry.get(name) {
                Value::Text(text) => text,
                _ => String::new(),
            };
            (
                text("change"),
                text("target_entity"),
                text("field"),
                entry.get("old_value"),
                entry.get("new_value"),
            )
        })
        .collect()
}

fn count(lines: &[(String, String, String, Value, Value)], change: &str) -> usize {
    lines.iter().filter(|line| line.0 == change).count()
}

#[test]
fn a_full_audit_says_who_changed_what_from_what_to_what() {
    let world = world();
    let (mut space, _) = audited(&world, AuditSettings::new(AuditMode::Full));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "changed");
    assert_eq!(lines[0].2, "name");
    assert_eq!(lines[0].3, Value::Text("Order 1".into()));
    assert_eq!(lines[0].4, Value::Text("Renamed".into()));
    let entry = world.database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads").remove(0);
    assert_eq!(entry.get("author"), Value::Text("ana".into()));
    assert!(matches!(entry.get("happened_at"), Value::Date(_)));

    space.delete("Order", &Value::Number(1.0)).expect("waits");
    space.commit().expect("committed");
    assert_eq!(count(&trail_of(&world), "deleted"), 1);

    let mut fresh = space.new_object("Order").expect("new");
    fresh.set("id", 9.0);
    fresh.set("name", "Nine");
    space.insert(fresh).expect("waits");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(count(&lines, "created"), 1);
    assert_eq!(count(&lines, "initial_value"), 1);
}

#[test]
fn the_modes_write_less() {
    let world = world();
    let (mut space, _) = audited(&world, AuditSettings::new(AuditMode::Lightweight));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(lines.len(), 1);
    assert_eq!((lines[0].0.as_str(), lines[0].2.as_str()), ("changed", ""));
    assert_eq!(lines[0].4, Value::Null);

    let world2 = self::world();
    let (mut space, _) = audited(&world2, AuditSettings::new(AuditMode::CreationOnly));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    space.delete("Order", &Value::Number(2.0)).expect("waits");
    let mut fresh = space.new_object("Order").expect("new");
    fresh.set("id", 9.0);
    space.insert(fresh).expect("waits");
    space.commit().expect("committed");
    let lines = trail_of(&world2);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "created");
}

#[test]
fn settings_leave_out_fields_and_entities() {
    let world = world();
    let settings =
        AuditSettings::new(AuditMode::Full).without_field("Order", "cost").entity("Line", AuditMode::Off);
    let (mut space, _) = audited(&world, settings);
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("cost", 11.0);
    space.save(record).expect("saved");
    let mut line = space.new_object("Line").expect("line");
    line.set("id", 1.0);
    space.insert(line).expect("waits");
    space.commit().expect("committed");
    assert!(trail_of(&world).is_empty());
}

#[test]
fn a_secret_says_it_changed_and_not_to_what() {
    let world = world();
    let (mut space, _) = audited(&world, AuditSettings::new(AuditMode::Full));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("token", "another secret");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(lines[0].2, "token");
    assert_eq!((lines[0].3.clone(), lines[0].4.clone()), (Value::Null, Value::Null));
}

#[test]
fn changes_in_a_collection_show_on_its_owner() {
    let world = world();
    let (mut space, _) = audited(&world, AuditSettings::new(AuditMode::Full));
    let mut line = space.new_object("Line").expect("line");
    line.set("id", 1.0);
    line.set("order_id", 1.0);
    space.insert(line.clone()).expect("waits");
    space.commit().expect("committed");
    assert_eq!(count(&trail_of(&world), "added_to_collection"), 1);

    line.set("text", "changed");
    space.save(line.clone()).expect("waits");
    space.commit().expect("committed");
    assert_eq!(count(&trail_of(&world), "collection_object_changed"), 1);

    line.set("order_id", 2.0);
    space.save(line).expect("waits");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(count(&lines, "removed_from_collection"), 1);
    assert_eq!(count(&lines, "added_to_collection"), 2);

    space.delete("Line", &Value::Number(1.0)).expect("waits");
    space.commit().expect("committed");
    assert_eq!(count(&trail_of(&world), "removed_from_collection"), 2);
}

#[test]
fn a_commit_that_fails_leaves_no_trail_and_can_be_tried_again() {
    let world = world();
    let (mut space, _) = audited(&world, AuditSettings::new(AuditMode::Full));
    let refuse = Rc::new(Cell::new(true));
    let gate = refuse.clone();
    space.on_event(Box::new(move |event| match event {
        ravenrs_app::SpaceEvent::Committing(_) if gate.get() => Err("not now".to_owned()),
        _ => Ok(()),
    }));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    assert!(space.commit().is_err());
    assert!(trail_of(&world).is_empty());
    assert!(
        !space
            .changes()
            .iter()
            .any(|change| matches!(change, Change::New(o) if o.entity() == SECURITY_AUDIT_ENTRY))
    );
    refuse.set(false);
    space.commit().expect("committed");
    assert_eq!(trail_of(&world).len(), 1);
}

#[test]
fn a_custom_note_goes_with_the_next_commit_and_passwords_leave_only_a_note() {
    let world = world();
    let (mut space, trail) = audited(&world, AuditSettings::new(AuditMode::Full));
    trail.custom("Order", &Value::Number(1.0), "", "Printed");
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(count(&lines, "custom"), 1);

    let world = self::world();
    let identity = signed_up(&world, "ana");
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "ana".to_owned()));
    trail.write_with(raw_maker(&world.database));
    let context = Rc::new(
        SecurityContext::new(identity, world.directory.clone(), entities(), RoleMerge::AnyRole)
            .with_audit_trail(trail),
    );
    context.change_own_password("correct horse", "brand new one").expect("changed");
    let lines = trail_of(&world);
    assert_eq!(count(&lines, "custom"), 1);
    assert!(!format!("{lines:?}").contains("argon2"));
    assert!(!format!("{lines:?}").contains("brand new"));
}

#[test]
fn secured_over_audited_writes_the_truth_of_what_they_did() {
    let world = world();
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "ana".to_owned()));
    let inner = AuditedObjectSpace::new(Box::new(world.database.space()), trail);
    let role = Role::new("r").policy(PermissionPolicy::AllowAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Read, Write],
        Access::Deny,
    );
    let mut space = SecuredObjectSpace::new(Box::new(inner), world.context(vec![role]));
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("name", "Renamed");
    space.save(record).expect("saved");
    space.commit().expect("committed");
    let lines = trail_of(&world);
    assert_eq!(lines.len(), 1, "the field kept back is not a change: {lines:?}");
    assert_eq!(lines[0].2, "name");
}

#[test]
fn what_waits_for_a_commit_does_not_give_away_what_a_save_filled_in() {
    let world = world();
    let role = Role::new("r").policy(PermissionPolicy::AllowAll).member_permission(
        "Order",
        ["cost"],
        None,
        [Read],
        Access::Deny,
    );
    let mut space = world.secured(vec![role]);
    let seen_by_watcher = Rc::new(std::cell::RefCell::new(Vec::new()));
    let keep = seen_by_watcher.clone();
    space.on_event(Box::new(move |event| {
        if let ravenrs_app::SpaceEvent::Committing(changes) = event {
            keep.borrow_mut().extend(changes.to_vec());
        }
        Ok(())
    }));
    let mut found = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    found.set("name", "Renamed");
    space.save(found).expect("waits");
    for change in space.changes() {
        let Change::Updated(object) = change else { panic!("an update") };
        assert_eq!(object.get("cost"), Value::Null);
        assert_eq!(object.get("token"), Value::Null);
        assert_eq!(object.get("name"), Value::Text("Renamed".into()));
    }
    assert!(space.is_modified());
    space.commit().expect("committed");
    let watched = seen_by_watcher.borrow();
    assert!(!watched.is_empty());
    for change in watched.iter() {
        let Change::Updated(object) = change else { panic!("an update") };
        assert_eq!(object.get("cost"), Value::Null);
        assert_eq!(object.get("token"), Value::Null);
    }
    assert_eq!(world.stored(1.0).get("cost"), Value::Number(10.0));
    assert_eq!(world.stored(1.0).get("token"), Value::Text("top-secret".into()));
}

#[test]
fn saving_nothing_new_writes_nothing_so_it_cannot_bring_back_an_old_record() {
    let world = world();
    let identity = signed_up(&world, "ana");
    let user_key = identity.key.clone();
    add_user(&world, "bia", "Readers", false);
    let bia = world.directory.identity_of("bia").expect("reads").expect("there");
    // A reader holds on to a record with no permission to write, and saves it as it was.
    let mut reader = SecuredObjectSpace::new(Box::new(world.database.space()), context_of(&world, bia));
    let order = reader.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    reader.save(order).expect("nothing to refuse");
    assert!(reader.changes().is_empty());
    // An administrator's own copy with nothing changed does not queue the whole user either.
    let mut admin = SecuredObjectSpace::new(Box::new(world.database.space()), context_of(&world, identity));
    let user = admin.find(SECURITY_USER, &user_key).expect("reads").expect("there");
    admin.save(user).expect("nothing to do");
    assert!(admin.changes().is_empty());
}

#[test]
fn lines_of_an_owner_decided_record_by_record_stay_closed() {
    let world = world();
    let closed = Role::new("r").type_permission("Line", [Read], Access::Allow).object_permission(
        "Line",
        "[text] = 'secret'",
        [Read],
        Access::Deny,
    );
    let all = permissions(vec![closed], RoleMerge::AnyRole);
    // Notes hang on lines whose reading depends on the record, so they say nothing by themselves.
    assert!(!all.can(Read, "Note", None));
    let open = Role::new("r")
        .type_permission("Line", [Read], Access::Allow)
        .object_permission("Line", "[text] = 'secret'", [Read], Access::Deny)
        .type_permission("Note", [Read], Access::Allow);
    assert!(permissions(vec![open], RoleMerge::AnyRole).can(Read, "Note", None));
    drop(world);
}

#[test]
fn a_denial_whose_expression_fails_on_a_record_still_denies() {
    let world = world();
    let record = world.stored(1.0);
    let denies = Role::new("r").policy(PermissionPolicy::AllowAll).object_permission(
        "Order",
        "NoSuchFunction([cost]) > 0",
        [Read],
        Access::Deny,
    );
    assert!(!permissions(vec![denies], RoleMerge::AnyRole).can(Read, "Order", Some(&record)));
    let allows =
        Role::new("r").object_permission("Order", "NoSuchFunction([cost]) > 0", [Read], Access::Allow);
    assert!(!permissions(vec![allows], RoleMerge::AnyRole).can(Read, "Order", Some(&record)));
}

#[test]
fn a_rule_about_a_calculated_field_reads_what_it_really_comes_to() {
    let world = world();
    let role = Role::new("r").policy(PermissionPolicy::ReadOnlyAll).object_permission(
        "Order",
        "[double_cost] < 100",
        [Write],
        Access::Allow,
    );
    let mut space = world.secured(vec![role]);
    let mut record = space.find("Order", &Value::Number(1.0)).expect("reads").expect("there");
    // The old total goes along, as if the rule could be told what it comes to.
    record.set("cost", 500.0);
    denied(space.save(record));
    assert_eq!(world.stored(1.0).get("cost"), Value::Number(10.0));
}

#[test]
fn someone_turned_off_cannot_change_their_password() {
    let world = world();
    let identity = signed_up(&world, "ana");
    let context = context_of(&world, identity);
    let mut space = world.database.space();
    let mut user = space.query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    user.set("active", false);
    space.save(user).expect("saved");
    space.commit().expect("committed");
    assert_eq!(context.change_own_password("correct horse", "brand new one"), Err(PasswordError::NotAllowed));
}
