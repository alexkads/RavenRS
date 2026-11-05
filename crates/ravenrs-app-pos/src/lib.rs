//! **BETA, proof of concept.** A point of sale as an application module: products, a checkout used
//! from the keyboard and a scanner, sales kept to pick up later, shelf labels and receipts made in
//! the standard report designer. The rules are in `ravenrs-pos`, and issuing in `ravenrs-app-nfe`.

mod caixa;
mod etiquetas;
mod modelo;
mod modulo;
mod telas;

pub use caixa::{Caixa, Tela};
pub use modelo::{CONFIGURACAO, PRODUTO, configuracao_entity, entidades, produto_entity};
pub use modulo::{Pos, VIEW_CAIXA, VIEW_ETIQUETAS, VIEW_INICIO};
