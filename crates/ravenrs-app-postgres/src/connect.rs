//! A connection opened from the address a database's panel gives: `postgresql://…?sslmode=require`,
//! as Supabase, Neon and Nhost hand it out, or the `key=value` form.
//!
//! - TLS is rustls with the Mozilla roots, and the server's certificate is always checked.
//!   `sslrootcert=<file.pem>` adds a certificate authority, as Supabase's direct connection needs.
//! - A pooler in transaction mode (Supavisor on 6543, Neon's `-pooler` host, or `pgbouncer=true`)
//!   is recognised, and statements then go unprepared.
//! - The address is kept, so a connection that dropped outside a transaction is opened again.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use postgres::config::{Host, SslMode};
use postgres::{Client, Config, NoTls};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use tokio_postgres_rustls::MakeRustlsConnect;

/// Long enough for a database that was asleep, as Neon's is after a while with nothing to do.
const WAKING_UP: Duration = Duration::from_secs(15);

/// How statements reach the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statements {
    /// Each statement is prepared once on the connection and kept. The quickest, when the
    /// connection is the server's own.
    Prepared,
    /// Each statement goes in one round trip and keeps no name on the server, so a pooler in
    /// transaction mode can hand every one to whichever server connection is free.
    Unprepared,
}

/// What [`PostgresBackend::connect_with`](crate::PostgresBackend::connect_with) can be told
/// besides the address.
#[derive(Clone, Debug, Default)]
pub struct PostgresOptions {
    statements: Option<Statements>,
    connect_timeout: Option<Duration>,
    roots: Vec<Vec<u8>>,
}

impl PostgresOptions {
    /// Nothing besides the address.
    pub fn new() -> Self {
        Self::default()
    }

    /// How statements go, whatever the address looks like.
    pub fn statements(mut self, statements: Statements) -> Self {
        self.statements = Some(statements);
        self
    }

    /// How long to wait for the connection when the address does not say (`connect_timeout`).
    pub fn connect_timeout(mut self, wait: Duration) -> Self {
        self.connect_timeout = Some(wait);
        self
    }

    /// A certificate authority to trust besides the usual ones, in PEM.
    pub fn root_certificate_pem(mut self, pem: impl Into<Vec<u8>>) -> Self {
        self.roots.push(pem.into());
        self
    }
}

/// Why a connection could not be opened. The address itself, with its password, is never in it.
#[derive(Debug)]
pub enum ConnectError {
    /// The address could not be read.
    Address(String),
    /// A certificate authority could not be read.
    Certificate(String),
    /// The server could not be reached, refused the login or its certificate.
    Connect(postgres::Error),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(why) => write!(f, "the database address could not be read: {why}"),
            Self::Certificate(why) => write!(f, "a certificate authority could not be read: {why}"),
            Self::Connect(error) => write!(f, "the database could not be reached: {error}"),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connect(error) => Some(error),
            _ => None,
        }
    }
}

/// What it takes to open the same connection again.
#[derive(Clone)]
pub(crate) struct Reopen {
    config: Config,
    tls: Option<MakeRustlsConnect>,
}

impl Reopen {
    pub(crate) fn open(&self) -> Result<Client, postgres::Error> {
        match &self.tls {
            Some(tls) => self.config.connect(tls.clone()),
            None => self.config.connect(NoTls),
        }
    }
}

/// What an address says, once the options only this crate knows are taken out of it.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Address {
    pub(crate) rest: String,
    pub(crate) root_file: Option<String>,
    pub(crate) pgbouncer: bool,
}

/// Turns `%2F` and the like back into what they stand for.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes.get(at + 1..at + 3).and_then(|pair| std::str::from_utf8(pair).ok());
        match (bytes[at], hex.and_then(|pair| u8::from_str_radix(pair, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The words of a `key=value` address, quotes kept as they were.
fn words(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = None;
    let mut quoted = false;
    let mut escaped = false;
    for (at, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '\'' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if let Some(from) = start.take() {
                    found.push(&text[from..at]);
                }
                continue;
            }
            _ => {}
        }
        if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(from) = start {
        found.push(&text[from..]);
    }
    found
}

/// Takes `sslrootcert` and `pgbouncer` out, since the driver refuses options it does not know.
pub(crate) fn read_address(address: &str) -> Address {
    let mut read = Address::default();
    let mut take = |key: &str, value: String| match key {
        "sslrootcert" => {
            read.root_file = Some(value);
            true
        }
        "pgbouncer" => {
            read.pgbouncer = value.eq_ignore_ascii_case("true");
            true
        }
        _ => false,
    };
    let trimmed = address.trim();
    if trimmed.starts_with("postgres://") || trimmed.starts_with("postgresql://") {
        let Some((base, query)) = trimmed.split_once('?') else {
            return Address { rest: trimmed.to_owned(), ..Address::default() };
        };
        let kept: Vec<&str> = query
            .split('&')
            .filter(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                !take(key, unescape(value))
            })
            .collect();
        let rest = if kept.is_empty() { base.to_owned() } else { format!("{base}?{}", kept.join("&")) };
        return Address { rest, ..read };
    }
    let kept: Vec<&str> = words(trimmed)
        .into_iter()
        .filter(|word| {
            let (key, value) = word.split_once('=').unwrap_or((word, ""));
            !take(key.trim(), value.trim().trim_matches('\'').to_owned())
        })
        .collect();
    Address { rest: kept.join(" "), ..read }
}

/// Whether the host is a pooler in transaction mode: Neon's `-pooler` endpoints and Supavisor's
/// transaction port.
pub(crate) fn behind_pooler(config: &Config) -> bool {
    let ports = config.get_ports();
    config.get_hosts().iter().enumerate().any(|(at, host)| {
        let Host::Tcp(name) = host else { return false };
        let name = name.to_ascii_lowercase();
        let port = ports.get(at).or_else(|| ports.first()).copied().unwrap_or(5432);
        let first = name.split('.').next().unwrap_or("");
        first.ends_with("-pooler") || (name.ends_with(".pooler.supabase.com") && port == 6543)
    })
}

fn tls(options: &PostgresOptions, root_file: Option<&str>) -> Result<MakeRustlsConnect, ConnectError> {
    let mut roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let mut pems = options.roots.clone();
    if let Some(file) = root_file {
        let read =
            std::fs::read(file).map_err(|error| ConnectError::Certificate(format!("{file}: {error}")))?;
        pems.push(read);
    }
    for pem in &pems {
        let mut any = false;
        for certificate in CertificateDer::pem_slice_iter(pem) {
            let certificate = certificate.map_err(|error| ConnectError::Certificate(error.to_string()))?;
            roots.add(certificate).map_err(|error| ConnectError::Certificate(error.to_string()))?;
            any = true;
        }
        if !any {
            return Err(ConnectError::Certificate("no certificate in it".to_owned()));
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| ConnectError::Certificate(error.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(MakeRustlsConnect::new(config))
}

/// Reads the address and works out how to reach it and how statements should go.
pub(crate) fn prepare(
    address: &str,
    options: &PostgresOptions,
) -> Result<(Reopen, Statements), ConnectError> {
    let read = read_address(address);
    let mut config = Config::from_str(&read.rest).map_err(|error| {
        // "invalid connection string" alone does not say which part; the cause names the option.
        let cause = std::error::Error::source(&error).map(ToString::to_string);
        ConnectError::Address(cause.unwrap_or_else(|| error.to_string()))
    })?;
    if config.get_connect_timeout().is_none() {
        config.connect_timeout(options.connect_timeout.unwrap_or(WAKING_UP));
    }
    let statements = options.statements.unwrap_or(if read.pgbouncer || behind_pooler(&config) {
        Statements::Unprepared
    } else {
        Statements::Prepared
    });
    let tls = match config.get_ssl_mode() {
        SslMode::Disable => None,
        _ => Some(tls(options, read.root_file.as_deref())?),
    };
    Ok((Reopen { config, tls }, statements))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn its_own_options_come_out_of_an_address() {
        let read = read_address(
            "postgresql://app:se%2Fcret@db.example.com:5432/app?sslmode=require&sslrootcert=%2Ftmp%2Fca.pem&pgbouncer=true",
        );
        assert_eq!(read.rest, "postgresql://app:se%2Fcret@db.example.com:5432/app?sslmode=require");
        assert_eq!(read.root_file.as_deref(), Some("/tmp/ca.pem"));
        assert!(read.pgbouncer);

        let read = read_address("host=localhost password='a b' sslrootcert='/tmp/my ca.pem' dbname=app");
        assert_eq!(read.rest, "host=localhost password='a b' dbname=app");
        assert_eq!(read.root_file.as_deref(), Some("/tmp/my ca.pem"));
        assert!(!read.pgbouncer);

        let read = read_address("postgres://localhost/app");
        assert_eq!(read.rest, "postgres://localhost/app");
    }

    fn statements_for(address: &str) -> Statements {
        prepare(address, &PostgresOptions::new()).unwrap().1
    }

    #[test]
    fn a_pooler_in_transaction_mode_gets_statements_unprepared() {
        let neon =
            "postgresql://u:p@ep-cool-darkness-123456-pooler.us-east-2.aws.neon.tech/neondb?sslmode=require";
        assert_eq!(statements_for(neon), Statements::Unprepared);
        let direct =
            "postgresql://u:p@ep-cool-darkness-123456.us-east-2.aws.neon.tech/neondb?sslmode=require";
        assert_eq!(statements_for(direct), Statements::Prepared);

        let supavisor = "postgresql://postgres.abc:p@aws-0-sa-east-1.pooler.supabase.com:6543/postgres";
        assert_eq!(statements_for(supavisor), Statements::Unprepared);
        let session = "postgresql://postgres.abc:p@aws-0-sa-east-1.pooler.supabase.com:5432/postgres";
        assert_eq!(statements_for(session), Statements::Prepared);

        assert_eq!(statements_for("postgresql://localhost:6432/app?pgbouncer=true"), Statements::Unprepared);
        let told = prepare(neon, &PostgresOptions::new().statements(Statements::Prepared)).unwrap().1;
        assert_eq!(told, Statements::Prepared);
    }

    #[test]
    fn an_address_that_cannot_be_read_says_why_without_the_password() {
        let Err(error) =
            prepare("postgresql://app:hunter2@localhost/app?colour=blue", &PostgresOptions::new())
        else {
            panic!("an unknown option should be refused")
        };
        let said = error.to_string();
        assert!(!said.contains("hunter2"), "{said}");
        assert!(said.contains("colour"), "{said}");
    }

    #[test]
    fn a_file_with_no_certificate_is_refused() {
        let options = PostgresOptions::new().root_certificate_pem(b"not a certificate".to_vec());
        let Err(ConnectError::Certificate(_)) =
            prepare("postgresql://localhost/app?sslmode=require", &options)
        else {
            panic!("an empty authority should be refused")
        };
    }

    #[test]
    fn with_no_wait_given_there_is_time_to_wake_up() {
        let (reopen, _) = prepare("postgresql://localhost/app", &PostgresOptions::new()).unwrap();
        assert_eq!(reopen.config.get_connect_timeout(), Some(&WAKING_UP));
        let (reopen, _) =
            prepare("postgresql://localhost/app?connect_timeout=3", &PostgresOptions::new()).unwrap();
        assert_eq!(reopen.config.get_connect_timeout(), Some(&Duration::from_secs(3)));
    }
}
