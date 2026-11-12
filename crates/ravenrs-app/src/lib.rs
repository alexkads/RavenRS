//! The model of a business application, with no GUI: entities, a layered application
//! model, object spaces, actions, controllers, frames and modules.
//!
//! ```
//! use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ModelLayers, ModelLayer, ObjectSpace, generate_model};
//!
//! let account = EntityInfo::new("Account")
//!     .caption("Conta")
//!     .navigation("Cadastros")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property());
//!
//! // The model: a list, a form and a place in the navigation, changed by a layer.
//! let mut layers = ModelLayers::new(generate_model(std::slice::from_ref(&account))?);
//! layers.set(&ModelLayer::User, &["NavigationItems", "Cadastros"], "Caption", "Registros".into());
//! let model = layers.merged();
//! assert_eq!(model.root.at(&["NavigationItems", "Cadastros"]).and_then(|n| n.text("Caption")), Some("Registros"));
//!
//! // The records: nothing is kept until the commit.
//! let mut space = MemoryDatabase::new(vec![account])?.space();
//! let mut object = space.new_object("Account")?;
//! object.set("name", "Corrente");
//! space.save(object)?;
//! assert!(space.is_modified());
//! space.commit()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod action;
mod appearance;
mod controller;
mod entity;
mod files;
mod frame;
mod functions;
mod job;
mod json;
#[cfg(feature = "live")]
mod live_list;
mod manifest;
mod model;
mod module;
mod object;
mod outbox;
mod provider;
mod record_action;
mod records;
mod security;
mod settings;
#[cfg(feature = "source")]
mod source;
mod space;
mod validation;
mod workflow;

pub use action::{
    Action, ActionCategory, ActionContext, ActionError, ActionInput, ActionKind, ActionResult, ChoiceItem,
    CriteriaMode, Message, MessageKind, Nesting, REASON_APPEARANCE, REASON_CONTROLLER, REASON_CRITERIA,
    REASON_RUNNING, REASON_SCOPE, REASON_SECURITY, REASON_SELECTION, Reasons, Selection, TargetScope,
};
pub use appearance::{Appearance, AppearanceContext, AppearanceRule, AppearanceTarget, Tint};
pub use controller::{ViewController, WindowController};
pub use entity::{
    DataAccess, EntityInfo, EnumOption, FieldInfo, FieldKind, ListFilter, ModelError, Object, ViewVariant,
};
pub use files::{AttachmentStore, MemoryAttachmentStore};
pub use frame::{Frame, FrameContext, FrameEvent, ViewInfo, ViewKind};
pub use functions::{environment, register_function};
pub use job::{Job, JobChange, JobContext, JobWaker, JobWork};
pub use json::{object_from_json, value_to_json};
pub use manifest::{GuardedObjectSpace, ModuleManifest, Target, Version, VersionRange};
#[cfg(not(target_arch = "wasm32"))]
pub use model::FileStore;
pub use model::{
    Diff, EmbeddedStore, MemoryModelStore, MergedModel, ModelLayer, ModelLayers, ModelNode, ModelStore,
    StoredModel, UnusableNode, check_model, generate_model,
};
pub use module::{
    Application, ApplicationBuilder, DashboardContent, DashboardInfo, DashboardItem, Module, Notice,
    NoticeKind,
};
pub use object::{BusinessObject, FieldValue, ObjectSpaceExt};
pub use outbox::{Dispatcher, Emitted, Event, OUTBOX_EVENT, OutboxModule, Report, outbox_entity};
pub use provider::{CAN_TELL_STATUS, EventSink, Provider};
pub use ravenrs_app_derive::{BusinessEnum, BusinessObject};
#[cfg(not(target_arch = "wasm32"))]
pub use ravenrs_core::FileStorage;
pub use ravenrs_core::{MemoryStorage, Storage};
pub use ravenrs_criteria::{Criteria, Value};
pub use record_action::RecordAction;
pub use records::{
    AppRecordsModule, DRAFT_ENTITY, Draft, SEQUENCE_ENTITY, draft_entity, drafts, next_number, save_draft,
    sequence_entity, take_draft,
};
pub use security::{
    ADMINISTRATORS_ROLE, Access, AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace, AuthError,
    Authenticator, Claims, Credentials, ExternalSignIn, Identity, IdentityProvider, MIN_PASSWORD_CHARS,
    Operation, PasswordError, PermissionPolicy, Permissions, Role, RoleMerge, SECURITY_ACTION_PERMISSION,
    SECURITY_API_KEY, SECURITY_AUDIT_ENTRY, SECURITY_EXTERNAL_LOGIN, SECURITY_MEMBER_PERMISSION,
    SECURITY_NAVIGATION_PERMISSION, SECURITY_OBJECT_PERMISSION, SECURITY_ROLE, SECURITY_TYPE_PERMISSION,
    SECURITY_USER, SECURITY_USER_ROLE, SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory,
    audit_entity, hash_password, security_entities, verify_password,
};
pub use settings::{
    APP_SETTING, MemorySettings, RecordSettings, SettingScope, SettingsStore, settings_entity,
};
#[cfg(feature = "source")]
pub use source::{SourceError, entities_from_source, entities_from_spec};
pub use space::{
    Change, ChangeSet, MemoryDatabase, MemoryObjectSpace, Merged, ObjectSpace, Query, SpaceError, SpaceEvent,
    SpaceWatcher, check_query, key_of, key_text, merge_changes, run_query,
};
pub use validation::{
    CONTEXT_DELETE, CONTEXT_SAVE, Comparison, Operand, Rule, RuleKind, RuleWords, Severity, TextComparison,
    Violation, validate, validate_in_place,
};
pub use workflow::{Reminder, StateMachine, Transition, check_transitions, local_now, reminders_of};

/// Registers this crate's types with a playground's interpreter (feature `live`).
#[cfg(feature = "live")]
pub fn live_expose(registry: &mut ravenrs_live::Registry) {
    live_list::expose_all(registry);
}
