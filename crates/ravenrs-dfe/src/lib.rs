//! **BETA, proof of concept.** What every Brazilian electronic fiscal document shares, for the
//! engines of RavenRS (`ravenrs-nfe`, `ravenrs-nfse`) and whatever comes next: a small XML tree,
//! inclusive C14N, enveloped XMLDSig with SHA-1 or SHA-256, the [`Transport`] every request goes
//! through, the MSW-style chain of handlers of the sandboxes, gzip and Base64, reading and issuing
//! test ICP-Brasil certificates (A1 and a simulated A3 token) and, with the `http` feature on native
//! targets, mutual TLS with an A1 certificate.
//!
//! Nothing here knows a layout: each engine says which element is signed, with which hash, and what
//! a rejection means. Not homologated by any authority; see the README of each engine.

pub mod assinatura;
pub mod c14n;
pub mod certificado_x509;
pub mod certificados;
pub mod compactacao;
mod erro;
pub mod interceptacao;
pub mod tempo;
pub mod transporte;
pub mod xml;

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub mod http;

pub use assinatura::{Algoritmo, FalhaAssinatura, HashAssinatura, RsaSigner, Signer};
pub use erro::DfeError;
pub use interceptacao::{Cadeia, OnUnhandled};
pub use tempo::{Instante, RelogioControlado};
pub use transporte::{HttpRequest, HttpResponse, Metodo, Transport, TransportError};
