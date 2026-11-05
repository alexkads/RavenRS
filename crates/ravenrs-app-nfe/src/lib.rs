//! **BETA, proof of concept.** A module that issues NF-e and NFC-e from a RavenRS application. It is
//! not homologated with any SEFAZ and cannot issue in production: everyone using an application with
//! it sees a notice that cannot be dismissed, and nothing it issues has fiscal value.
//!
//! The rules of the document are in `ravenrs-nfe`, which knows nothing about applications. This
//! crate keeps what was issued as records, runs the sending out of the way of the screens and makes
//! the receipt a report people can change in the standard designer.

mod cupom;
mod module;
mod service;

pub use cupom::{
    CAMPOS_OBRIGATORIOS, CUPOM_LINHA, MODELO_DANFE, campos_faltando, cupom_entity, cupom_report,
    danfe_report, linhas_do_documento, modelos_de_cupom, modelos_de_danfe, renderizar_cupom,
};
pub use module::{NFE_DOCUMENTO, NFE_EMITENTE, NfeModule, documento_entity, emitente_entity};
pub use ravenrs_nfe as engine;
pub use service::{EmissaoFinal, NfeService, TextoVenda, emitente_do_registro, emitente_salvo_de, uf_de};
