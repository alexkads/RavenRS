//! Peripherals as data. A driver is a signed JSON manifest that says exactly which device model it is
//! for, how it is connected, what protocol it speaks and which few bytes differ on that model. It
//! never carries code, so an update can only change values this crate has checked.
//!
//! [`Registro`] holds the installed versions and which one is active. A [`Catalogo`] offers newer
//! ones. Nothing updates by itself: a person accepts an [`Atualizacao`] and can go back.
//!
//! **No driver here was tested on a real device.** [`Situacao`] says so for each one.

mod armazem;
mod catalogo;
mod comando;
mod data;
mod embutidos;
mod manifesto;
mod registro;
mod transporte;

#[cfg(not(target_arch = "wasm32"))]
pub use armazem::ArmazemArquivo;
pub use armazem::{Armazem, ArmazemMemoria, ErroArmazem};
pub use catalogo::{
    Atualizacao, Catalogo, ChavePublica, ErroCatalogo, ItemCatalogo, assinar, chave_embutida,
    chave_publica_de, verificar_atualizacoes,
};
pub use comando::Comando;
#[cfg(not(target_arch = "wasm32"))]
pub use data::hoje_utc;
pub use embutidos::manifestos_embutidos;
pub use manifesto::{
    Ajustes, Capacidades, ErroManifesto, Manifesto, Modelo, NomeProtocolo, Padroes, Protocolo, Situacao,
    TipoConexao, TipoPeriferico,
};
pub use registro::{AcaoHistorico, Dispositivo, ErroRegistro, Evento, Registro};
pub use semver::Version;
#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use transporte::TransporteHttp;
pub use transporte::{ErroTransporte, TransporteCatalogo};
