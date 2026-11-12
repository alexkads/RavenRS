//! The serial protocol of the Brazilian pinpad, as defined by ABECS, version 2.20.
//!
//! In layers: [`quadro`] (packets and CRC) and [`comandos`] (commands and responses) do no input or
//! output; [`sessao`] adds the flow of the link with time passed in as a value; [`driver`] offers
//! calls over any [`PortaPinpad`]; [`simulado`] is a pinpad in memory and, with the `serial`
//! feature on native targets, `serial` is a real port.
//!
//! This is what a point-of-sale software may do with no key from an acquirer: show text and images,
//! ask for a value from the fixed table, offer a menu, read a key. **There is no PIN capture and no
//! card reading here, on purpose.** Those need the keys and tables of an acquirer network, and on a
//! real checkout they stay with the TEF software, which also owns the port while it runs.
//!
//! Written from the public specification and checked against its packet examples. It was never
//! connected to a real pinpad. The specification carries a notice restricting copies, so this crate
//! does not embed it nor its long text tables; see `docs/PAGAMENTOS.md`.

pub mod comandos;
pub mod driver;
pub mod quadro;
#[cfg(all(feature = "serial", not(target_arch = "wasm32")))]
pub mod serial;
pub mod sessao;
pub mod simulado;

pub use comandos::{
    Comando, ErroComando, InformacaoGeral, MensagemGcd, RespostaAbecs, StatusAbecs, TipoMidia,
};
pub use driver::{ErroPinpad, InformacaoEstendida, Pinpad, PortaPinpad};
pub use quadro::{Decodificador, EventoQuadro, codificar_pacote, crc16};
pub use sessao::{EventoSessao, FalhaEnlace, Sessao};
pub use simulado::{PinpadSimulado, PortaSimulada};
