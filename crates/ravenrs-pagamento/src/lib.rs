//! # ravenrs-pagamento: BETA, PROOF OF CONCEPT
//!
//! **Never tested against a real provider, a real pinpad, a real card or a real SEFAZ. It has no
//! homologation or certificate from anyone.** See `docs/PAGAMENTOS.md` for the origin of each rule,
//! each provider's homologation and what this proves and does not prove.
//!
//! Electronic payments for a Brazilian point of sale, with no GUI. [`Pagamentos`] holds a
//! [`ProvedorPagamento`] and a [`Diario`], writes before it acts and settles what a power loss left
//! open. The core compiles for `wasm32`; real files, processes, network and serial ports do not.
//!
//! This crate never asks for a PIN and never captures a card. On a real provider that stays with
//! the provider's own software.
//!
//! ```
//! use ravenrs_pagamento::simulador::SimuladorProvedor;
//! use ravenrs_pagamento::{
//!     DiarioMemoria, Estado, IdPagamento, Impressao, Pagamentos, Passo, PedidoPagamento, TipoPagamento,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut pagamentos = Pagamentos::new(SimuladorProvedor::new(), DiarioMemoria::new())?;
//! pagamentos.recuperar()?;
//!
//! let id = IdPagamento::new("venda-1")?;
//! let pedido = PedidoPagamento::new(id.clone(), 1990, TipoPagamento::Debito)?;
//! let Passo::Aprovado(aprovacao) = pagamentos.iniciar(pedido)? else { panic!("o padrão aprova") };
//!
//! // Issue the fiscal document with `aprovacao.dados_fiscais`, print `aprovacao.vias`, and only then:
//! assert_eq!(pagamentos.concluir(&id, Impressao::Concluida)?, Estado::Confirmado);
//! # Ok(())
//! # }
//! ```

pub mod abecs;
pub mod auxiliar;
mod diario;
mod dominio;
mod fiscal;
pub mod http;
#[cfg(not(target_arch = "wasm32"))]
pub mod intpos;
pub mod mercado_pago;
mod pagamentos;
mod provedor;
mod relogio;
pub mod simulador;
pub mod sitef;
pub mod stone;

#[cfg(not(target_arch = "wasm32"))]
pub use diario::DiarioArquivo;
pub use diario::{Acao, Conteudo, Diario, DiarioMemoria, ErroDiario, Lancamento, Registro};
pub use dominio::{
    Aprovacao, AvisoAprovacao, Capacidades, EntradaOperador, ErroDominio, Estado, Evento, IdPagamento,
    OpcaoMenu, Passo, PedidoPagamento, Pergunta, ProvedorId, ReferenciaTransacao, TAMANHO_MAXIMO_ID,
    TipoCampo, TipoPagamento, Vias, transitar,
};
pub use fiscal::{
    AlertaFiscal, BANDEIRAS, DadosFiscaisPagamento, LinhaBandeira, LinhaRede, REDES, TpIntegra, cnpj_da_rede,
    cnpj_valido, t_band_da_bandeira,
};
pub use pagamentos::{Desfecho, ErroPagamento, Impressao, Pagamentos, Recuperacao};
pub use provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};
#[cfg(not(target_arch = "wasm32"))]
pub use relogio::RelogioSistema;
pub use relogio::{Relogio, RelogioManual};
