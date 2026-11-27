//! Notifications a provider pushes in: a payment confirmed, a delivery made. Each source has a
//! secret per organization, and a request is read only after its organization and signature check
//! out. Each event is kept by organization, source and the provider's own id, so one delivered twice
//! has one effect, and one older than what is known about the same subject changes nothing.
//!
//! The envelope a provider's adapter sends, signed with HMAC-SHA256 of the body:
//!
//! ```text
//! POST /api/hooks/{source}
//! X-RavenRS-Organization: T1
//! X-RavenRS-Signature: sha256=<hex of HMAC-SHA256(secret, body)>
//!
//! { "id": "evt_1", "subject": "charge_9", "at": 1731600000000, "data": { .. } }
//! ```
//!
//! `at` is when the fact happened at the provider, in milliseconds; `subject` is what it is about.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, ObjectSpace, Query, Value};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};

use crate::Failure;

/// The entity of the events received, which storage has to know for [`ApiService::webhook`].
///
/// [`ApiService::webhook`]: crate::ApiService::webhook
pub const WEBHOOK_EVENT: &str = "WebhookEvent";
/// The header with the organization the event is for.
pub const ORGANIZATION_HEADER: &str = "x-ravenrs-organization";
/// The header with `sha256=` and the HMAC of the body.
pub const SIGNATURE_HEADER: &str = "x-ravenrs-signature";
/// The longest provider id or subject kept.
const MOST_ID: usize = 200;

/// The events received, kept to know a repeated one and the latest fact of each subject. It keeps
/// neither the body's signature nor the secret.
pub fn webhook_event_entity() -> EntityInfo {
    EntityInfo::new(WEBHOOK_EVENT)
        .caption("Webhook events")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("organization", FieldKind::Text))
        .field(FieldInfo::new("source", FieldKind::Text))
        .field(FieldInfo::new("provider_id", FieldKind::Text).default_property())
        .field(FieldInfo::new("subject", FieldKind::Text))
        .field(FieldInfo::new("at", FieldKind::Integer))
        .field(FieldInfo::new("applied", FieldKind::Boolean))
}

/// What a source's handler gets: the event and a unit of work, committed with the record of the
/// event, so the effect and the mark that it happened go in together or not at all.
#[non_exhaustive]
pub struct WebhookContext<'a> {
    /// The organization it is for.
    pub organization: &'a str,
    /// The provider's id of the event.
    pub id: &'a str,
    /// What it is about.
    pub subject: &'a str,
    /// When it happened at the provider, in milliseconds.
    pub at: i64,
    /// The unit of work, with no person behind it: the source is trusted by its secret.
    pub space: &'a RefCell<Box<dyn ObjectSpace>>,
}

impl fmt::Debug for WebhookContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookContext")
            .field("id", &self.id)
            .field("subject", &self.subject)
            .finish_non_exhaustive()
    }
}

/// What a source does with an event it has not seen and that is newer than what it knows.
pub type WebhookHandler = Rc<dyn Fn(&WebhookContext<'_>, &Json) -> Result<(), Failure>>;

/// A source of webhooks: its secret for each organization this service answers for.
pub struct Webhook {
    secrets: Vec<(String, Vec<u8>)>,
    handler: WebhookHandler,
}

impl fmt::Debug for Webhook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The secrets never leave, not even in a debug print.
        let organizations: Vec<&str> = self.secrets.iter().map(|(o, _)| o.as_str()).collect();
        f.debug_struct("Webhook").field("organizations", &organizations).finish_non_exhaustive()
    }
}

impl Webhook {
    /// A source whose events `handler` applies.
    pub fn new(handler: impl Fn(&WebhookContext<'_>, &Json) -> Result<(), Failure> + 'static) -> Self {
        Self { secrets: Vec::new(), handler: Rc::new(handler) }
    }

    /// The secret this source signs with for `organization`, kept apart from any payload or log.
    pub fn secret(mut self, organization: impl Into<String>, secret: impl AsRef<[u8]>) -> Self {
        self.secrets.push((organization.into(), secret.as_ref().to_vec()));
        self
    }

    fn secret_of(&self, organization: &str) -> Option<&[u8]> {
        self.secrets.iter().find(|(o, _)| o == organization).map(|(_, s)| s.as_slice())
    }
}

/// HMAC-SHA256 of `body` with `secret`, as RFC 2104 puts it.
pub fn sign(secret: &[u8], body: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut key = if secret.len() > BLOCK { Sha256::digest(secret).to_vec() } else { secret.to_vec() };
    key.resize(BLOCK, 0);
    let pad = |byte: u8| key.iter().map(|k| k ^ byte).collect::<Vec<u8>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(body).finalize();
    let outer = Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize();
    format!("sha256={}", outer.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// Whether `signature` (`sha256=<hex>`) is the HMAC-SHA256 of `body` with `secret`, compared
/// without stopping at the first difference, so the time taken says nothing of where it differs.
///
/// It reads the signature of [`ApiService::webhook`](crate::ApiService::webhook) and also the
/// `X-Hub-Signature-256` a GitHub webhook sends, which is the same text.
pub fn verify(secret: &[u8], body: &[u8], signature: &str) -> bool {
    same(sign(secret, body).as_bytes(), signature.trim().as_bytes())
}

/// Compares without stopping at the first difference, so the time taken says nothing.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The one answer to anything wrong before the body is read: no detail of which part failed.
fn refused() -> Failure {
    Failure::new(401, "unauthorized", "The notification was not accepted.")
}

/// Answers one notification for `source`.
pub(crate) fn receive(
    hook: &Webhook,
    source: &str,
    request: &crate::ApiRequest,
    space: Box<dyn ObjectSpace>,
) -> Result<(u16, Json), Failure> {
    let organization = request.headers.get(ORGANIZATION_HEADER).ok_or_else(refused)?;
    let secret = hook.secret_of(organization).ok_or_else(refused)?;
    let signature = request.headers.get(SIGNATURE_HEADER).ok_or_else(refused)?;
    if !verify(secret, &request.body, signature) {
        return Err(refused());
    }
    // Only now is the body read.
    let body: Json = serde_json::from_slice(&request.body)
        .map_err(|_| Failure::bad_request("The body has to be JSON."))?;
    let field = |name: &str| {
        body.get(name)
            .and_then(Json::as_str)
            .filter(|t| !t.is_empty() && t.len() <= MOST_ID)
            .map(str::to_owned)
            .ok_or_else(|| Failure::bad_request(format!("The notification needs {name}.")))
    };
    let (id, subject) = (field("id")?, field("subject")?);
    let at = body
        .get("at")
        .and_then(Json::as_i64)
        .ok_or_else(|| Failure::bad_request("The notification needs at."))?;
    let data = body.get("data").cloned().unwrap_or(Json::Null);

    let space = RefCell::new(space);
    let known = space.borrow().query(WEBHOOK_EVENT, &Query::all())?;
    let ours: Vec<&Object> = known
        .iter()
        .filter(|e| e.text("organization") == *organization && e.text("source") == source)
        .collect();
    if ours.iter().any(|e| e.text("provider_id") == id) {
        return Ok((200, json!({ "received": true, "duplicate": true })));
    }
    let latest = ours
        .iter()
        .filter(|e| e.text("subject") == subject && e.get("applied") == Value::Bool(true))
        .filter_map(|e| match e.get("at") {
            Value::Number(at) => Some(at as i64),
            _ => None,
        })
        .max();
    // A fact older than one already applied to the same subject is kept, but changes nothing.
    let applies = latest.is_none_or(|latest| at > latest);
    if applies {
        let context = WebhookContext { organization, id: &id, subject: &subject, at, space: &space };
        (hook.handler)(&context, &data)?;
    }
    let mut record = space.borrow_mut().new_object(WEBHOOK_EVENT)?;
    record.set("organization", organization.as_str());
    record.set("source", source);
    record.set("provider_id", id.as_str());
    record.set("subject", subject.as_str());
    record.set("at", at as f64);
    record.set("applied", applies);
    let mut space = space.into_inner();
    space.save(record)?;
    space.commit()?;
    tracing::info!(source, applied = applies, "webhook received");
    Ok((200, json!({ "received": true, "applied": applies })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_is_the_hmac_of_rfc_4231() {
        // Test case 2 of RFC 4231.
        let signed = sign(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(signed, "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert!(same(b"abc", b"abc") && !same(b"abc", b"abd") && !same(b"abc", b"ab"));
    }

    #[test]
    fn verify_takes_only_the_hmac_of_the_body_with_its_prefix() {
        let (secret, body) = (b"Jefe".as_slice(), b"what do ya want for nothing?".as_slice());
        let good = "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
        assert!(verify(secret, body, good));
        assert!(verify(secret, body, &format!("{good}\n")), "a line end around it is not part of it");
        assert!(!verify(secret, body, &good.replace("43", "44")));
        assert!(!verify(secret, body, &good[..good.len() - 2]));
        assert!(!verify(secret, body, good.trim_start_matches("sha256=")));
        assert!(!verify(b"outro", body, good));
    }
}
