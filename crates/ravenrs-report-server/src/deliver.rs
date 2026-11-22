//! Sends a run's files: by email, or to a folder under the server's output folder.
//!
//! The email itself is `ravenrs-mail`, shared with the other services that send email; this
//! module reads the server's [`SmtpConfig`], with the credentials from the environment, and keeps
//! the delivery to a folder.

use std::fmt;
use std::path::Path;

pub use ravenrs_mail::{Mail, MailAttachment, Mailer};

use crate::config::{SmtpConfig, SmtpSecurity};
use crate::job::safe_relative_path;
use crate::render::OutputFile;

/// Sends email through the SMTP server of the configuration.
pub struct SmtpMailer(ravenrs_mail::SmtpMailer);

impl fmt::Debug for SmtpMailer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl SmtpMailer {
    /// Prepares the connection; credentials come from the environment variables the
    /// configuration names.
    pub fn new(config: &SmtpConfig) -> Result<Self, String> {
        let from_env = |variable: &str| {
            std::env::var(variable).map_err(|_| format!("the environment variable {variable} is not set"))
        };
        let mut settings =
            ravenrs_mail::SmtpSettings::new(&config.host, &config.from).security(match config.security {
                SmtpSecurity::Starttls => ravenrs_mail::Security::Starttls,
                SmtpSecurity::Tls => ravenrs_mail::Security::Tls,
                SmtpSecurity::None => ravenrs_mail::Security::None,
            });
        if let Some(port) = config.port {
            settings = settings.port(port);
        }
        if let (Some(username), Some(password)) = (&config.username_env, &config.password_env) {
            settings = settings.credentials(from_env(username)?, from_env(password)?);
        }
        ravenrs_mail::SmtpMailer::new(&settings).map(Self)
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, mail: &Mail) -> Result<(), String> {
        self.0.send(mail)
    }
}

/// The files of a run, as attachments of a message.
pub fn attachments(files: &[OutputFile]) -> Vec<MailAttachment> {
    files.iter().map(|file| MailAttachment::new(file.name.clone(), file.mime, file.bytes.clone())).collect()
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
}
