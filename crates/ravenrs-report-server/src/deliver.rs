//! Sends a run's files: by email, or to a folder under the server's output folder.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment as MailAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::config::{SmtpConfig, SmtpSecurity};
use crate::job::safe_relative_path;
use crate::render::OutputFile;

/// How long the mail server may take.
const SMTP_TIMEOUT: Duration = Duration::from_secs(120);

/// A message to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mail {
    /// The recipients.
    pub to: Vec<String>,
    /// The subject.
    pub subject: String,
    /// The text.
    pub body: String,
    /// The attached files.
    pub attachments: Vec<OutputFile>,
}

/// Sends email. Blocking: it runs off the async threads.
pub trait Mailer: Send + Sync {
    /// Sends one message.
    fn send(&self, mail: &Mail) -> Result<(), String>;
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
    /// Prepares the connection; credentials come from the environment.
    pub fn new(config: &SmtpConfig) -> Result<Self, String> {
        let from = config.from.parse().map_err(|_| format!("'{}' is not a valid sender", config.from))?;
        let relay_error = |e: lettre::transport::smtp::Error| format!("mail server {}: {e}", config.host);
        let mut builder = match config.security {
            SmtpSecurity::Starttls => SmtpTransport::starttls_relay(&config.host).map_err(relay_error)?,
            SmtpSecurity::Tls => SmtpTransport::relay(&config.host).map_err(relay_error)?,
            // Chosen explicitly in the configuration, for a relay on the same machine.
            SmtpSecurity::None => SmtpTransport::builder_dangerous(&config.host),
        };
        if let Some(port) = config.port {
            builder = builder.port(port);
        }
        let from_env = |variable: &str| {
            std::env::var(variable).map_err(|_| format!("the environment variable {variable} is not set"))
        };
        if let (Some(username), Some(password)) = (&config.username_env, &config.password_env) {
            builder = builder.credentials(Credentials::new(from_env(username)?, from_env(password)?));
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

fn build_message(from: &Mailbox, mail: &Mail) -> Result<Message, String> {
    let mut builder = Message::builder().from(from.clone()).subject(mail.subject.clone());
    for address in &mail.to {
        let mailbox: Mailbox = address.parse().map_err(|_| format!("'{address}' is not a valid address"))?;
        builder = builder.to(mailbox);
    }
    let mut parts = MultiPart::mixed().singlepart(SinglePart::plain(mail.body.clone()));
    for file in &mail.attachments {
        let kind = ContentType::parse(file.mime).map_err(|e| e.to_string())?;
        parts = parts.singlepart(MailAttachment::new(file.name.clone()).body(file.bytes.clone(), kind));
    }
    builder.multipart(parts).map_err(|e| e.to_string())
}

/// Writes files to `path` under `output_dir`, refusing to leave it.
pub fn write_to_folder(output_dir: &Path, path: &str, files: &[OutputFile]) -> Result<(), String> {
    let parts = safe_relative_path(path)?;
    let mut folder = output_dir.to_path_buf();
    folder.extend(parts);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{path}: {e}"))?;
    // A symbolic link inside the output folder could still point outside it.
    let base = output_dir.canonicalize().map_err(|e| format!("output folder: {e}"))?;
    let real = folder.canonicalize().map_err(|e| format!("{path}: {e}"))?;
    if !real.starts_with(&base) {
        return Err(format!("'{path}' leads outside the output folder"));
    }
    for file in files {
        std::fs::write(real.join(&file.name), &file.bytes).map_err(|e| format!("{}: {e}", file.name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Format;

    fn file() -> OutputFile {
        OutputFile {
            format: Format::Csv,
            name: "a.csv".to_owned(),
            mime: "text/csv; charset=utf-8",
            bytes: b"x".to_vec(),
        }
    }

    #[test]
    fn folder_deliveries_stay_inside_the_output_folder() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        write_to_folder(dir.path(), "a/b", &[file()])?;
        assert!(dir.path().join("a/b/a.csv").exists());
        assert!(write_to_folder(dir.path(), "../x", &[file()]).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_link_cannot_lead_out_of_the_output_folder() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let outside = tempfile::tempdir().map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).map_err(|e| e.to_string())?;
        assert!(write_to_folder(dir.path(), "link", &[file()]).is_err());
        assert!(!outside.path().join("a.csv").exists());
        Ok(())
    }

    #[test]
    fn messages_carry_the_files_and_keep_headers_safe() -> Result<(), String> {
        let from: Mailbox = "Reports <r@example.com>".parse().map_err(|_| "from")?;
        let mail = Mail {
            to: vec!["Ana <ana@example.com>".to_owned(), "b@example.com".to_owned()],
            subject: "Oi\r\nBcc: evil@example.com".to_owned(),
            body: "Segue".to_owned(),
            attachments: vec![file()],
        };
        let message = build_message(&from, &mail)?;
        let raw = String::from_utf8_lossy(&message.formatted()).into_owned();
        assert!(raw.contains("a.csv"));
        assert!(!raw.contains("\r\nBcc: evil"), "{raw}");
        assert_eq!(message.envelope().to().len(), 2);
        Ok(())
    }

    #[test]
    fn a_bad_address_is_an_error() {
        let from: Result<Mailbox, _> = "r@example.com".parse();
        let Ok(from) = from else { return };
        let mail = Mail {
            to: vec!["nope".to_owned()],
            subject: String::new(),
            body: String::new(),
            attachments: vec![],
        };
        assert!(build_message(&from, &mail).is_err());
    }
}
