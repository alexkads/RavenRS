//! **BETA, proof of concept.** A point of sale as an application module: products, a checkout used
//! from the keyboard and a scanner, sales kept to pick up later, shelf labels and receipts made in
//! the standard report designer. The rules are in `ravenrs-pos`, and issuing in `ravenrs-app-nfe`.

mod caixa;
mod codigos;
mod entrada;
mod etiquetas;
mod modelo;
mod modulo;
mod preco;
mod telas;

pub use caixa::{Caixa, Tela};
pub use codigos::{Encontrado, cadastrar_codigo, codigos_do_produto, produto_por_codigo};
pub use entrada::{Entradas, Previa, criar_codigo_de_barras};
pub use modelo::{
    CLIENTE, CODIGO, CONFIGURACAO, MOVIMENTO, PRODUTO, VINCULO, cliente_entity, codigo_entity,
    configuracao_entity, entidades, movimento_entity, produto_entity, vinculo_entity,
};
pub use modulo::{Pos, VIEW_CAIXA, VIEW_ENTRADA, VIEW_ETIQUETAS, VIEW_INICIO, VIEW_PRECO};
