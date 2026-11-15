//! The steps of the family `ravenrs-widgets-grid`: the page Grade de dados.

use super::*;
use feature_center::widgets::grade::dados::{CUSTOMERS, USES};

#[test]
fn the_data_grid_adds_acts_on_a_row_counts_what_shows_and_asks_a_server() {
    let mut c = Center::memory("ana");
    c.press("Grade de dados");
    c.press("Adicionar");
    assert!(c.says(&format!("{} clientes", CUSTOMERS + 1)), "on_add_row: {:?}", c.texts());
    c.press("Bloquear Cliente 001");
    assert!(c.says("Cliente 001 bloqueado"), "a row action names its row: {:?}", c.texts());
    c.activate(USES[1]);
    c.press("Contar o que a grade mostra");
    assert!(c.says("A grade mostra 30 contas"), "current: {:?}", c.texts());
    c.activate(USES[2]);
    assert!(c.says("Nenhuma conta cadastrada ainda"), "empty_text: {:?}", c.texts());
    c.activate(USES[3]);
    assert!(c.says("O servidor recebeu"), "{:?}", c.texts());
    assert!(!c.says("O servidor recebeu 0 pedidos"), "the remote grids asked: {:?}", c.texts());
}
