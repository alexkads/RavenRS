//! **BETA, proof of concept. Not homologated. Production is blocked.**
//!
//! NFS-e of the national standard (Padrão Nacional): the service provider writes a DPS, signs it,
//! and the Sefin Nacional answers with the NFS-e. It is not issued through the city halls any more,
//! and not through the SEFAZ: the authority is the CGNFS-e, with the Receita Federal. See
//! `docs/NFSE.md` for the sources, and what is still unconfirmed.
//!
//! The smallest proof of concept:
//! - provider ME/EPP of the Simples Nacional (`opSimpNac=3`, `regApTribSN=1`), `tpAmb=2` only;
//! - one service per DPS, ISS taxable and not withheld, the taker optional;
//! - `POST /nfse`, `HEAD` and `GET /dps/{id}`, `GET /nfse/{chave}`, the cancellation `e101101` and
//!   the agreement of the city;
//! - **recovery after a lost answer**, by the same DPS Id: the part that matters most at a counter.
//!
//! Everything leaves through [`ravenrs_dfe::Transport`]; [`sandbox::SefinSandbox`] intercepts it
//! the way MSW does, and checks what the Sefin would check.
//!
//! ```compile_fail,E0599
//! let producao = ravenrs_nfse::Ambiente::Producao;
//! ```
//!
//! ```
//! use ravenrs_nfse::sandbox::SefinSandbox;
//! use ravenrs_nfse::{Ambiente, BetaAck, Emissor, exemplo};
//!
//! let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
//!     .ambiente(Ambiente::Sandbox)
//!     .signer(ravenrs_dfe::certificados::certificado_curinga()?)
//!     .transport(SefinSandbox::new())
//!     .build()?;
//! let emitida = emissor.emitir(&exemplo::dps(1))?;
//! assert_eq!(emitida.chave.as_str().len(), 50);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod ambiente;
pub mod chave;
pub mod dps;
mod emissor;
mod erro;
pub mod evento;
pub mod exemplo;
pub mod modelo;
pub mod retorno;
pub mod sandbox;

pub use ambiente::{Ambiente, BetaAck, NS_NFSE, VERSAO};
pub use chave::ChaveNfse;
pub use emissor::{Emissor, EmissorBuilder, Emitida};
pub use erro::{Mensagem, NfseError};
pub use evento::MotivoCancelamento;
pub use modelo::{Dps, Inscricao, Prestador, Servico, Tomador};
pub use rust_decimal::Decimal;
