//! # ravenrs-nfe: BETA, PROOF OF CONCEPT
//!
//! **Not homologated. Must not issue real fiscal documents. Production is blocked by design:**
//! [`Ambiente`] has no production variant and no code path here writes `tpAmb=1`. An emitter only
//! exists with [`BetaAck::i_understand_this_is_experimental`], and every document carries a
//! sentence in `infCpl` identifying it as a beta. Nothing about this crate has been validated
//! against a real SEFAZ server. See `REGRAS.md` for the origin of each rule and what has no source.
//!
//! NF-e and NFC-e with no GUI: draft, totals, access key, deterministic XML, inclusive C14N,
//! XMLDSig, SOAP 1.2 and the reading of the returns. Limits of this POC: only Simples Nacional
//! (CRT 1, CSOSN 102 and 500), only synchronous authorization, models 55 and 65, no voiding of
//! number ranges, no contingency, no DANFE. The sandbox also explains failures and reproduces
//! field problems offline: see [`sandbox`].
//!
//! Everything that leaves goes through [`Transport`]. [`sandbox::SefazSandbox`] replaces it with a
//! local SEFAZ that really checks what arrives; the emitter does not know the difference.
//!
//! ```
//! use std::str::FromStr;
//! use ravenrs_nfe::sandbox::{SandboxSigner, SefazSandbox};
//! use ravenrs_nfe::{
//!     Ambiente, BetaAck, Csosn, Decimal, Emissor, Emitente, Endereco, FormaPagamento, Item,
//!     Modelo, Rascunho, Situacao, Uf,
//! };
//!
//! # fn main() -> Result<(), ravenrs_nfe::NfeError> {
//! let emitente = Emitente {
//!     cnpj: "11222333000181".into(),
//!     razao_social: "MERCADO DE TESTE LTDA".into(),
//!     nome_fantasia: None,
//!     inscricao_estadual: "123456789012".into(),
//!     endereco: Endereco {
//!         logradouro: "RUA DAS FLORES".into(),
//!         numero: "100".into(),
//!         bairro: "CENTRO".into(),
//!         codigo_municipio: "3550308".into(),
//!         municipio: "SAO PAULO".into(),
//!         uf: Uf::SP,
//!         cep: "01001000".into(),
//!     },
//! };
//! let server = SefazSandbox::new();
//! let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
//!     .ambiente(Ambiente::Sandbox)
//!     .emitente(emitente)
//!     .signer(SandboxSigner::new()?)
//!     .transport(server.clone())
//!     .build()?;
//!
//! let preco = Decimal::from_str("7.90").expect("decimal");
//! let rascunho = Rascunho::novo(Modelo::Nfce65, 1, 1, "VENDA", "2026-11-05T14:30:00-03:00")
//!     .com_item(Item {
//!         codigo: "1".into(),
//!         gtin: None,
//!         descricao: "ARROZ".into(),
//!         ncm: "10063021".into(),
//!         cest: None,
//!         cfop: "5102".into(),
//!         unidade: "UN".into(),
//!         quantidade: Decimal::ONE,
//!         valor_unitario: preco,
//!         desconto: Decimal::ZERO,
//!         csosn: Csosn::S102,
//!         origem: 0,
//!     })
//!     .com_pagamento(FormaPagamento::Dinheiro, preco);
//!
//! let emissao = emissor.emitir(&rascunho)?;
//! assert_eq!(emissao.autorizacao.situacao(), Situacao::Autorizada);
//! assert!(emissao.autorizacao.x_motivo().starts_with("[SANDBOX] "));
//! # Ok(())
//! # }
//! ```

pub mod ambiente;
pub mod chave;
pub mod documento;
mod emissor;
pub mod entrada;
mod error;
pub mod modelo;
pub mod retorno;
pub mod sandbox;
pub mod soap;
pub mod totais;
pub mod urls;

// The pieces every fiscal document shares live in `ravenrs-dfe`; they stay reachable from here.
pub use ravenrs_dfe::{DfeError, assinatura, c14n, transporte, xml};

/// Mutual TLS with an A1 certificate, from `ravenrs-dfe`.
#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use ravenrs_dfe::http;

pub use ambiente::{Ambiente, BetaAck, Modelo, Origem, Uf};
pub use assinatura::{FalhaAssinatura, HashAssinatura, RsaSigner, Signer};
pub use chave::ChaveAcesso;
pub use documento::{QrCode, TipoManifestacao};
pub use emissor::{Cancelamento, Documento, Emissao, Emissor, EmissorBuilder};
pub use error::NfeError;
pub use modelo::{
    Cartao, Csosn, Destinatario, DocumentoDestinatario, Emitente, Endereco, FormaPagamento, Item, Pagamento,
    Rascunho, ResponsavelTecnico, TipoIntegracao,
};
pub use retorno::{
    Autorizacao, ConsultaProtocolo, DocumentoDfe, LoteDfe, Protocolo, Retorno, RetornoEvento, Situacao,
    StatusServico,
};
pub use rust_decimal::Decimal;
pub use soap::Servico;
pub use totais::Totais;
pub use transporte::{HttpRequest, HttpResponse, Transport, TransportError};
