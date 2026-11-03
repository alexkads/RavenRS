//! Who may do what: roles, permissions and how they are decided.
//!
//! Everything here is enforced by [`SecuredObjectSpace`], under the screens, so a call with no
//! screen around it gets the same answer. What the screens hide on top of that is only courtesy.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Environment, Scope, Value};

use crate::{EntityInfo, FieldKind, Object};

mod audit;
mod directory;
mod password;
mod records;
mod space;

pub use audit::{AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace};
pub use directory::{
    AuthError, Authenticator, Credentials, MIN_PASSWORD_CHARS, PasswordError, SecurityContext, SpaceMaker,
    UserDirectory,
};
pub use password::{hash_password, verify_password};
pub use records::{
    ADMINISTRATORS_ROLE, SECURITY_ACTION_PERMISSION, SECURITY_API_KEY, SECURITY_AUDIT_ENTRY,
    SECURITY_MEMBER_PERMISSION, SECURITY_NAVIGATION_PERMISSION, SECURITY_OBJECT_PERMISSION, SECURITY_ROLE,
    SECURITY_TYPE_PERMISSION, SECURITY_USER, SECURITY_USER_ROLE, audit_entity, security_entities,
};
pub use space::SecuredObjectSpace;

/// What can be done to a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Operation {
    /// See it.
    Read,
    /// Change it.
    Write,
    /// Make a new one. A new record is also written, so it takes [`Write`](Self::Write) too.
    Create,
    /// Take it away.
    Delete,
}

/// What a permission says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// It can be done.
    Allow,
    /// It cannot.
    Deny,
}

/// What a role says about what no permission mentions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermissionPolicy {
    /// Nothing can be done.
    #[default]
    DenyAll,
    /// Everything can be seen, and nothing changed.
    ReadOnlyAll,
    /// Everything can be done.
    AllowAll,
}

impl PermissionPolicy {
    fn allows(self, operation: Operation) -> bool {
        match self {
            Self::DenyAll => false,
            Self::ReadOnlyAll => operation == Operation::Read,
            Self::AllowAll => true,
        }
    }
}

/// How the answers of several roles of one person come together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoleMerge {
    /// One role that allows it is enough. Each role is asked apart, so what one denies does not
    /// take away what another allows.
    #[default]
    AnyRole,
    /// Every role has to allow it.
    AllRoles,
}

/// An expression a permission depends on. One that does not read, or that fails on a record, never
/// allows and always denies, so a mistake in it cannot open anything.
#[derive(Clone, Debug)]
struct Condition {
    criteria: Option<Criteria>,
}

impl Condition {
    fn new(source: &str) -> Self {
        Self { criteria: Criteria::parse(source).ok() }
    }

    /// Whether it holds for this record, given what is being said.
    fn holds(&self, record: &Object, access: Access, environment: &Environment) -> bool {
        let denies = access == Access::Deny;
        match &self.criteria {
            Some(criteria) => match Scope::new(environment).row(record).evaluate(criteria) {
                Ok(Value::Bool(holds)) => holds,
                _ => denies,
            },
            None => denies,
        }
    }
}

#[derive(Clone, Debug)]
struct TypePermission {
    entity: String,
    operation: Operation,
    access: Access,
}

#[derive(Clone, Debug)]
struct ObjectPermission {
    entity: String,
    condition: Condition,
    operation: Operation,
    access: Access,
}

#[derive(Clone, Debug)]
struct MemberPermission {
    entity: String,
    fields: Vec<String>,
    condition: Option<Condition>,
    operation: Operation,
    access: Access,
}

/// A set of permissions a person can have.
#[derive(Clone, Debug, Default)]
pub struct Role {
    name: String,
    policy: PermissionPolicy,
    administrative: bool,
    edit_model: bool,
    types: Vec<TypePermission>,
    objects: Vec<ObjectPermission>,
    members: Vec<MemberPermission>,
    navigation: BTreeMap<String, Access>,
    actions: BTreeMap<String, Access>,
}

impl Role {
    /// A role with this name, which allows nothing yet.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What it says about what no permission mentions.
    pub fn policy(mut self, policy: PermissionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// It can do everything, and no permission takes anything away from it.
    pub fn administrative(mut self) -> Self {
        self.administrative = true;
        self.edit_model = true;
        self
    }

    /// Whether it can do everything.
    pub fn is_administrative(&self) -> bool {
        self.administrative
    }

    /// It can change the application's model while it runs.
    pub fn can_edit_model(mut self) -> Self {
        self.edit_model = true;
        self
    }

    /// Says this about these operations on every record of an entity.
    pub fn type_permission(
        mut self,
        entity: impl Into<String>,
        operations: impl IntoIterator<Item = Operation>,
        access: Access,
    ) -> Self {
        let entity = entity.into();
        self.types.extend(operations.into_iter().map(|operation| TypePermission {
            entity: entity.clone(),
            operation,
            access,
        }));
        self
    }

    /// Says this about these operations on the records an expression holds for. `?CurrentUserId`
    /// and `?CurrentUserName` are the person. It comes before what is said about the whole entity.
    pub fn object_permission(
        mut self,
        entity: impl Into<String>,
        criteria: &str,
        operations: impl IntoIterator<Item = Operation>,
        access: Access,
    ) -> Self {
        let entity = entity.into();
        self.objects.extend(operations.into_iter().map(|operation| ObjectPermission {
            entity: entity.clone(),
            condition: Condition::new(criteria),
            operation,
            access,
        }));
        self
    }

    /// Says this about reading or writing these fields, on every record or, with an expression,
    /// on those it holds for. It comes before everything else, the expression first.
    pub fn member_permission(
        mut self,
        entity: impl Into<String>,
        fields: impl IntoIterator<Item = impl Into<String>>,
        criteria: Option<&str>,
        operations: impl IntoIterator<Item = Operation>,
        access: Access,
    ) -> Self {
        let entity = entity.into();
        let fields: Vec<String> = fields.into_iter().map(Into::into).collect();
        self.members.extend(operations.into_iter().map(|operation| MemberPermission {
            entity: entity.clone(),
            fields: fields.clone(),
            condition: criteria.map(Condition::new),
            operation,
            access,
        }));
        self
    }

    /// Whether this navigation item or group shows, by its id. An item comes before its group. It
    /// only hides: what protects records are the other permissions.
    pub fn navigation_permission(mut self, item: impl Into<String>, access: Access) -> Self {
        self.navigation.insert(item.into(), access);
        self
    }

    /// Whether this action exists for it, by its id. A denied one goes away, whatever else says.
    pub fn action_permission(mut self, action: impl Into<String>, access: Access) -> Self {
        self.actions.insert(action.into(), access);
        self
    }

    fn type_says(&self, operation: Operation, entity: &str) -> Option<Access> {
        strongest(
            self.types.iter().filter(|p| p.operation == operation && p.entity == entity).map(|p| p.access),
        )
    }

    /// What it says about the records of this entity. With no record, an allowance that depends on
    /// one counts, as something might fit it; a denial that depends on one does not.
    fn objects_say(
        &self,
        operation: Operation,
        entity: &str,
        record: Option<&Object>,
        environment: &Environment,
    ) -> Option<Access> {
        strongest(
            self.objects
                .iter()
                .filter(|p| p.operation == operation && p.entity == entity)
                .filter(|p| match record {
                    Some(record) => p.condition.holds(record, p.access, environment),
                    None => p.access == Access::Allow && p.condition.criteria.is_some(),
                })
                .map(|p| p.access),
        )
    }

    fn members_say(
        &self,
        operation: Operation,
        entity: &str,
        field: &str,
        record: Option<&Object>,
        environment: &Environment,
    ) -> Option<Access> {
        let about = || {
            self.members
                .iter()
                .filter(|p| p.operation == operation && p.entity == entity)
                .filter(|p| p.fields.iter().any(|name| name == field))
        };
        let by_criteria = strongest(
            about()
                .filter_map(|p| p.condition.as_ref().map(|condition| (p, condition)))
                .filter(|(p, condition)| match record {
                    Some(record) => condition.holds(record, p.access, environment),
                    None => p.access == Access::Allow && condition.criteria.is_some(),
                })
                .map(|(p, _)| p.access),
        );
        by_criteria.or_else(|| strongest(about().filter(|p| p.condition.is_none()).map(|p| p.access)))
    }

    /// What it says about the records of an aggregated collection: they belong to their owner, so
    /// what it says about the owner, or about the collection, goes for them. Where that depends on
    /// which owner, it cannot be told from here, so they stay closed unless said otherwise.
    fn owner_says(&self, operation: Operation, entity: &str, entities: &[EntityInfo]) -> Option<Access> {
        let asked = if operation == Operation::Read { Operation::Read } else { Operation::Write };
        strongest(entities.iter().flat_map(|owner| {
            owner
                .fields
                .iter()
                .filter(move |field| {
                    field.aggregated
                        && matches!(&field.kind, FieldKind::Collection(target) if target == entity)
                })
                .filter_map(move |field| {
                    let conditional =
                        self.objects.iter().any(|p| p.entity == owner.name && p.operation == asked)
                            || self.members.iter().any(|p| {
                                p.operation == asked
                                    && p.entity == owner.name
                                    && p.condition.is_some()
                                    && p.fields.contains(&field.name)
                            });
                    if conditional {
                        return Some(Access::Deny);
                    }
                    strongest(
                        self.members
                            .iter()
                            .filter(|p| {
                                p.operation == asked
                                    && p.entity == owner.name
                                    && p.condition.is_none()
                                    && p.fields.contains(&field.name)
                            })
                            .map(|p| p.access),
                    )
                    .or_else(|| self.type_says(asked, &owner.name))
                })
        }))
    }

    fn can(
        &self,
        operation: Operation,
        entity: &str,
        record: Option<&Object>,
        entities: &[EntityInfo],
        environment: &Environment,
    ) -> bool {
        if self.administrative {
            return true;
        }
        let said = self
            .objects_say(operation, entity, record, environment)
            .or_else(|| self.type_says(operation, entity))
            .or_else(|| self.owner_says(operation, entity, entities));
        said.map_or_else(|| self.policy.allows(operation), |access| access == Access::Allow)
    }

    fn can_field(
        &self,
        operation: Operation,
        entity: &str,
        field: &str,
        record: Option<&Object>,
        entities: &[EntityInfo],
        environment: &Environment,
    ) -> bool {
        if self.administrative {
            return true;
        }
        match self.members_say(operation, entity, field, record, environment) {
            Some(access) => access == Access::Allow,
            None => self.can(operation, entity, record, entities, environment),
        }
    }

    fn depends_on_record(&self, entity: &str) -> bool {
        !self.administrative
            && (self.objects.iter().any(|p| p.entity == entity)
                || self.members.iter().any(|p| p.entity == entity && p.condition.is_some()))
    }

    /// Merges what a role in code says with what its record says. Only ever adds to the record.
    fn merged_with(mut self, code: &Role) -> Self {
        self.administrative |= code.administrative;
        self.edit_model |= code.edit_model;
        self.types.extend(code.types.iter().cloned());
        self.objects.extend(code.objects.iter().cloned());
        self.members.extend(code.members.iter().cloned());
        for (item, access) in &code.navigation {
            self.navigation.entry(item.clone()).or_insert(*access);
        }
        for (action, access) in &code.actions {
            self.actions.entry(action.clone()).or_insert(*access);
        }
        if self.policy == PermissionPolicy::DenyAll {
            self.policy = code.policy;
        }
        self
    }
}

/// A denial beats an allowance said at the same level.
fn strongest(said: impl Iterator<Item = Access>) -> Option<Access> {
    let mut found = None;
    for access in said {
        if access == Access::Deny {
            return Some(Access::Deny);
        }
        found = Some(access);
    }
    found
}

/// Who is using the application.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Identity {
    /// The key of their record, for `?CurrentUserId`.
    pub key: Value,
    /// Their name, for `?CurrentUserName` and the audit trail.
    pub name: String,
    /// Their roles.
    pub roles: Vec<Role>,
    /// They have to choose a new password before anything else.
    pub must_change_password: bool,
}

impl Identity {
    /// Someone with this name and these roles, as another way of signing in would say.
    pub fn new(key: impl Into<Value>, name: impl Into<String>, roles: Vec<Role>) -> Self {
        Self { key: key.into(), name: name.into(), roles, must_change_password: false }
    }
}

/// What one person can do: the answers of their roles, brought together.
#[derive(Debug)]
pub struct Permissions {
    identity: Identity,
    merge: RoleMerge,
    entities: Rc<Vec<EntityInfo>>,
    environment: Environment,
}

impl Permissions {
    /// What this person can do over these entities.
    pub fn new(identity: Identity, entities: Vec<EntityInfo>, merge: RoleMerge) -> Self {
        let environment = crate::environment()
            .parameter("CurrentUserId", identity.key.clone())
            .parameter("CurrentUserName", identity.name.clone());
        Self { identity, merge, entities: Rc::new(entities), environment }
    }

    /// Who this is about.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// What expressions about this person are evaluated with.
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub(crate) fn entity(&self, name: &str) -> Option<&EntityInfo> {
        self.entities.iter().find(|entity| entity.name == name)
    }

    fn agreed(&self, ask: impl Fn(&Role) -> bool) -> bool {
        let roles = &self.identity.roles;
        match self.merge {
            RoleMerge::AnyRole => roles.iter().any(ask),
            RoleMerge::AllRoles => !roles.is_empty() && roles.iter().all(ask),
        }
    }

    /// Whether they can do everything.
    pub fn is_administrator(&self) -> bool {
        self.identity.roles.iter().any(|role| role.administrative)
    }

    /// Whether they can change the application's model while it runs.
    pub fn can_edit_model(&self) -> bool {
        self.identity.roles.iter().any(|role| role.edit_model)
    }

    /// What holds whatever roles say: who manages people decides who can do what, so only an
    /// administrator reaches those records, and the audit trail can only be read.
    fn built_in(&self, operation: Operation, entity: &str) -> Option<bool> {
        if entity == SECURITY_AUDIT_ENTRY {
            return Some(operation == Operation::Read && self.is_administrator());
        }
        records::is_security_entity(entity).then(|| self.is_administrator())
    }

    /// Whether they can do this to this record or, with none, to some record of the entity.
    pub fn can(&self, operation: Operation, entity: &str, record: Option<&Object>) -> bool {
        if let Some(answer) = self.built_in(operation, entity) {
            return answer;
        }
        self.agreed(|role| role.can(operation, entity, record, &self.entities, &self.environment))
    }

    /// Whether they can read or write this field of this record or, with none, of some record.
    pub fn can_field(
        &self,
        operation: Operation,
        entity: &str,
        field: &str,
        record: Option<&Object>,
    ) -> bool {
        self.can_field_within(operation, entity, field, record, 0)
    }

    fn can_field_within(
        &self,
        operation: Operation,
        entity: &str,
        field: &str,
        record: Option<&Object>,
        depth: usize,
    ) -> bool {
        if let Some(answer) = self.built_in(operation, entity)
            && !answer
        {
            return false;
        }
        let info = self.entity(entity).and_then(|info| info.field_named(field));
        if info.is_some_and(|info| info.secret) {
            return false;
        }
        let allowed = self.agreed(|role| {
            role.can_field(operation, entity, field, record, &self.entities, &self.environment)
        });
        if !allowed {
            return false;
        }
        // A calculated value says what it is made of, so it takes being able to read all of that.
        match info.and_then(|info| info.calculated.as_deref()) {
            Some(expression) if operation == Operation::Read => {
                depth < 8
                    && Criteria::parse(expression).is_ok_and(|criteria| {
                        criteria.properties().iter().all(|path| {
                            let head = path.split('.').next().unwrap_or(path).trim_start_matches('^');
                            head == field
                                || self.can_field_within(Operation::Read, entity, head, record, depth + 1)
                        })
                    })
            }
            _ => true,
        }
    }

    /// Whether what they can do with this entity changes from record to record.
    pub(crate) fn depends_on_record(&self, entity: &str) -> bool {
        self.identity.roles.iter().any(|role| role.depends_on_record(entity))
    }

    /// Whether this navigation item shows, inside this group, if it has one. An item comes before
    /// its group, and what no role mentions shows.
    pub fn can_navigate(&self, group: Option<&str>, item: &str) -> bool {
        if self.identity.roles.iter().any(|role| role.administrative) {
            return true;
        }
        self.agreed(|role| {
            let said =
                role.navigation.get(item).or_else(|| group.and_then(|group| role.navigation.get(group)));
            said != Some(&Access::Deny)
        })
    }

    /// Whether this action exists for them. Any role that denies it takes it away.
    pub fn can_use_action(&self, action: &str) -> bool {
        if self.is_administrator() {
            return true;
        }
        !self.identity.roles.iter().any(|role| role.actions.get(action) == Some(&Access::Deny))
    }
}
