//! The protocol of an external plugin: what its `plugin.toml` says, the methods of the conversation
//! and the JSON shape of what goes in them.
//!
//! # The role
//!
//! An external plugin is a program apart that speaks JSON-RPC 2.0 on its standard input and
//! output, one message per line. The core (`raven-ci-core`, module `external`) is one side; the
//! other is any program, and for a plugin written in Rust, [`serve`](crate::serve) (feature
//! `serve`). Both sides read and write the same types from here, so the two cannot drift apart.
//!
//! # How it works
//!
//! - [`PluginToml`] is the file in the plugin's folder that says who it is and what it offers,
//!   read by the core without starting the program. Each point is declared with its ids
//!   (`[[publishers]] id = "relatorio"`), and what the core needs to know of a point before
//!   asking (the fields of a credential, the method and path of a route) is declared there too.
//! - [`method`] names every method, the ones the core asks and the `host/*` the plugin asks.
//! - [`FieldDecl`] describes a field of a credential, of the configuration or of a parameter of
//!   a pipeline with the vocabulary [`check_settings`](crate::check_settings) understands, and
//!   turns into and out of an [`EntityInfo`].
//! - [`AttachmentWire`], [`PipelineWire`], [`http_request_to_wire`] and the others carry what
//!   has no JSON shape of its own (bytes, a [`Pipeline`]).
//! - [`error_to_wire`] and [`error_from_wire`] carry a [`CiError`] whole, so a refusal of the host
//!   is still a [`CiError::Denied`] on the other side.

use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, Rule, RuleKind, VersionRange};
use ravenrs_flows::FlowDefinition;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    Attachment, CiError, FormatError, HttpRequest, HttpResponse, Permission, Pipeline, PostSteps, PostWhen,
    Use,
};

/// The version of this API, as the core and the plugins built with it say it: the plugin writes it
/// in its `plugin.toml` as `api`, and the core accepts what is compatible with its own.
pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The version of the conversation, sent in `initialize`.
pub const PROTOCOL: u32 = 2;

/// Above this many bytes, [`AttachmentWire::from_attachment`] writes the bytes to a file in the
/// folder of the build rather than in the line.
pub const INLINE_LIMIT: usize = 256 * 1024;

/// The folder, in the folder of the build, where a plugin leaves the attachments too big for a line.
pub const ATTACHMENTS_DIR: &str = ".raven-ci-anexos";

/// The name of every method, as it goes in `"method"`.
pub mod method {
    /// The first call: who the program is.
    pub const INITIALIZE: &str = "initialize";
    /// A notification: end now.
    pub const SHUTDOWN: &str = "shutdown";
    /// A node of a build whose kind is one of its steps.
    pub const STEP_EXECUTE: &str = "step/execute";
    /// A subcommand of `raven-ci`.
    pub const CLI_RUN: &str = "cli/run";
    /// [`Scm::checkout`](crate::Scm::checkout).
    pub const SCM_CHECKOUT: &str = "scm/checkout";
    /// [`BranchSource::discover`](crate::BranchSource::discover).
    pub const BRANCH_DISCOVER: &str = "branch_source/discover";
    /// [`BranchSource::file`](crate::BranchSource::file).
    pub const BRANCH_FILE: &str = "branch_source/file";
    /// [`Publisher::publish`](crate::Publisher::publish).
    pub const PUBLISH: &str = "publisher/publish";
    /// [`StatusReporter::report`](crate::StatusReporter::report).
    pub const STATUS_REPORT: &str = "status/report";
    /// [`Notifier::notify`](crate::Notifier::notify).
    pub const NOTIFY: &str = "notifier/notify";
    /// [`CredentialKind::hidden`](crate::CredentialKind::hidden).
    pub const CREDENTIAL_HIDDEN: &str = "credential/hidden";
    /// [`CredentialKind::secret`](crate::CredentialKind::secret).
    pub const CREDENTIAL_SECRET: &str = "credential/secret";
    /// [`BuildHook::before`](crate::BuildHook::before).
    pub const HOOK_BEFORE: &str = "hook/before";
    /// [`BuildHook::after`](crate::BuildHook::after).
    pub const HOOK_AFTER: &str = "hook/after";
    /// A request to a [`Route`](crate::Route) the plugin declared.
    pub const ROUTE_HANDLE: &str = "route/handle";
    /// [`Trigger::watch`](crate::Trigger::watch).
    pub const TRIGGER_WATCH: &str = "trigger/watch";
    /// [`Trigger::unwatch`](crate::Trigger::unwatch).
    pub const TRIGGER_UNWATCH: &str = "trigger/unwatch";
    /// [`PipelineFormat::read`](crate::PipelineFormat::read).
    pub const FORMAT_READ: &str = "format/read";
    /// [`AgentLauncher::launch`](crate::AgentLauncher::launch).
    pub const AGENT_LAUNCH: &str = "agent/launch";
    /// [`LaunchedAgent::alive`](crate::LaunchedAgent::alive).
    pub const AGENT_ALIVE: &str = "agent/alive";
    /// [`LaunchedAgent::stop`](crate::LaunchedAgent::stop).
    pub const AGENT_STOP: &str = "agent/stop";

    /// [`Host::log`](crate::Host::log), a notification.
    pub const HOST_LOG: &str = "host/log";
    /// [`Host::settings`](crate::Host::settings).
    pub const HOST_SETTINGS: &str = "host/settings";
    /// [`Host::secret`](crate::Host::secret).
    pub const HOST_SECRET: &str = "host/secret";
    /// [`Host::data_dir`](crate::Host::data_dir).
    pub const HOST_DATA_DIR: &str = "host/data_dir";
    /// [`Host::database`](crate::Host::database).
    pub const HOST_DATABASE: &str = "host/database";
    /// [`Host::granted`](crate::Host::granted).
    pub const HOST_GRANTED: &str = "host/granted";
    /// [`Host::require`](crate::Host::require).
    pub const HOST_REQUIRE: &str = "host/require";
    /// A line for the log of the build of a call in progress, a notification with the call's `id`.
    pub const HOST_BUILD_LOG: &str = "host/build_log";
    /// A value the log of the build of a call in progress hides from now on, a notification.
    pub const HOST_HIDE: &str = "host/hide";
    /// [`Builds::request`](crate::Builds::request), for a job a trigger of the plugin watches.
    pub const HOST_REQUEST_BUILD: &str = "host/request_build";
    /// [`Builds::scan`](crate::Builds::scan), for a job a trigger of the plugin watches.
    pub const HOST_SCAN: &str = "host/scan";
    /// [`Host::attachments`](crate::Host::attachments): what builds left, without the bytes.
    pub const HOST_ATTACHMENTS: &str = "host/attachments";
    /// [`Host::attachment`](crate::Host::attachment): one attachment, its bytes in base64.
    pub const HOST_ATTACHMENT: &str = "host/attachment";
}

fn first_version() -> u32 {
    1
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_first(version: &u32) -> bool {
    *version == 1
}

/// A kind of step an external plugin brings: `[[steps]]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepDecl {
    /// What a pipeline names it by.
    pub kind: String,
    /// Its version.
    #[serde(default = "first_version", skip_serializing_if = "is_first")]
    pub version: u32,
    /// What it is called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The parameters a step of this kind must have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

/// A subcommand an external plugin brings: `[[commands]]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDecl {
    /// What follows `raven-ci`.
    pub name: String,
    /// One line for the list of commands.
    #[serde(default)]
    pub summary: String,
}

/// A point known by its id alone: an scm, a branch source, a trigger, a publisher, a status
/// reporter, a notifier, an agent launcher, a build hook.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdDecl {
    /// What a job or a pipeline names it by.
    pub id: String,
}

impl IdDecl {
    /// The point `id`.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// A pipeline format: `[[pipeline_formats]]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatDecl {
    /// What a job names it by.
    pub id: String,
    /// The names of the files it reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_names: Vec<String>,
}

/// An address of the API: `[[routes]]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDecl {
    /// `GET`, `POST`.
    pub method: String,
    /// The path, with parts between braces.
    pub path: String,
    /// Reached with no one signed in, as a webhook is.
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

/// A kind of credential: `[[credential_kinds]]`, with its fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialDecl {
    /// What a credential names its kind by.
    pub id: String,
    /// What people see when they choose the kind.
    pub caption: String,
    /// What a credential of the kind holds.
    #[serde(default)]
    pub fields: Vec<FieldDecl>,
}

/// One option of a field that is a choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionDecl {
    /// What the value holds.
    pub code: String,
    /// What people see.
    #[serde(default)]
    pub caption: String,
}

/// A field, with the vocabulary of [`check_settings`](crate::check_settings): its kind, whether it
/// is secret and the rules a setting has to meet.
///
/// `kind` is one of `text`, `long_text`, `integer`, `decimal`, `money`, `boolean`, `date`,
/// `date_time`, `color`, `criteria` and `enum` (with `options`). The rules are `required`, `min`
/// and `max` (a range) and `pattern` (a regular expression).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDecl {
    /// Its name.
    pub name: String,
    /// Its kind, in words: `text`.
    pub kind: String,
    /// What people see; from the name when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// A value no screen shows and every log hides.
    #[serde(default, skip_serializing_if = "is_false")]
    pub secret: bool,
    /// It has to be filled.
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    /// For `enum`: the options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionDecl>,
    /// For `decimal`: the digits after the point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// The least it may be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The most it may be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// A regular expression it has to match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl FieldDecl {
    /// The field `name` of the kind `kind` (`text`, `boolean`…).
    pub fn new(name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self { name: name.into(), kind: kind.into(), ..Self::default() }
    }

    /// The field of `entity` named `field`, said in this vocabulary with the rules of the entity
    /// about it; what has no word here is said.
    pub fn of(entity: &EntityInfo, field: &FieldInfo) -> Result<Self, String> {
        let kind = match &field.kind {
            FieldKind::Text => "text",
            FieldKind::LongText => "long_text",
            FieldKind::Integer => "integer",
            FieldKind::Decimal(_) => "decimal",
            FieldKind::Money => "money",
            FieldKind::Boolean => "boolean",
            FieldKind::Date => "date",
            FieldKind::DateTime => "date_time",
            FieldKind::Color => "color",
            FieldKind::Criteria => "criteria",
            FieldKind::Enum(_) => "enum",
            other => {
                return Err(format!(
                    "o campo {} de {} é de um tipo ({other:?}) que não cabe numa configuração",
                    field.name, entity.name
                ));
            }
        };
        let mut decl = Self::new(&field.name, kind);
        if field.caption != FieldInfo::new(&field.name, FieldKind::Text).caption {
            decl.caption = Some(field.caption.clone());
        }
        decl.secret = field.secret;
        if let FieldKind::Enum(options) = &field.kind {
            decl.options = options
                .iter()
                .map(|o| OptionDecl { code: o.code.clone(), caption: o.caption.clone() })
                .collect();
        }
        if let FieldKind::Decimal(digits) = field.kind {
            decl.decimals = Some(digits);
        }
        for rule in entity.rules.iter().filter(|rule| rule.field.as_deref() == Some(field.name.as_str())) {
            match &rule.kind {
                RuleKind::Required => decl.required = true,
                RuleKind::Range { min, max } => {
                    decl.min = *min;
                    decl.max = *max;
                }
                RuleKind::Matches(pattern) => decl.pattern = Some(pattern.clone()),
                other => {
                    return Err(format!(
                        "a regra {} do campo {} de {} ({}) não cabe num plugin externo: use required, um intervalo ou um padrão",
                        rule.id,
                        field.name,
                        entity.name,
                        rule_name(other)
                    ));
                }
            }
        }
        Ok(decl)
    }

    /// The field it describes, for an entity of `ravenrs-app`; a kind it does not know is said.
    pub fn field(&self) -> Result<FieldInfo, String> {
        let kind = match self.kind.as_str() {
            "text" => FieldKind::Text,
            "long_text" => FieldKind::LongText,
            "integer" => FieldKind::Integer,
            "decimal" => FieldKind::Decimal(self.decimals.unwrap_or(2)),
            "money" => FieldKind::Money,
            "boolean" => FieldKind::Boolean,
            "date" => FieldKind::Date,
            "date_time" => FieldKind::DateTime,
            "color" => FieldKind::Color,
            "criteria" => FieldKind::Criteria,
            "enum" => FieldKind::Enum(
                self.options
                    .iter()
                    .map(|o| {
                        EnumOption::new(&o.code, if o.caption.is_empty() { &o.code } else { &o.caption })
                    })
                    .collect(),
            ),
            other => {
                return Err(format!(
                    "o campo {} é do tipo {other:?}, que não existe; os que há são text, long_text, integer, decimal, money, boolean, date, date_time, color, criteria e enum",
                    self.name
                ));
            }
        };
        let mut field = FieldInfo::new(&self.name, kind);
        if let Some(caption) = &self.caption {
            field = field.caption(caption);
        }
        if self.secret {
            field = field.secret();
        }
        Ok(field)
    }

    /// Its rules, as an entity holds them.
    pub fn rules(&self) -> Vec<Rule> {
        let mut rules = Vec::new();
        if self.required {
            rules.push(Rule::required(&self.name));
        }
        if self.min.is_some() || self.max.is_some() {
            rules.push(Rule::range(&self.name, self.min, self.max));
        }
        if let Some(pattern) = &self.pattern {
            rules.push(Rule::matches(&self.name, pattern));
        }
        rules
    }
}

/// The name of a kind of rule, for what is said when it does not fit.
fn rule_name(kind: &RuleKind) -> &'static str {
    match kind {
        RuleKind::Required => "required",
        RuleKind::Range { .. } => "range",
        RuleKind::Matches(_) => "matches",
        RuleKind::Text { .. } => "text",
        RuleKind::Compare { .. } => "compare",
        RuleKind::Criteria(_) => "criteria",
        RuleKind::IsTrue => "is_true",
        RuleKind::Unique => "unique",
        RuleKind::UniqueTogether(_) => "unique_together",
        RuleKind::Exists { .. } => "exists",
        RuleKind::Referenced { .. } => "referenced",
        _ => "custom",
    }
}

/// Every field of `entity` in this vocabulary.
pub fn fields_of(entity: &EntityInfo) -> Result<Vec<FieldDecl>, String> {
    entity.fields.iter().map(|field| FieldDecl::of(entity, field)).collect()
}

/// The entity `name` whose fields are `fields`, with their rules.
pub fn entity_of(name: &str, fields: &[FieldDecl]) -> Result<EntityInfo, String> {
    let mut entity = EntityInfo::new(name);
    for decl in fields {
        entity = entity.field(decl.field()?);
        for rule in decl.rules() {
            entity = entity.rule(rule);
        }
    }
    Ok(entity)
}

/// The `plugin.toml` of an external plugin, as it is written. The core reads it without starting
/// the program and checks it ([`check`](Self::check)); [`serve`](crate::serve) writes it for a
/// plugin in Rust, from its manifest and its points.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginToml {
    /// Lower-case letters, numbers and `-`.
    pub name: String,
    /// Its version, `0.1.0`.
    pub version: String,
    /// What it is for, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// The program: a name in `PATH`, or a path inside the folder (`./plugin`).
    pub command: String,
    /// The program on Windows, when it is another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows_command: Option<String>,
    /// Its arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// What it asks the host for: `network`, `process`, `secrets`, `disk`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// Plugins that come before it, with no version asked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
    /// The longest one call may take, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// How long it may sit with no call before it is stopped, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_ms: Option<u64>,
    /// The version of `raven-ci-plugin-api` it was built with, for a plugin in Rust.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    /// The versions it accepts of other plugins: `git = "^2"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub requires: BTreeMap<String, String>,
    /// What its configuration holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<FieldDecl>,
    /// Its kinds of step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepDecl>,
    /// Its subcommands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandDecl>,
    /// Its ways of fetching code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scms: Vec<IdDecl>,
    /// Its ways of finding branches and pull requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_sources: Vec<IdDecl>,
    /// Its ways of asking for builds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<IdDecl>,
    /// What it takes from a build when it ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishers: Vec<IdDecl>,
    /// Where it sends the status of a commit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_reporters: Vec<IdDecl>,
    /// How it tells people about builds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifiers: Vec<IdDecl>,
    /// How it brings agents up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_launchers: Vec<IdDecl>,
    /// What it does to every build.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_hooks: Vec<IdDecl>,
    /// The pipeline files it reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline_formats: Vec<FormatDecl>,
    /// The kinds of credential it brings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credential_kinds: Vec<CredentialDecl>,
    /// Its addresses in the HTTP API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouteDecl>,
}

impl PluginToml {
    /// Whether it offers any point at all.
    pub fn offers_something(&self) -> bool {
        !(self.steps.is_empty()
            && self.commands.is_empty()
            && self.scms.is_empty()
            && self.branch_sources.is_empty()
            && self.triggers.is_empty()
            && self.publishers.is_empty()
            && self.status_reporters.is_empty()
            && self.notifiers.is_empty()
            && self.agent_launchers.is_empty()
            && self.build_hooks.is_empty()
            && self.pipeline_formats.is_empty()
            && self.credential_kinds.is_empty()
            && self.routes.is_empty())
    }

    /// Every problem with it, in words; empty when it is fine.
    pub fn check(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.name.is_empty()
            || !self.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!("o nome {:?} tem que ter só letras minúsculas, números e '-'", self.name));
        }
        if let Err(e) = ravenrs_app::Version::parse(&self.version) {
            problems.push(format!("a versão: {e}"));
        }
        for name in &self.permissions {
            if Permission::parse(name).is_none() {
                problems.push(format!(
                    "a permissão {name:?} não existe; as que há são {}",
                    Permission::ALL.map(Permission::name).join(", ")
                ));
            }
        }
        for (plugin, range) in &self.requires {
            if let Err(e) = VersionRange::compatible(range.trim_start_matches('^')) {
                problems.push(format!("requires.{plugin}: {e}"));
            }
        }
        if let Some(api) = &self.api
            && let Err(why) = api_accepts(api)
        {
            problems.push(why);
        }
        if !self.offers_something() {
            problems.push(
                "não oferece nenhum passo ([[steps]]), comando ([[commands]]) nem outro ponto".to_owned(),
            );
        }
        for kind in &self.credential_kinds {
            if let Err(why) = entity_of(&kind.id, &kind.fields) {
                problems.push(format!("a credencial {}: {why}", kind.id));
            }
        }
        if let Err(why) = entity_of(&self.name, &self.config) {
            problems.push(format!("a configuração: {why}"));
        }
        for route in &self.routes {
            if !route.path.starts_with('/') {
                problems.push(format!("a rota {} {} tem que começar com /", route.method, route.path));
            }
        }
        problems
    }
}

/// Whether a plugin that asks for `wanted` of `raven-ci-plugin-api` (a version it was built with,
/// `0.1.0`, or a range, `^2`) runs with this one, [`API_VERSION`]; the refusal says both.
pub fn api_accepts(wanted: &str) -> Result<(), String> {
    let ours = ravenrs_app::Version::parse(API_VERSION).unwrap_or_default();
    let range = VersionRange::compatible(wanted.trim().trim_start_matches(['^', '=']))
        .map_err(|e| format!("a versão {wanted:?} da raven-ci-plugin-api: {e}"))?;
    if range.accepts(ours) {
        Ok(())
    } else {
        Err(format!(
            "o plugin pede a raven-ci-plugin-api {wanted} (a partir da {} e abaixo da {}), e este Raven CI tem a {API_VERSION}",
            range.from, range.below
        ))
    }
}

/// An attachment as it goes in a line: its bytes in base64, or the path of a file in the folder of
/// the build when they are too many for a line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachmentWire {
    /// What kind it is.
    pub kind: String,
    /// Its name, as a path.
    pub name: String,
    /// Its type.
    pub mime: String,
    /// What a screen shows without reading the bytes.
    #[serde(default)]
    pub summary: Value,
    /// Its bytes, in base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    /// The file that holds its bytes, inside the folder of the build; the core removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

impl AttachmentWire {
    /// `attachment` for a line: in base64 up to [`INLINE_LIMIT`] bytes, otherwise written to a
    /// file in [`ATTACHMENTS_DIR`] of `workspace`.
    pub fn from_attachment(attachment: Attachment, workspace: &Path) -> Result<Self, String> {
        let mut wire = Self {
            kind: attachment.kind,
            name: attachment.name,
            mime: attachment.mime,
            summary: attachment.summary,
            base64: None,
            file: None,
        };
        if attachment.bytes.len() <= INLINE_LIMIT {
            wire.base64 = Some(STANDARD.encode(&attachment.bytes));
            return Ok(wire);
        }
        let dir = workspace.join(ATTACHMENTS_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join(format!("{}-{}", std::process::id(), next_file()));
        std::fs::write(&file, &attachment.bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        wire.file = Some(file.display().to_string());
        Ok(wire)
    }

    /// The attachment, reading (and removing) its file when it came in one. A file outside
    /// `workspace` is refused: a plugin hands over only what is in the folder of the build.
    pub fn into_attachment(self, workspace: &Path) -> Result<Attachment, String> {
        let bytes = match (self.base64, self.file) {
            (Some(text), _) => {
                STANDARD.decode(text.as_bytes()).map_err(|e| format!("o anexo {}: base64: {e}", self.name))?
            }
            (None, Some(file)) => {
                let path = Path::new(&file);
                let inside = path
                    .canonicalize()
                    .ok()
                    .zip(workspace.canonicalize().ok())
                    .is_some_and(|(path, root)| path.starts_with(root));
                if !inside {
                    return Err(format!("o anexo {} está fora da pasta do build: {file}", self.name));
                }
                let bytes = std::fs::read(path).map_err(|e| format!("o anexo {}: {file}: {e}", self.name))?;
                let _ = std::fs::remove_file(path);
                if let Some(dir) = path.parent() {
                    let _ = std::fs::remove_dir(dir);
                }
                bytes
            }
            (None, None) => Vec::new(),
        };
        Ok(Attachment { kind: self.kind, name: self.name, mime: self.mime, bytes, summary: self.summary })
    }
}

fn next_file() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

/// A request to a route, as it goes in a line: the body in base64.
pub fn http_request_to_wire(request: &HttpRequest) -> Value {
    json!({
        "method": request.method,
        "path": request.path,
        "query": request.query,
        "headers": request.headers,
        "body": STANDARD.encode(&request.body),
        "actor": request.actor,
        "roles": request.roles,
    })
}

/// A request to a route, out of a line.
pub fn http_request_from_wire(value: &Value) -> Result<HttpRequest, String> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let map = |key: &str| -> BTreeMap<String, String> {
        value.get(key).cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    };
    Ok(HttpRequest {
        method: text("method"),
        path: text("path"),
        query: map("query"),
        headers: map("headers"),
        body: STANDARD.decode(text("body").as_bytes()).map_err(|e| format!("o corpo: base64: {e}"))?,
        actor: value.get("actor").and_then(Value::as_str).map(str::to_owned),
        roles: value.get("roles").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(),
    })
}

/// What a route answers, as it goes in a line.
pub fn http_response_to_wire(response: &HttpResponse) -> Value {
    json!({
        "status": response.status,
        "content_type": response.content_type,
        "body": STANDARD.encode(&response.body),
    })
}

/// What a route answers, out of a line.
pub fn http_response_from_wire(value: &Value) -> Result<HttpResponse, String> {
    let body = value.get("body").and_then(Value::as_str).unwrap_or_default();
    Ok(HttpResponse {
        status: value
            .get("status")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok())
            .unwrap_or(200),
        content_type: value
            .get("content_type")
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream")
            .to_owned(),
        body: STANDARD.decode(body.as_bytes()).map_err(|e| format!("o corpo: base64: {e}"))?,
    })
}

/// The steps a build runs after its own, as they go in a line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PostWire {
    /// When they run.
    pub when: PostWhen,
    /// The steps.
    pub flow: FlowDefinition,
    /// The node they start at.
    pub start: String,
}

/// A [`Pipeline`] as it goes in a line: its parameters as [`FieldDecl`]s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PipelineWire {
    /// The steps.
    pub flow: FlowDefinition,
    /// The node a build starts at.
    pub start: String,
    /// The parameters a build takes.
    #[serde(default)]
    pub parameters: Vec<FieldDecl>,
    /// The variables every step gets.
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// The steps after the build's.
    #[serde(default)]
    pub post: Vec<PostWire>,
    /// What is taken from the build.
    #[serde(default)]
    pub publishers: Vec<Use>,
    /// The most the steps may take, in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// How many builds to keep.
    #[serde(default)]
    pub keep_builds: Option<u32>,
    /// Where the build runs.
    #[serde(default)]
    pub agent: Option<Use>,
}

impl PipelineWire {
    /// `pipeline` for a line; a parameter with no word in [`FieldDecl`] is said.
    pub fn from_pipeline(pipeline: Pipeline) -> Result<Self, String> {
        let entity = EntityInfo::new("parametros");
        let parameters = pipeline
            .parameters
            .iter()
            .map(|field| FieldDecl::of(&entity, field))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            flow: pipeline.flow,
            start: pipeline.start,
            parameters,
            environment: pipeline.environment,
            post: pipeline
                .post
                .into_iter()
                .map(|post| PostWire { when: post.when, flow: post.flow, start: post.start })
                .collect(),
            publishers: pipeline.publishers,
            timeout_ms: pipeline.timeout_ms,
            keep_builds: pipeline.keep_builds,
            agent: pipeline.agent,
        })
    }

    /// The pipeline it carries.
    pub fn into_pipeline(self) -> Result<Pipeline, String> {
        let parameters = self.parameters.iter().map(FieldDecl::field).collect::<Result<_, _>>()?;
        Ok(Pipeline {
            flow: self.flow,
            start: self.start,
            parameters,
            environment: self.environment,
            post: self
                .post
                .into_iter()
                .map(|post| PostSteps { when: post.when, flow: post.flow, start: post.start })
                .collect(),
            publishers: self.publishers,
            timeout_ms: self.timeout_ms,
            keep_builds: self.keep_builds,
            agent: self.agent,
        })
    }
}

/// The errors of a pipeline file, as they go in a line.
pub fn format_errors_to_wire(errors: &[FormatError]) -> Value {
    Value::Array(errors.iter().map(|e| json!({ "line": e.line, "message": e.message })).collect())
}

/// The errors of a pipeline file, out of a line.
pub fn format_errors_from_wire(value: &Value) -> Vec<FormatError> {
    value
        .as_array()
        .map(|errors| {
            errors
                .iter()
                .map(|e| FormatError {
                    line: e.get("line").and_then(Value::as_u64).and_then(|l| usize::try_from(l).ok()),
                    message: e.get("message").and_then(Value::as_str).unwrap_or_default().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The code of every error the protocol sends: a JSON-RPC error of the application.
pub const ERROR_CODE: i64 = -32000;

/// `error` as the `error` of a JSON-RPC answer: its words in `message`, and in `data` what makes
/// it the same [`CiError`] on the other side.
pub fn error_to_wire(error: &CiError) -> Value {
    let data = match error {
        CiError::Denied { plugin, permission, what } => {
            json!({ "kind": "denied", "plugin": plugin, "permission": permission.name(), "what": what })
        }
        CiError::Settings { field, reason } => {
            json!({ "kind": "settings", "field": field, "reason": reason })
        }
        CiError::Unavailable(what) => json!({ "kind": "unavailable", "what": what }),
        CiError::Failed(what) => json!({ "kind": "failed", "what": what }),
    };
    json!({ "code": ERROR_CODE, "message": error.to_string(), "data": data })
}

/// The [`CiError`] of the `error` of a JSON-RPC answer: the same one, when `data` says it, or a
/// failure with its `message`.
pub fn error_from_wire(error: &Value) -> CiError {
    let message = error.get("message").and_then(Value::as_str).unwrap_or("erro sem mensagem").to_owned();
    let data = error.get("data").unwrap_or(&Value::Null);
    let text = |key: &str| data.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    match data.get("kind").and_then(Value::as_str) {
        Some("denied") => match Permission::parse(&text("permission")) {
            Some(permission) => CiError::Denied { plugin: text("plugin"), permission, what: text("what") },
            None => CiError::Failed(message),
        },
        Some("settings") => CiError::Settings { field: text("field"), reason: text("reason") },
        Some("unavailable") => CiError::Unavailable(text("what")),
        Some("failed") => CiError::Failed(text("what")),
        _ => CiError::Failed(message),
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_app::Rule;

    use super::*;

    #[test]
    fn a_form_goes_to_plugin_toml_and_back_with_its_rules() {
        let token = EntityInfo::new("Token")
            .field(FieldInfo::new("usuario", FieldKind::Text).caption("Usuário"))
            .field(FieldInfo::new("token", FieldKind::Text).secret())
            .field(FieldInfo::new(
                "escopo",
                FieldKind::Enum(vec![EnumOption::new("ler", "Ler"), EnumOption::new("tudo", "Tudo")]),
            ))
            .rule(Rule::required("token"))
            .rule(Rule::range("usuario", None, Some(3.0)));
        let fields = fields_of(&token).expect("fits");
        assert_eq!(fields[0].caption.as_deref(), Some("Usuário"));
        assert!(fields[1].secret && fields[1].required);
        assert_eq!(fields[2].options.len(), 2);
        let back = entity_of("Token", &fields).expect("back");
        assert_eq!(back.fields, token.fields);
        let checked = crate::check_settings(&back, &json!({ "escopo": "nada" })).expect_err("wrong");
        assert_eq!(checked.len(), 2, "the choice and the required token: {checked:?}");
        let image = EntityInfo::new("X").field(FieldInfo::new("foto", FieldKind::Image));
        assert!(fields_of(&image).expect_err("no word").contains("foto"));
    }

    #[test]
    fn plugin_toml_says_every_problem() {
        let file = PluginToml {
            name: "Ruim".into(),
            version: "x".into(),
            command: "nada".into(),
            permissions: vec!["root".into()],
            api: Some("^2".into()),
            ..PluginToml::default()
        };
        let problems = file.check().join("; ");
        assert!(problems.contains("o nome \"Ruim\""), "{problems}");
        assert!(problems.contains("a versão"), "{problems}");
        assert!(problems.contains("\"root\""), "{problems}");
        assert!(problems.contains(&format!("pede a raven-ci-plugin-api ^2 (a partir da 2.0.0 e abaixo da 3.0.0), e este Raven CI tem a {API_VERSION}")), "{problems}");
        assert!(problems.contains("não oferece nenhum passo"), "{problems}");
        assert!(api_accepts(API_VERSION).is_ok());
    }

    #[test]
    fn a_denial_crosses_the_wire_whole() {
        let denied =
            CiError::Denied { plugin: "p".into(), permission: Permission::Secrets, what: "ler x".into() };
        assert_eq!(error_from_wire(&error_to_wire(&denied)), denied);
        assert_eq!(
            error_from_wire(&json!({ "code": -32000, "message": "caiu" })),
            CiError::failed("caiu"),
            "an error with no data is a failure with its words"
        );
    }

    #[test]
    fn a_big_attachment_goes_by_a_file_of_the_build_and_comes_back() {
        let workspace = std::env::temp_dir().join(format!("raven-ci-anexo-{}", std::process::id()));
        std::fs::create_dir_all(&workspace).expect("folder");
        let small = Attachment::new("artifact", "a.txt", "text/plain", b"oi".to_vec());
        let wire = AttachmentWire::from_attachment(small.clone(), &workspace).expect("wire");
        assert!(wire.base64.is_some());
        assert_eq!(wire.into_attachment(&workspace).expect("back"), small);
        let big = Attachment::new("artifact", "b.bin", "application/octet-stream", vec![7; INLINE_LIMIT + 1]);
        let wire = AttachmentWire::from_attachment(big.clone(), &workspace).expect("wire");
        let file = wire.file.clone().expect("in a file");
        assert_eq!(wire.into_attachment(&workspace).expect("back"), big);
        assert!(!Path::new(&file).exists(), "the file is removed once read");
        let outside = AttachmentWire {
            file: Some("/etc/hosts".into()),
            base64: None,
            ..AttachmentWire::from_attachment(small, &workspace).expect("w")
        };
        assert!(outside.into_attachment(&workspace).expect_err("outside").contains("fora da pasta"));
        let _ = std::fs::remove_dir_all(&workspace);
    }
}
