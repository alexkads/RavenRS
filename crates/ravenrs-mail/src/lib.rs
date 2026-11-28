//! Sending email from a RavenRS service: a message with its attachments, through an SMTP server.
//!
//! # Its role
//!
//! A service that tells people something by email (the report server sending a scheduled report,
//! the CI telling that a build failed) needs the same few things: a message that cannot be turned
//! into another by what goes in its subject, attachments, and a connection to a mail server with
//! TLS. This crate is that, and nothing of where the message comes from: no reports, no builds.
//!
//! # How it works
//!
//! - [`Mail`] is a message: recipients, subject, text and [`MailAttachment`]s.
//! - [`Mailer`] sends one; [`SmtpMailer`] does it through an SMTP server with `lettre`, over
//!   STARTTLS (587), TLS (465) or, chosen on purpose, nothing ([`Security::None`], for a relay on
//!   the same machine or a test server). It blocks: call it off the async threads.
//! - [`SmtpSettings`] takes the credentials already read: where they come from (environment
//!   variables, a vault) is the caller's.
//! - Headers stay safe: a line break in the subject cannot add a `Bcc`.
//!
//! # How to use it
//!
//! ```no_run
//! use ravenrs_mail::{Mail, Mailer, Security, SmtpMailer, SmtpSettings};
//!
//! let settings = SmtpSettings::new("smtp.example.com", "CI <ci@example.com>")
//!     .security(Security::Starttls)
//!     .credentials("ci@example.com", "secret");
//! let mailer = SmtpMailer::new(&settings)?;
//! mailer.send(&Mail::new(["ana@example.com"], "Build 12 falhou", "O passo cargo test falhou."))?;
//! # Ok::<(), String>(())
//! ```

use std::fmt;
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment as LettreAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

/// How long the mail server may take.
const SMTP_TIMEOUT: Duration = Duration::from_secs(120);

/// A file that goes with a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailAttachment {
    /// Its name, as the person sees it.
    pub name: String,
    /// Its media type, such as `application/pdf`.
    pub mime: String,
    /// Its contents.
    pub bytes: Vec<u8>,
}

impl MailAttachment {
    /// The file `name`, of type `mime`, holding `bytes`.
    pub fn new(name: impl Into<String>, mime: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self { name: name.into(), mime: mime.into(), bytes }
    }
}

/// A message to send.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mail {
    /// The recipients, as `ana@example.com` or `Ana <ana@example.com>`.
    pub to: Vec<String>,
    /// The subject.
    pub subject: String,
    /// The text.
    pub body: String,
    /// The attached files.
    pub attachments: Vec<MailAttachment>,
}

impl Mail {
    /// A message of `body` to `to`, about `subject`, with no files.
    pub fn new(
        to: impl IntoIterator<Item = impl Into<String>>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            to: to.into_iter().map(Into::into).collect(),
            subject: subject.into(),
            body: body.into(),
            attachments: Vec::new(),
        }
    }

    /// With `file` attached too.
    pub fn attach(mut self, file: MailAttachment) -> Self {
        self.attachments.push(file);
        self
    }
}

/// Whether `address` is one a message can go to: `ana@example.com` or `Ana <ana@example.com>`;
/// otherwise, why not.
pub fn check_address(address: &str) -> Result<(), String> {
    address.parse::<Mailbox>().map(|_| ()).map_err(|_| format!("'{address}' is not a valid email address"))
}

/// Sends email. It blocks: call it off the async threads.
pub trait Mailer: Send + Sync {
    /// Sends one message, or says why it could not.
    fn send(&self, mail: &Mail) -> Result<(), String>;
}

/// How the connection to the mail server is protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Security {
    /// Upgrades to TLS, and fails if the server cannot (port 587).
    #[default]
    Starttls,
    /// TLS from the start (port 465).
    Tls,
    /// No encryption: only for a relay on the same machine or a local test server.
    None,
}

/// Where and how to send: the server, the sender and the credentials, already read.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SmtpSettings {
    /// The host name of the server.
    pub host: String,
    /// Its port; the usual one for `security` when none.
    pub port: Option<u16>,
    /// How the connection is protected.
    pub security: Security,
    /// The sender of every message, such as `Reports <reports@example.com>`.
    pub from: String,
    /// The user name and password, when the server asks for them.
    pub credentials: Option<(String, String)>,
}

impl fmt::Debug for SmtpSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The password never leaves, not even in a debug print.
        f.debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("from", &self.from)
            .field("user", &self.credentials.as_ref().map(|(user, _)| user))
            .finish()
    }
}

impl SmtpSettings {
    /// The server `host`, sending as `from`, over STARTTLS on its usual port, with no credentials.
    pub fn new(host: impl Into<String>, from: impl Into<String>) -> Self {
        Self { host: host.into(), from: from.into(), ..Self::default() }
    }

    /// On `port`.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Protected by `security`.
    pub fn security(mut self, security: Security) -> Self {
        self.security = security;
        self
    }

    /// Signing in as `user` with `password`.
    pub fn credentials(mut self, user: impl Into<String>, password: impl Into<String>) -> Self {
        self.credentials = Some((user.into(), password.into()));
        self
    }
}

/// Sends email through an SMTP server.
pub struct SmtpMailer {
    from: Mailbox,
    transport: SmtpTransport,
}

impl fmt::Debug for SmtpMailer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpMailer").field("from", &self.from).finish_non_exhaustive()
    }
}

impl SmtpMailer {
    /// Prepares the connection of `settings`; nothing is sent yet.
    pub fn new(settings: &SmtpSettings) -> Result<Self, String> {
        let from = settings.from.parse().map_err(|_| format!("'{}' is not a valid sender", settings.from))?;
        let relay_error = |e: lettre::transport::smtp::Error| format!("mail server {}: {e}", settings.host);
        let mut builder = match settings.security {
            Security::Starttls => SmtpTransport::starttls_relay(&settings.host).map_err(relay_error)?,
            Security::Tls => SmtpTransport::relay(&settings.host).map_err(relay_error)?,
            // Chosen explicitly, for a relay on the same machine.
            Security::None => SmtpTransport::builder_dangerous(&settings.host),
        };
        if let Some(port) = settings.port {
            builder = builder.port(port);
        }
        if let Some((user, password)) = &settings.credentials {
            builder = builder.credentials(Credentials::new(user.clone(), password.clone()));
        }
        Ok(Self { from, transport: builder.timeout(Some(SMTP_TIMEOUT)).build() })
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, mail: &Mail) -> Result<(), String> {
        let message = build_message(&self.from, mail)?;
        self.transport.send(&message).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// The message `mail` from `from`, as it goes over the wire.
fn build_message(from: &Mailbox, mail: &Mail) -> Result<Message, String> {
    let mut builder = Message::builder().from(from.clone()).subject(mail.subject.clone());
    if mail.to.is_empty() {
        return Err("the message has no recipient".to_owned());
    }
    for address in &mail.to {
        let mailbox: Mailbox = address.parse().map_err(|_| format!("'{address}' is not a valid address"))?;
        builder = builder.to(mailbox);
    }
    let mut parts = MultiPart::mixed().singlepart(SinglePart::plain(mail.body.clone()));
    for file in &mail.attachments {
        let kind = ContentType::parse(&file.mime).map_err(|e| e.to_string())?;
        parts = parts.singlepart(LettreAttachment::new(file.name.clone()).body(file.bytes.clone(), kind));
    }
    builder.multipart(parts).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv() -> MailAttachment {
        MailAttachment::new("a.csv", "text/csv; charset=utf-8", b"x".to_vec())
    }

    #[test]
    fn messages_carry_the_files_and_keep_headers_safe() -> Result<(), String> {
        let from: Mailbox = "Reports <r@example.com>".parse().map_err(|_| "from")?;
        let mail =
            Mail::new(["Ana <ana@example.com>", "b@example.com"], "Oi\r\nBcc: evil@example.com", "Segue")
                .attach(csv());
        let message = build_message(&from, &mail)?;
        let raw = String::from_utf8_lossy(&message.formatted()).into_owned();
        assert!(raw.contains("a.csv"));
        assert!(!raw.contains("\r\nBcc: evil"), "{raw}");
        assert_eq!(message.envelope().to().len(), 2);
        Ok(())
    }

    #[test]
    fn a_bad_address_or_none_is_an_error() {
        let Ok(from) = "r@example.com".parse::<Mailbox>() else { return };
        assert!(build_message(&from, &Mail::new(["nope"], "", "")).is_err());
        assert!(build_message(&from, &Mail::new(Vec::<String>::new(), "", "")).is_err());
        assert!(check_address("Ana <ana@example.com>").is_ok());
        assert!(check_address("ana").is_err());
    }

    #[test]
    fn a_bad_sender_is_said_and_the_password_never_shows() {
        let settings = SmtpSettings::new("localhost", "not an address").credentials("ci", "s3gr3do");
        assert!(SmtpMailer::new(&settings).is_err_and(|e| e.contains("not a valid sender")));
        assert!(!format!("{settings:?}").contains("s3gr3do"));
    }
}
