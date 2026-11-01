//! The server's configuration. Secrets are never in the file: it names the
//! environment variables that hold them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// An API token shorter than this is refused at startup.
const MIN_TOKEN_LENGTH: usize = 16;

/// The server's configuration, read from a JSON file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// Address and port to listen on; the loopback interface by default.
    #[serde(default = "default_listen")]
    pub listen: String,
    /// Where layouts, jobs, history and documents are kept.
    pub data_dir: PathBuf,
    /// The environment variable holding the API token.
    #[serde(default = "default_token_env")]
    pub api_token_env: String,
    /// The folder under which `folder` deliveries write; such deliveries are refused without it.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    /// The mail server; `email` deliveries are refused without it.
    #[serde(default)]
    pub smtp: Option<SmtpConfig>,
    /// The hosts `url` data sources may read from. Any other host is refused.
    #[serde(default)]
    pub data_hosts: Vec<DataHost>,
    /// How many reports render at the same time.
    #[serde(default = "default_concurrency")]
    pub max_concurrent_renders: usize,
    /// The wait before the first retry of a failed run; it doubles on each retry.
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,
    /// A run this late (the server was down) still happens; a later one is recorded as missed.
    #[serde(default = "default_misfire_grace")]
    pub misfire_grace_minutes: u64,
    /// How many runs of each job the history keeps.
    #[serde(default = "default_history")]
    pub history_limit: usize,
    /// The largest request body accepted, in megabytes.
    #[serde(default = "default_body_limit")]
    pub max_body_megabytes: usize,
}

/// How to reach the mail server.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmtpConfig {
    /// Host name.
    pub host: String,
    /// Port; the usual one for `security` when absent.
    #[serde(default)]
    pub port: Option<u16>,
    /// How the connection is protected.
    #[serde(default)]
    pub security: SmtpSecurity,
    /// The sender of every message, such as `Reports <reports@example.com>`.
    pub from: String,
    /// The environment variable holding the user name, when the server asks for one.
    #[serde(default)]
    pub username_env: Option<String>,
    /// The environment variable holding the password.
    #[serde(default)]
    pub password_env: Option<String>,
}

/// How the connection to the mail server is protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmtpSecurity {
    /// Upgrades to TLS, and fails if the server cannot (port 587).
    #[default]
    Starttls,
    /// TLS from the start (port 465).
    Tls,
    /// No encryption: only for a relay on the same machine or a local test server.
    None,
}

/// A host `url` data sources may read from.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataHost {
    /// The host name, exactly as in the URL.
    pub host: String,
    /// Headers sent to this host: header name to the environment variable holding its value.
    #[serde(default)]
    pub headers_env: BTreeMap<String, String>,
}

fn default_listen() -> String {
    "127.0.0.1:8090".to_owned()
}

fn default_token_env() -> String {
    "RAVENRS_SERVER_TOKEN".to_owned()
}

fn default_concurrency() -> usize {
    4
}

fn default_retry_delay() -> u64 {
    60
}

fn default_misfire_grace() -> u64 {
    60
}

fn default_history() -> usize {
    1000
}

fn default_body_limit() -> usize {
    32
}

impl Config {
    /// A configuration with every default, keeping its data in `data_dir`.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            listen: default_listen(),
            data_dir: data_dir.into(),
            api_token_env: default_token_env(),
            output_dir: None,
            smtp: None,
            data_hosts: Vec::new(),
            max_concurrent_renders: default_concurrency(),
            retry_delay_seconds: default_retry_delay(),
            misfire_grace_minutes: default_misfire_grace(),
            history_limit: default_history(),
            max_body_megabytes: default_body_limit(),
        }
    }

    /// Reads the configuration from a JSON file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The API token, from the environment. There is no way to run without one.
    pub fn api_token(&self) -> Result<String, String> {
        let token = std::env::var(&self.api_token_env)
            .map_err(|_| format!("set the API token in the environment variable {}", self.api_token_env))?;
        if token.len() < MIN_TOKEN_LENGTH {
            return Err(format!("the API token must have at least {MIN_TOKEN_LENGTH} characters"));
        }
        Ok(token)
    }
}
