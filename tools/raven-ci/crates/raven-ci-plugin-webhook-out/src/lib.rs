//! The plugin `webhook-out` of Raven CI: the end of a build posted as JSON to an address, signed.
//!
//! # Its role
//!
//! A chat, a bot or another service hears of builds through a webhook. This plugin is the
//! [`Notifier`] `webhook`: at the end of a build of a job that names it, it posts the build as
//! JSON to the address the job says.
//!
//! # How it works
//!
//! - **On the job**: `{"notify": [{"id": "webhook", "url": "https://chat.example.com/hook",
//!   "secret": "chat-hook", "when": "falha"}]}`. `when` is the rule every notifier shares
//!   ([`NoticeWhen`]); `secret`, when there is one, names a credential of the vault.
//! - **The body** is `{"event": "build.ended", "says": "falhou", "build": {...}}`, the build as
//!   the points of Raven CI see it ([`BuildInfo`]): job, number, status, duration, reason, commit,
//!   parameters, the previous status and the address of the panel.
//! - **Signed**, with a secret: `X-Raven-CI-Signature: sha256=<HMAC-SHA256 of the body>`, the
//!   same signature the webhooks of the house check (`ravenrs_app_server::verify`).
//!   `X-Raven-CI-Event` says the event.
//! - **Tried again**: an answer 5xx, a 429 or no answer is tried again up to three more times,
//!   waiting 1 s, 4 s and 16 s (`retry_wait_ms` in the plugin's settings changes the first
//!   wait); a 4xx other than 429 is not, since sending it again would change nothing. It all runs
//!   on a thread of its own, so the end of the build does not wait, and what happened goes to the
//!   log of the server.
//! - **Refused** before anything goes: a job with no `url`, an address that is not http or https,
//!   a secret that is not in the vault; said in the log of the build, which goes on.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{BuildEvent, CiError, CiPlugin, Host, NoticeWhen, Notifier, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use serde_json::{Value, json};

/// The header with the signature.
pub const SIGNATURE_HEADER: &str = "X-Raven-CI-Signature";
/// The header with the event.
pub const EVENT_HEADER: &str = "X-Raven-CI-Event";
/// The event of the end of a build.
pub const ENDED: &str = "build.ended";
/// How many times a delivery is tried again.
const AGAIN: u32 = 3;
/// The first wait before trying again, unless the settings say another.
const FIRST_WAIT_MS: u64 = 1000;
/// How long one try may take.
const TRY_TIMEOUT: Duration = Duration::from_secs(15);

/// The plugin `webhook-out`.
#[derive(Default)]
pub struct WebhookOutPlugin {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl std::fmt::Debug for WebhookOutPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookOutPlugin").finish_non_exhaustive()
    }
}

impl WebhookOutPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for WebhookOutPlugin {
    fn name(&self) -> &str {
        "webhook-out"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Network.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for WebhookOutPlugin {
    fn summary(&self) -> String {
        "Manda o fim de cada build como JSON assinado para um endereço, tentando de novo quando falha."
            .to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("WebhookOutSettings").field(FieldInfo::new("retry_wait_ms", FieldKind::Integer)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Network, "mandar os avisos dos builds")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn notifiers(&self) -> Vec<Arc<dyn Notifier>> {
        vec![Arc::new(Webhook(self.host.clone()))]
    }
}

/// The notifier `webhook`.
struct Webhook(Arc<Mutex<Option<Arc<dyn Host>>>>);

/// The waits before each new try: `first`, then four times the one before.
pub fn waits(first_ms: u64) -> Vec<Duration> {
    (0..AGAIN).map(|n| Duration::from_millis(first_ms.saturating_mul(4u64.pow(n)))).collect()
}

/// What one try got: delivered, worth trying again, or not worth it.
enum Tried {
    Delivered,
    Again(String),
    Refused(String),
}

fn post(agent: &ureq::Agent, url: &str, body: &[u8], signature: Option<&str>) -> Tried {
    let mut request = agent
        .post(url)
        .header("Content-Type", "application/json")
        .header("User-Agent", concat!("raven-ci-webhook-out/", env!("CARGO_PKG_VERSION")))
        .header(EVENT_HEADER, ENDED);
    if let Some(signature) = signature {
        request = request.header(SIGNATURE_HEADER, signature);
    }
    match request.send(body) {
        Ok(answer) => {
            let status = answer.status().as_u16();
            match status {
                200..=299 => Tried::Delivered,
                429 | 500..=599 => Tried::Again(format!("respondeu {status}")),
                _ => Tried::Refused(format!("respondeu {status}")),
            }
        }
        Err(e) => Tried::Again(e.to_string()),
    }
}

impl Notifier for Webhook {
    fn id(&self) -> &str {
        "webhook"
    }

    fn notify(&self, event: &BuildEvent, settings: &Value) -> Result<(), CiError> {
        let when = NoticeWhen::of(settings)?;
        let Some(says) = when.due(event) else { return Ok(()) };
        let BuildEvent::Ended(build) = event else { return Ok(()) };
        let url = settings
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| CiError::settings("url", "diga o endereço que recebe o aviso"))?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(CiError::settings("url", format!("{url} não é um endereço http ou https")));
        }
        let host = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin webhook-out está desligado"))?;
        let body = serde_json::to_vec(&json!({ "event": ENDED, "says": says, "build": build }))
            .map_err(|e| CiError::failed(e.to_string()))?;
        let signature = match settings.get("secret").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            Some(name) => Some(ravenrs_app_server::sign(host.secret(name)?.as_bytes(), &body)),
            None => None,
        };
        let first = host.settings().get("retry_wait_ms").and_then(Value::as_u64).unwrap_or(FIRST_WAIT_MS);
        let (url, job, number) = (url.to_owned(), build.job.clone(), build.number);
        std::thread::Builder::new()
            .name("webhook-out".to_owned())
            .spawn(move || {
                let agent = ureq::Agent::config_builder()
                    .http_status_as_error(false)
                    .timeout_global(Some(TRY_TIMEOUT))
                    .build()
                    .new_agent();
                let mut waits = waits(first).into_iter();
                let mut tries = 0;
                loop {
                    tries += 1;
                    match post(&agent, &url, &body, signature.as_deref()) {
                        Tried::Delivered => {
                            if tries > 1 {
                                host.log(&format!("aviso de {job} #{number} entregue em {url} na tentativa {tries}"));
                            }
                            return;
                        }
                        Tried::Refused(why) => {
                            host.log(&format!("aviso de {job} #{number} recusado por {url}: {why}"));
                            return;
                        }
                        Tried::Again(why) => match waits.next() {
                            Some(wait) => std::thread::sleep(wait),
                            None => {
                                host.log(&format!(
                                    "aviso de {job} #{number} não chegou em {url} depois de {tries} tentativas: {why}"
                                ));
                                return;
                            }
                        },
                    }
                }
            })
            .map(|_| ())
            .map_err(|e| CiError::failed(format!("o aviso não começou: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_wait_is_four_times_the_one_before() {
        assert_eq!(waits(1000), [Duration::from_secs(1), Duration::from_secs(4), Duration::from_secs(16)]);
    }
}
