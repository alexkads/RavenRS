//! A raw connection for the tests that set tables up by hand, opened as the crate opens its own:
//! TLS with the public authorities and the one in `RAVENRS_POSTGRES_CA`, unless the address says
//! `sslmode=disable`. Hosted databases (Supabase, Neon) refuse a connection without TLS.

use std::sync::Arc;

use postgres::config::SslMode;
use postgres::{Client, Config, NoTls};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use tokio_postgres_rustls::MakeRustlsConnect;

/// The address without what only the crate reads (`sslrootcert`, `pgbouncer`).
fn plain(address: &str) -> String {
    let Some((base, query)) = address.split_once('?') else { return address.to_owned() };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| !pair.starts_with("sslrootcert=") && !pair.starts_with("pgbouncer="))
        .collect();
    if kept.is_empty() { base.to_owned() } else { format!("{base}?{}", kept.join("&")) }
}

/// A connection to `address`, with TLS unless it says `sslmode=disable`.
pub fn raw(address: &str) -> Client {
    let config: Config = plain(address).parse().expect("a PostgreSQL address");
    if config.get_ssl_mode() == SslMode::Disable {
        return config.connect(NoTls).expect("connects");
    }
    let mut roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    if let Some(file) = std::env::var("RAVENRS_POSTGRES_CA").ok().filter(|f| !f.is_empty()) {
        let pem = std::fs::read(&file).expect("the certificate authority");
        for certificate in CertificateDer::pem_slice_iter(&pem) {
            roots.add(certificate.expect("a certificate")).expect("added");
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.connect(MakeRustlsConnect::new(tls)).expect("connects")
}
