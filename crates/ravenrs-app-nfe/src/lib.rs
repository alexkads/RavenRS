//! **BETA, proof of concept.** A module that issues NF-e and NFC-e from a RavenRS application. It is
//! not homologated with any SEFAZ and cannot issue in production: everyone using an application with
//! it sees a notice that cannot be dismissed, and nothing it issues has fiscal value.
//!
//! The rules of the document are in `ravenrs-nfe`, which knows nothing about applications. This
//! crate keeps what was issued as records, runs the sending out of the way of the screens and makes
//! the receipt a report people can change in the standard designer.
//!
//! The NFS-e of the national standard comes through `ravenrs-nfse` on the same service: the DPS is
//! kept before it goes, and one that never got a sure answer goes again as it is.

mod cupom;
mod danfse;
mod module;
mod service;
mod servico;

pub use cupom::{
    CAMPOS_OBRIGATORIOS, CUPOM_LINHA, MODELO_DANFE, campos_faltando, cupom_entity, cupom_report,
    danfe_report, linhas_do_documento, modelos_de_cupom, modelos_de_danfe, renderizar_cupom,
};
pub use danfse::{
    CAMPOS_OBRIGATORIOS_DANFSE, MODELO_DANFSE, NFSE_LINHA, campo_da_danfse, campos_faltando_danfse,
    danfse_entity, danfse_report, linha_da_nfse, modelos_de_danfse, preparar_danfse, renderizar_danfse,
};
pub use module::{NFE_DOCUMENTO, NFE_EMITENTE, NfeModule, documento_entity, emitente_entity};
pub use ravenrs_nfe as engine;
/// The NFS-e engine, beta like the NF-e.
pub use ravenrs_nfse as nfse;
pub use service::{EmissaoFinal, NfeService, TextoVenda, emitente_do_registro, emitente_salvo_de, uf_de};
pub use servico::{
    AUTORIZADA, CANCELADA, ENVIANDO, NFSE_DOCUMENTO, NfseFinal, PENDENTE, REJEITADA, ServicoVendido,
    documento_danfse, dps_do_registro, inscricao_de, linha_danfse_do_registro, mostrar_danfse, nfse_entity,
    nfses_pendentes, prestador_do_emitente,
};
