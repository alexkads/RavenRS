//! Users, roles and permissions as records, so administrators manage them on generated screens.

use ravenrs_criteria::Value;

use super::{Access, Operation, PermissionPolicy, Role};
use crate::{EntityInfo, EnumOption, FieldInfo, FieldKind, ObjectSpace, Query, SpaceError};

/// The entity of the people who can sign in.
pub const SECURITY_USER: &str = "SecurityUser";
/// The entity of roles.
pub const SECURITY_ROLE: &str = "SecurityRole";
/// The entity that says who has which role.
pub const SECURITY_USER_ROLE: &str = "SecurityUserRole";
/// What a role says about a whole entity.
pub const SECURITY_TYPE_PERMISSION: &str = "SecurityTypePermission";
/// What a role says about the records an expression holds for.
pub const SECURITY_OBJECT_PERMISSION: &str = "SecurityObjectPermission";
/// What a role says about fields.
pub const SECURITY_MEMBER_PERMISSION: &str = "SecurityMemberPermission";
/// What a role says about a navigation item.
pub const SECURITY_NAVIGATION_PERMISSION: &str = "SecurityNavigationPermission";
/// What a role says about an action.
pub const SECURITY_ACTION_PERMISSION: &str = "SecurityActionPermission";
/// The entity of the audit trail.
pub const SECURITY_AUDIT_ENTRY: &str = "SecurityAuditEntry";
/// The role the first person gets.
pub const ADMINISTRATORS_ROLE: &str = "Administrators";

const GROUP: &str = "Administration";

pub(crate) fn is_security_entity(name: &str) -> bool {
    [
        SECURITY_USER,
        SECURITY_ROLE,
        SECURITY_USER_ROLE,
        SECURITY_TYPE_PERMISSION,
        SECURITY_OBJECT_PERMISSION,
        SECURITY_MEMBER_PERMISSION,
        SECURITY_NAVIGATION_PERMISSION,
        SECURITY_ACTION_PERMISSION,
    ]
    .contains(&name)
}

fn key() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Guid).key()
}

fn access_kind() -> FieldKind {
    FieldKind::Enum(vec![
        EnumOption::new("", "Not set"),
        EnumOption::new("allow", "Allow"),
        EnumOption::new("deny", "Deny"),
    ])
}

fn owned_list(name: &str, caption: &str, target: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Collection(target.to_owned()))
        .caption(caption)
        .back_reference("role_id")
        .aggregated()
}

fn of_role(name: &str) -> EntityInfo {
    EntityInfo::new(name)
        .field(key())
        .field(FieldInfo::new("role_id", FieldKind::Reference(SECURITY_ROLE.to_owned())).caption("Role"))
}

/// The entities of users, roles and permissions, which storage has to know.
pub fn security_entities() -> Vec<EntityInfo> {
    let operations = |entity: EntityInfo| {
        ["can_read", "can_write", "can_create", "can_delete"]
            .iter()
            .fold(entity, |entity, name| entity.field(FieldInfo::new(*name, access_kind())))
    };
    vec![
        EntityInfo::new(SECURITY_USER)
            .caption("Users")
            .navigation(GROUP)
            .icon("users")
            .field(key())
            .field(FieldInfo::new("name", FieldKind::Text).default_property())
            .field(FieldInfo::new("active", FieldKind::Boolean).default_value(true))
            .field(FieldInfo::new("must_change_password", FieldKind::Boolean).default_value(true))
            .field(FieldInfo::new("password_hash", FieldKind::Text).secret())
            .field(
                FieldInfo::new("failed_attempts", FieldKind::Integer)
                    .default_value(0.0)
                    .read_only()
                    .in_list(false)
                    .in_detail(false),
            )
            .field(FieldInfo::new("locked_until", FieldKind::DateTime).read_only().in_list(false))
            .field(
                FieldInfo::new("roles", FieldKind::Collection(SECURITY_USER_ROLE.to_owned()))
                    .back_reference("user_id"),
            )
            .rule(crate::Rule::required("name"))
            .rule(crate::Rule::unique("name")),
        EntityInfo::new(SECURITY_USER_ROLE)
            .caption("Roles of a user")
            .field(key())
            .field(FieldInfo::new("user_id", FieldKind::Reference(SECURITY_USER.to_owned())).caption("User"))
            .field(FieldInfo::new("role_id", FieldKind::Reference(SECURITY_ROLE.to_owned())).caption("Role")),
        EntityInfo::new(SECURITY_ROLE)
            .caption("Roles")
            .navigation(GROUP)
            .icon("shield")
            .field(key())
            .field(FieldInfo::new("name", FieldKind::Text).default_property())
            .field(FieldInfo::new(
                "policy",
                FieldKind::Enum(vec![
                    EnumOption::new("deny_all", "Deny all"),
                    EnumOption::new("read_only_all", "Read only"),
                    EnumOption::new("allow_all", "Allow all"),
                ]),
            ))
            .field(FieldInfo::new("administrative", FieldKind::Boolean).default_value(false))
            .field(FieldInfo::new("edit_model", FieldKind::Boolean).default_value(false))
            .field(owned_list("type_permissions", "Entities", SECURITY_TYPE_PERMISSION))
            .field(owned_list("object_permissions", "Records", SECURITY_OBJECT_PERMISSION))
            .field(owned_list("member_permissions", "Fields", SECURITY_MEMBER_PERMISSION))
            .field(owned_list("navigation_permissions", "Navigation", SECURITY_NAVIGATION_PERMISSION))
            .field(owned_list("action_permissions", "Actions", SECURITY_ACTION_PERMISSION))
            .rule(crate::Rule::required("name"))
            .rule(crate::Rule::unique("name")),
        operations(
            of_role(SECURITY_TYPE_PERMISSION)
                .caption("Permissions on entities")
                .field(FieldInfo::new("target_entity", FieldKind::Text).default_property()),
        ),
        operations(
            of_role(SECURITY_OBJECT_PERMISSION)
                .caption("Permissions on records")
                .field(FieldInfo::new("target_entity", FieldKind::Text).default_property())
                .field(FieldInfo::new("criteria", FieldKind::Criteria)),
        ),
        of_role(SECURITY_MEMBER_PERMISSION)
            .caption("Permissions on fields")
            .field(FieldInfo::new("target_entity", FieldKind::Text).default_property())
            .field(FieldInfo::new("fields", FieldKind::Text).caption("Fields, separated by commas"))
            .field(FieldInfo::new("criteria", FieldKind::Criteria))
            .field(FieldInfo::new("can_read", access_kind()))
            .field(FieldInfo::new("can_write", access_kind())),
        of_role(SECURITY_NAVIGATION_PERMISSION)
            .caption("Permissions on navigation")
            .field(FieldInfo::new("item", FieldKind::Text).default_property())
            .field(FieldInfo::new("access", access_kind())),
        of_role(SECURITY_ACTION_PERMISSION)
            .caption("Permissions on actions")
            .field(FieldInfo::new("action", FieldKind::Text).default_property())
            .field(FieldInfo::new("access", access_kind())),
    ]
}

/// The entity of the audit trail, which storage has to know when there is one.
pub fn audit_entity() -> EntityInfo {
    EntityInfo::new(SECURITY_AUDIT_ENTRY)
        .caption("Audit trail")
        .navigation(GROUP)
        .icon("history")
        .field(key())
        .field(FieldInfo::new("happened_at", FieldKind::DateTime).read_only())
        .field(FieldInfo::new("author", FieldKind::Text).read_only())
        .field(FieldInfo::new(
            "change",
            FieldKind::Enum(vec![
                EnumOption::new("created", "Created"),
                EnumOption::new("initial_value", "Initial value"),
                EnumOption::new("changed", "Changed"),
                EnumOption::new("deleted", "Deleted"),
                EnumOption::new("added_to_collection", "Added to a collection"),
                EnumOption::new("removed_from_collection", "Removed from a collection"),
                EnumOption::new("collection_object_changed", "Changed in a collection"),
                EnumOption::new("custom", "Custom"),
            ]),
        ))
        .field(FieldInfo::new("target_entity", FieldKind::Text).read_only().default_property())
        .field(FieldInfo::new("target_key", FieldKind::Text).read_only())
        .field(FieldInfo::new("field", FieldKind::Text).read_only())
        .field(FieldInfo::new("old_value", FieldKind::LongText).read_only())
        .field(FieldInfo::new("new_value", FieldKind::LongText).read_only())
}

fn access_of(value: &Value) -> Option<Access> {
    match value {
        Value::Text(text) if text == "allow" => Some(Access::Allow),
        Value::Text(text) if text == "deny" => Some(Access::Deny),
        _ => None,
    }
}

fn text_of(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

/// The rows of a table that belong to this role.
fn rows_of(space: &dyn ObjectSpace, entity: &str, role: &Value) -> Result<Vec<crate::Object>, SpaceError> {
    Ok(space.query(entity, &Query::all())?.into_iter().filter(|row| row.get("role_id") == *role).collect())
}

/// A role as its records say, with everything it has.
pub(crate) fn role_from_records(space: &dyn ObjectSpace, record: &crate::Object) -> Result<Role, SpaceError> {
    let key = record.get("id");
    let mut role =
        Role::new(text_of(&record.get("name"))).policy(match text_of(&record.get("policy")).as_str() {
            "read_only_all" => PermissionPolicy::ReadOnlyAll,
            "allow_all" => PermissionPolicy::AllowAll,
            _ => PermissionPolicy::DenyAll,
        });
    role.administrative = record.get("administrative") == Value::Bool(true);
    role.edit_model = role.administrative || record.get("edit_model") == Value::Bool(true);
    let operations = [
        ("can_read", Operation::Read),
        ("can_write", Operation::Write),
        ("can_create", Operation::Create),
        ("can_delete", Operation::Delete),
    ];
    for row in rows_of(space, SECURITY_TYPE_PERMISSION, &key)? {
        let entity = text_of(&row.get("target_entity"));
        for (name, operation) in operations {
            if let Some(access) = access_of(&row.get(name)) {
                role = role.type_permission(entity.clone(), [operation], access);
            }
        }
    }
    for row in rows_of(space, SECURITY_OBJECT_PERMISSION, &key)? {
        let (entity, criteria) = (text_of(&row.get("target_entity")), text_of(&row.get("criteria")));
        for (name, operation) in operations {
            if let Some(access) = access_of(&row.get(name)) {
                role = role.object_permission(entity.clone(), &criteria, [operation], access);
            }
        }
    }
    for row in rows_of(space, SECURITY_MEMBER_PERMISSION, &key)? {
        let entity = text_of(&row.get("target_entity"));
        let (fields, criteria) = (text_of(&row.get("fields")), text_of(&row.get("criteria")));
        let names: Vec<String> =
            fields.split(',').map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned).collect();
        let condition = (!criteria.trim().is_empty()).then_some(criteria.as_str());
        for (name, operation) in &operations[..2] {
            if let Some(access) = access_of(&row.get(name)) {
                role = role.member_permission(entity.clone(), names.clone(), condition, [*operation], access);
            }
        }
    }
    for row in rows_of(space, SECURITY_NAVIGATION_PERMISSION, &key)? {
        if let Some(access) = access_of(&row.get("access")) {
            role = role.navigation_permission(text_of(&row.get("item")), access);
        }
    }
    for row in rows_of(space, SECURITY_ACTION_PERMISSION, &key)? {
        if let Some(access) = access_of(&row.get("access")) {
            role = role.action_permission(text_of(&row.get("action")), access);
        }
    }
    Ok(role)
}
