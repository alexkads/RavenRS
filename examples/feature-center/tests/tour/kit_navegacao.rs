//! The steps of the pages in src/widgets/kit/navegacao/.

use super::*;
use feature_center::widgets::kit::navegacao::{barras, colecoes, destaques, etapas, lateral, novidades};
use ravenrs_core::{Event, Key, Modifiers};

/// Gives the focus to what is called `name`, as Tab would.
fn focus(c: &mut Center, name: &str) {
    c.settle();
    let tree = c.ui.access_tree();
    let found = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name));
    let id = found.unwrap_or_else(|| panic!("nothing called {name}: {:?}", c.texts())).id;
    c.ui.perform_action(id, AccessAction::Focus);
    c.settle();
}

/// Presses `key` with Shift held.
fn shifted(c: &mut Center, key: Key) {
    c.ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers { shift: true, ..Modifiers::default() } });
    c.settle();
}

#[test]
fn the_navigation_menu_closes_the_path_goes_back_and_the_details_fold() {
    let mut c = Center::memory("ana");
    c.press(lateral::PAGE);
    assert!(c.has_name("Menu da loja"), "label names the landmark: {:?}", c.texts());
    assert!(
        c.says("Raven Loja") && c.says("Matriz"),
        "SidebarBrand with subtitle in the header: {:?}",
        c.texts()
    );
    assert!(c.says("Ana · caixa 2"), "footer: {:?}", c.texts());
    c.press("Clientes");
    assert!(c.says("Menu: Clientes"), "an item of the menu: {:?}", c.texts());
    c.press("Alternar barra lateral");
    assert!(c.says("Menu fechado"), "the trigger shares the state: {:?}", c.texts());
    c.press("Alternar barra lateral");
    assert!(c.says("Menu aberto"), "{:?}", c.texts());

    c.activate("2026");
    assert!(c.says("Caminho: voltou para 2026"), "on_navigate: {:?}", c.texts());
    assert!(!c.has_name("Pedido 1042"), "the path ends where it went back to: {:?}", c.texts());
    c.activate("Loja");
    assert!(c.says("Caminho: voltou para Loja"), "{:?}", c.texts());

    c.activate(lateral::TABS[1]);
    assert!(c.says("3 itens: café, filtro e caneca"), "tab with content: {:?}", c.texts());
    c.activate("Mês");
    assert!(c.says("período: Mês"), "a bar of items only: {:?}", c.texts());
    c.activate("Travar o período");
    c.activate("Dia");
    assert!(c.says("período: Mês"), "disabled keeps the open tab: {:?}", c.texts());

    assert!(c.says("Cartão de crédito em 2 vezes"), "open(true) starts unfolded: {:?}", c.texts());
    assert!(c.has_name("Falta confirmar"), "the mark of a folded one: {:?}", c.texts());
    c.press("Endereço de entrega");
    assert!(c.says("endereço à vista") && c.says("Rua das Flores"), "expanded binding: {:?}", c.texts());
    let tree = c.ui.access_tree();
    let switch =
        tree.nodes.iter().find(|n| n.role == Role::Switch && n.name.as_deref() == Some("Avisos ao cliente"));
    c.ui.perform_action(switch.expect("the switch of the disclosure").id, AccessAction::Click);
    assert!(c.says("avisos desligados"), "switch: {:?}", c.texts());
    assert!(!c.says("Mensagem quando sair"), "the switch does not unfold it: {:?}", c.texts());
}

#[test]
fn the_navigation_menu_changes_variant_side_and_how_it_closes() {
    let mut c = Center::memory("ana");
    c.press(lateral::PAGE);
    c.activate(lateral::VARIANTS[1]);
    c.activate(lateral::COLLAPSES[2]);
    c.activate("Menu do lado direito");
    assert!(c.has_name("Menu da loja"), "floating on the right: {:?}", c.texts());
    c.press("Alternar barra lateral");
    assert!(c.says("Menu fechado"), "{:?}", c.texts());
    assert!(!c.has_name("Clientes"), "hidden takes the menu away: {:?}", c.texts());
}

#[test]
fn the_toolbars_run_commands_move_the_zoom_bar_and_show_the_keys() {
    let mut c = Center::memory("ana");
    c.press(barras::PAGE);
    assert!(c.has_name("Ações do pedido"), "label of the toolbar: {:?}", c.texts());
    c.press("Novo");
    assert!(c.says("Novo pedido: agora são 4") && c.says("4 abertos"), "action and view: {:?}", c.texts());
    c.press("Copiar o pedido");
    assert!(c.says("Pedido copiado"), "accessible_name of an icon_only action: {:?}", c.texts());
    c.press("Exportar");
    assert!(c.says("Lista exportada"), "aligned_end bar: {:?}", c.texts());

    c.press("Aumentar");
    c.press("Aumentar");
    assert!(c.says("Vitrine da loja · 150 %"), "the floating bar over the photo: {:?}", c.texts());
    c.press("Ajustar");
    assert!(c.says("Vitrine da loja · 100 %"), "{:?}", c.texts());
    focus(&mut c, "Mover: Zoom");
    c.key(Key::ArrowLeft);
    assert!(c.says("Barra de zoom solta em"), "movable from the keyboard: {:?}", c.texts());

    c.press("Salvar");
    assert!(c.says("Salvo"), "main command of the split button: {:?}", c.texts());
    c.press("Salvar: Mais");
    c.activate("Salvar como modelo");
    assert!(c.says("Salvo como modelo"), "an alternative of the menu: {:?}", c.texts());

    c.key(Key::Function(4));
    assert!(c.says("F4: desconto"), "a key of the KeyBar: {:?}", c.texts());
    c.activate("Pedido em processamento");
    c.key(Key::Function(8));
    assert!(c.says("F4: desconto"), "a disabled key does nothing: {:?}", c.texts());
    c.press("Excluir");
    assert!(!c.says("Pedido excluído"), "a disabled action does nothing: {:?}", c.texts());

    assert!(c.says("Pedido 3 de 4") && c.says("trocar"), "the hint bar: {:?}", c.texts());
    c.press("Ver atalhos");
    assert!(
        c.says("Atalhos do pedido") && c.says("Nova venda") && c.says("Caixa"),
        "the sheet: {:?}",
        c.texts()
    );
    c.key(Key::Escape);
    assert!(!c.says("Atalhos do pedido"), "Escape closes the sheet: {:?}", c.texts());
}

#[test]
fn the_lists_and_tables_pick_cards_open_one_and_move_a_tile() {
    let mut c = Center::memory("ana");
    c.press(colecoes::PAGE);
    assert!(c.says("Filtro de papel") && c.says("R$ 120,00"), "the rows of the table: {:?}", c.texts());
    c.press("Bruno Reis Bruno Reis 8 pedidos R$ 1.104");
    assert!(c.says("Cliente: Bruno Reis"), "a row of the list: {:?}", c.texts());

    c.activate("Caneca por R$ 24,90");
    c.activate("Bule por R$ 59,90");
    assert!(c.says("Selecionados: Caneca, Bule"), "several selected: {:?}", c.texts());
    c.activate(colecoes::MODES[1]);
    c.activate("Moedor por R$ 31,90");
    assert!(c.says("Selecionados: Moedor"), "one at a time: {:?}", c.texts());
    // The keys go to the list of cards, which keeps the active one.
    let tree = c.ui.access_tree();
    let cards = tree.nodes.iter().find(|n| n.role == Role::List && n.name.as_deref() == Some("Catálogo"));
    c.ui.perform_action(cards.expect("the list of cards").id, AccessAction::Focus);
    c.key(Key::Enter);
    assert!(c.says("Aberto: Moedor"), "on_open with Enter: {:?}", c.texts());

    focus(&mut c, "Mover Vendas");
    c.key(Key::ArrowDown);
    assert!(!c.says("Quadros: vendas"), "a tile moved from the keyboard: {:?}", c.texts());
    focus(&mut c, "Mover Estoque");
    shifted(&mut c, Key::ArrowRight);
    assert!(c.says("estoque 2×1"), "Shift with an arrow resizes: {:?}", c.texts());
    c.press("Sobre as vendas");
    assert!(c.says("Vendas: soma dos pedidos pagos"), "accessory of a tile: {:?}", c.texts());
    c.activate("Travar os quadros");
    assert!(!c.has_name("Mover Vendas"), "locked tiles have no handles: {:?}", c.texts());
}

#[test]
fn the_steps_go_back_and_forth_go_wrong_and_the_copy_advances() {
    let mut c = Center::memory("ana");
    c.press(etapas::PAGE);
    assert!(c.has_name("Etapa 3 de 4"), "where the steps stand: {:?}", c.texts());
    c.press(etapas::STEPS[0]);
    assert!(c.says("Etapa atual: Cliente") && c.has_name("Etapa 1 de 4"), "on_select: {:?}", c.texts());
    c.activate("Faltam fotos");
    assert!(c.says("Faltam duas fotos"), "a step with an error and a note: {:?}", c.texts());
    assert!(c.says("Com erro") && c.says("Próxima"), "the marks of every state: {:?}", c.texts());

    assert!(c.says("33%"), "the meters start a third done: {:?}", c.texts());
    c.press("Avançar a cópia");
    assert!(c.says("Cópia local: 60 de 120") && c.says("50%"), "meter and bar: {:?}", c.texts());
}

#[test]
fn the_highlights_change_tone_fill_the_empty_day_and_read_the_badges() {
    let mut c = Center::memory("ana");
    c.press(destaques::PAGE);
    assert!(c.says("Pode retirar o cartão") && c.says("Semana do café"), "the banners: {:?}", c.texts());
    c.activate(destaques::TONES[3]);
    assert!(c.says("Pagamento aprovado"), "another tone: {:?}", c.texts());
    assert!(c.has_name("Atenção") && c.has_name("Favorito"), "labelled tiles: {:?}", c.texts());

    c.press("Importar");
    assert!(c.says("Nada para importar"), "a second action of the empty state: {:?}", c.texts());
    c.press("Criar pedido");
    assert!(
        c.says("1 pedido hoje") && !c.says("Nenhum pedido hoje"),
        "the empty state goes: {:?}",
        c.texts()
    );
    c.press("Nova venda");
    assert!(c.says("Nenhum pedido hoje") && c.says("Caixa pronto"), "the banner's action: {:?}", c.texts());

    c.activate("Procurando");
    assert!(c.says("Procurando atualizações"), "the shimmer stops, the text stays: {:?}", c.texts());
    assert!(c.has_name("Não salvo") && c.has_name("Sincronizando"), "labelled dots: {:?}", c.texts());

    assert!(c.has_name("Disponível"), "the badge on the avatar: {:?}", c.texts());
    c.activate("Ana disponível");
    assert!(!c.has_name("Disponível"), "visible(false) hides the badge: {:?}", c.texts());
    c.press("Avisos da loja");
    assert!(c.says("Avisos lidos") && c.says("Avisos não lidos: 0"), "{:?}", c.texts());
}

#[test]
fn the_release_notes_walk_the_versions_and_the_days_have_their_words() {
    let mut c = Center::memory("ana");
    c.press(novidades::PAGE);
    assert!(c.says("Caixa mais rápido") && c.says("Por que atualizar"), "the newest note: {:?}", c.texts());
    c.press("Pedido #579");
    assert!(c.says("Abriu o pedido #579"), "a link of the note: {:?}", c.texts());
    c.press("Mais antiga");
    assert!(c.says("Versão aberta: Versão 2.3") && c.says("Tema escuro"), "{:?}", c.texts());
    c.press("Versão 2.2 20 de out. de 2026");
    assert!(c.says("Versão aberta: Versão 2.2"), "the list of versions: {:?}", c.texts());

    assert!(c.says(novidades::INSTALL), "the code block: {:?}", c.texts());
    c.press("Copiar");
    assert!(c.has_name("Copiado"), "the copy button says so: {:?}", c.texts());
    for day in ["Semana passada", "12/11/2026", "Ontem", "Hoje"] {
        assert!(c.has_name(day), "day separator {day}: {:?}", c.texts());
    }
}
