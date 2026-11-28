//! The plugin `email` of Raven CI: a message when a build fails, and when it passes again.
//!
//! # Its role
//!
//! Who cares for a job should hear that it broke without opening the panel. This plugin is the
//! [`Notifier`] `email`: the core calls it at the end of each build of a job that names it, and
//! it sends the message through `ravenrs-mail`, the same sending the report server uses.
//!
//! # How it works
//!
//! - **The server** is in the plugin's settings: `host`, `port`, `security` (`starttls`, the
//!   default, `tls` or `none`), `from` and `credential`, the name of a credential `user-password`
//!   of the vault, read when a message goes.
//! - **On the job**: `{"notify": [{"id": "email", "to": ["ana@example.com"], "when": "falha"}]}`.
//!   `when` is the rule every notifier shares ([`NoticeWhen`]): `falha` (the default: it failed,
//!   ran out of time or lost its agent), `mudanca` (it broke after one that did not, or passed
//!   after one that broke) or `sempre`.
//! - **The message** says the job, the number, how it ended, how long it took, why, the commit and
//!   the address of the panel when the server has one. Its subject starts with `[Raven CI]`.
//! - **What goes wrong** never stops the build: a job with no `to`, an address that is not one,
//!   a server that does not answer, are said in the log of the build.

use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{BuildEvent, BuildInfo, CiError, CiPlugin, Host, NoticeWhen, Notifier, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_mail::{Mail, Mailer, Security, SmtpMailer, SmtpSettings};
use serde_json::Value;

/// What sends the messages: the SMTP server of the settings, or one a test gives.
type Sender = Arc<dyn Fn(&Value, &dyn Host) -> Result<Arc<dyn Mailer>, CiError> + Send + Sync>;

#[derive(Default)]
struct State {
    host: Option<Arc<dyn Host>>,
}

/// The plugin `email`.
pub struct EmailPlugin {
    state: Arc<Mutex<State>>,
    sender: Sender,
}

impl std::fmt::Debug for EmailPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailPlugin").finish_non_exhaustive()
    }
}

impl Default for EmailPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailPlugin {
    /// The plugin, sending through the SMTP server of its settings.
    pub fn new() -> Self {
        Self { state: Arc::default(), sender: Arc::new(smtp) }
    }

    /// The plugin, sending through `mailer`: for tests, and for a service that has its own.
    pub fn with_mailer(mailer: Arc<dyn Mailer>) -> Self {
        Self { state: Arc::default(), sender: Arc::new(move |_, _| Ok(mailer.clone())) }
    }
}

/// The SMTP server of `settings`, with the password of its credential.
fn smtp(settings: &Value, host: &dyn Host) -> Result<Arc<dyn Mailer>, CiError> {
    let text =
        |name: &str| settings.get(name).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
    let server =
        text("host").ok_or_else(|| CiError::settings("host", "diga o servidor SMTP do plugin email"))?;
    let from = text("from")
        .ok_or_else(|| CiError::settings("from", "diga o remetente, como CI <ci@example.com>"))?;
    let security = match text("security") {
        None | Some("starttls") => Security::Starttls,
        Some("tls") => Security::Tls,
        Some("none") => Security::None,
        Some(other) => {
            return Err(CiError::settings(
                "security",
                format!("{other} não existe; use starttls, tls ou none"),
            ));
        }
    };
    let mut smtp = SmtpSettings::new(server, from).security(security);
    if let Some(port) = settings.get("port").and_then(Value::as_u64) {
        smtp =
            smtp.port(u16::try_from(port).map_err(|_| CiError::settings("port", "a porta vai até 65535"))?);
    }
    if let Some(name) = text("credential") {
        let secret = host.secret(name)?;
        let (user, password) = secret.split_once(':').ok_or_else(|| {
            CiError::settings("credential", format!("a credencial {name} tem que ser user-password"))
        })?;
        smtp = smtp.credentials(user, password);
    }
    Ok(Arc::new(SmtpMailer::new(&smtp).map_err(CiError::failed)?))
}

impl Module for EmailPlugin {
    fn name(&self) -> &str {
        "email"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Network.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for EmailPlugin {
    fn summary(&self) -> String {
        "Avisa por e-mail quando um build falha, quando volta a passar ou sempre.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(
            EntityInfo::new("EmailSettings")
                .field(FieldInfo::new("host", FieldKind::Text))
                .field(FieldInfo::new("port", FieldKind::Integer))
                .field(FieldInfo::new("security", FieldKind::Text))
                .field(FieldInfo::new("from", FieldKind::Text))
                .field(FieldInfo::new("credential", FieldKind::Text)),
        )
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Network, "falar com o servidor de e-mail")?;
        self.state.lock().unwrap_or_else(PoisonError::into_inner).host = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).host = None;
    }

    fn notifiers(&self) -> Vec<Arc<dyn Notifier>> {
        vec![Arc::new(Email { state: self.state.clone(), sender: self.sender.clone() })]
    }
}

/// The notifier `email`.
struct Email {
    state: Arc<Mutex<State>>,
    sender: Sender,
}

/// `65 s` as `1min 5s`.
pub fn duration_words(ms: u64) -> String {
    let seconds = ms / 1000;
    match (seconds / 3600, (seconds % 3600) / 60, seconds % 60) {
        (0, 0, 0) => format!("{ms}ms"),
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m}min"),
        (0, m, s) => format!("{m}min {s}s"),
        (h, m, _) => format!("{h}h {m}min"),
    }
}

/// The message about `build`, which ended `words`.
pub fn message(build: &BuildInfo, words: &str, to: Vec<String>) -> Mail {
    let subject = format!("[Raven CI] {} #{}: {words}", build.job, build.number);
    let mut lines = vec![
        format!("O build {} do job {} {words}.", build.number, build.job),
        String::new(),
        format!("Resultado: {}", build.status.words()),
    ];
    if let Some(ms) = build.duration_ms {
        lines.push(format!("Duração: {}", duration_words(ms)));
    }
    if let Some(reason) = build.reason.as_deref().filter(|r| !r.is_empty()) {
        lines.push(format!("Motivo: {reason}"));
    }
    if let Some(commit) = &build.commit {
        lines.push(format!("Commit: {commit}"));
    }
    if let Some(url) = &build.url {
        lines.push(format!("Build: {url}"));
    }
    Mail { to, subject, body: lines.join("\n") + "\n", attachments: Vec::new() }
}

impl Notifier for Email {
    fn id(&self) -> &str {
        "email"
    }

    fn notify(&self, event: &BuildEvent, settings: &Value) -> Result<(), CiError> {
        let when = NoticeWhen::of(settings)?;
        let Some(words) = when.due(event) else { return Ok(()) };
        let BuildEvent::Ended(build) = event else { return Ok(()) };
        let to: Vec<String> = match settings.get("to") {
            Some(Value::String(one)) => vec![one.clone()],
            Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        if to.is_empty() {
            return Err(CiError::settings(
                "to",
                "diga para quem vai o e-mail: \"to\": [\"ana@example.com\"]",
            ));
        }
        for address in &to {
            ravenrs_mail::check_address(address).map_err(|why| CiError::settings("to", why))?;
        }
        let host = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .host
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin email está desligado"))?;
        let mailer = (self.sender)(&host.settings(), host.as_ref())?;
        mailer.send(&message(build, words, to)).map_err(|e| CiError::failed(format!("o e-mail não foi: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_said_as_people_say_them() {
        assert_eq!(duration_words(450), "450ms");
        assert_eq!(duration_words(65_000), "1min 5s");
        assert_eq!(duration_words(120_000), "2min");
        assert_eq!(duration_words(3_725_000), "1h 2min");
    }
}
