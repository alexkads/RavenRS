//! The steps of the family `ravenrs-widgets-lists`: the pages Árvores e arquivos, Listas longas,
//! Documentos e miniaturas and Ano inteiro.

use super::*;
use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton};

impl Center {
    /// Gives the keyboard to what has this role and this name.
    fn focus_on(&mut self, role: Role, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {role:?} {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
    }

    /// Presses a key with these modifiers held.
    fn key_with(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(&Event::KeyDown { key, modifiers });
        self.settle();
    }

    /// Double-clicks what has this name.
    fn double_click(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name) && n.role != Role::Group);
        let b = node.unwrap_or_else(|| panic!("nothing called {name}: {:?}", self.texts())).bounds;
        let at = Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 2.0);
        self.ui.handle_event(&Event::PointerMove { position: at });
        self.ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
        self.ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
        self.ui.handle_event(&Event::DoubleClick { position: at });
        self.settle();
    }
}

fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::default() }
}

fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::default() }
}

#[test]
fn the_tree_opens_filters_checks_brings_a_branch_and_the_explorer_goes_in_and_back() {
    let mut c = Center::memory("ana");
    c.press("Árvores e arquivos");
    assert!(!c.has_name("Pedidos"), "the branches start closed");
    c.press("Abrir tudo");
    assert!(c.has_name("Pedidos") && c.has_name("Depósitos"), "expanded from outside: {:?}", c.texts());
    c.activate("Clientes");
    assert!(c.says("Selecionado: clientes"), "selected: {:?}", c.texts());
    c.type_into("Filtrar o menu", "Prod");
    assert!(c.has_name("Produtos") && !c.has_name("Clientes"), "filter: {:?}", c.texts());
    for _ in 0..4 {
        c.key(Key::Backspace);
    }
    // The reports open from the keyboard, and only then come.
    c.focus_on(Role::Tree, "Menu do sistema");
    c.key(Key::End);
    c.key(Key::ArrowRight);
    assert!(c.says("Relatórios carregados ao abrir"), "on_expand: {:?}", c.texts());
    assert!(c.has_name("Estoque baixo"), "the lazy branch has its items now");
    c.key(Key::Space);
    assert!(
        c.says("Marcados: Estoque baixo, Vendas do mês"),
        "checking a branch checks its tips: {:?}",
        c.texts()
    );
    c.key(Key::Enter);
    assert!(c.says("Abriu relatorios"), "on_open: {:?}", c.texts());
    c.key_with(Key::Function(10), shift());
    c.activate("Renomear");
    assert!(c.says("Renomear relatorios"), "context_menu: {:?}", c.texts());

    assert!(!c.has_name("Item 1.1"), "the long tree starts closed: {:?}", c.texts());
    c.focus_on(Role::Tree, "Mil itens");
    c.key(Key::ArrowRight);
    assert!(c.has_name("Item 1.1") && !c.has_name("Item 1.100"), "only the lines in sight are built");

    c.double_click("Contratos");
    assert!(
        c.says("Entrou em Contratos") && c.has_name("aluguel.pdf"),
        "on_open of a folder: {:?}",
        c.texts()
    );
    c.activate("aluguel.pdf");
    assert!(c.says("1 selecionado(s)"), "selection: {:?}", c.texts());
    c.focus_on(Role::ListBox, "Arquivos do servidor");
    c.key(Key::Enter);
    assert!(c.says("Abriu o arquivo aluguel.pdf"), "on_open of a file: {:?}", c.texts());
    c.key(Key::Backspace);
    assert!(c.says("Voltou ao nível 0") && c.has_name("Fotos"), "on_navigate: {:?}", c.texts());
}

#[test]
fn the_long_lists_jump_select_follow_the_end_and_reorder() {
    let mut c = Center::memory("ana");
    c.press("Listas longas");
    assert!(c.says("À vista: linhas 1 a"), "on_visible: {:?}", c.texts());
    assert!(!c.has_name("Pedido 5000"), "ten thousand rows, only the first built");
    c.press("Ir para o pedido 5000");
    assert!(c.has_name("Pedido 5000"), "scroll_to: {:?}", c.texts());
    c.activate("Pedido 5000");
    assert!(c.says("Escolhido: pedido 5000"), "selection: {:?}", c.texts());
    c.focus_on(Role::List, "Dez mil pedidos");
    c.key(Key::ArrowDown);
    c.key(Key::Enter);
    assert!(c.says("Abriu o pedido 5001"), "on_activate from the keyboard: {:?}", c.texts());
    assert!(!c.says("Rolagem: 0 de"), "the list tells how far it went: {:?}", c.texts());

    c.activate("Conversa e faixa");
    c.press("Chegar mensagem");
    assert!(c.says("13 mensagens") && c.says("Mensagem 13:"), "follow_end shows the newest: {:?}", c.texts());

    c.activate("Ordenar");
    c.focus_on(Role::ListItem, "Conferir estoque");
    c.key_with(Key::ArrowUp, alt());
    assert!(c.says("Conferir estoque foi da posição 3 para a 2"), "on_reorder: {:?}", c.texts());
    c.key_with(Key::Home, alt());
    assert!(c.says("Primeira: Conferir estoque"), "to the top: {:?}", c.texts());
}

#[test]
fn the_document_tabs_open_close_and_the_document_and_the_strip_go_where_they_are_sent() {
    let mut c = Center::memory("ana");
    c.press("Documentos e miniaturas");
    assert!(c.says("Aberto: Contrato de aluguel · 3 abas, 1 sem salvar"), "{:?}", c.texts());
    c.activate("Proposta comercial");
    assert!(c.says("Aberto: Proposta comercial"), "a click opens the tab: {:?}", c.texts());
    c.focus_on(Role::Tab, "Proposta comercial");
    c.key(Key::Delete);
    assert!(c.says("Fechou Proposta comercial") && c.says("2 abas"), "on_close: {:?}", c.texts());
    c.focus_on(Role::Tab, "Contrato de aluguel");
    c.key_with(Key::ArrowRight, alt());
    assert!(c.says("Aba da posição 1 foi para a 2"), "on_reorder: {:?}", c.texts());

    c.press("Ir para a página 60");
    assert!(c.says("Página 60 de 120"), "page: {:?}", c.texts());
    c.press("Zoom 100%");
    assert!(c.says("zoom 100%"), "zoom: {:?}", c.texts());

    c.press("Escolher a última foto");
    assert!(c.has_name("Foto 2000"), "a selection from outside comes into view: {:?}", c.texts());
    c.focus_on(Role::ListBox, "Fotos do ensaio");
    c.key(Key::ArrowLeft);
    c.key(Key::Enter);
    assert!(c.says("Abriu a foto 1999"), "on_activate: {:?}", c.texts());
}

#[test]
fn the_year_marks_the_days_and_picks_a_day_and_a_month() {
    let mut c = Center::memory("ana");
    c.press("Ano inteiro");
    assert!(c.says("Feriado: Natal"), "marks: {:?}", c.texts());
    c.press("25 dezembro de 2026");
    assert!(c.says("Escolheu 25/12/2026: Feriado: Natal"), "on_pick: {:?}", c.texts());
    assert!(c.says("dia escolhido: 25/12"), "selected: {:?}", c.texts());
    c.press("dezembro");
    assert!(c.says("Abriu o mês de dezembro"), "on_month: {:?}", c.texts());
    c.activate("4");
    assert!(c.has_name("1 março de 2026"), "columns keep every month: {:?}", c.texts());
    c.press("Ano seguinte");
    assert!(c.says("Ano 2027") && c.has_name("1 março de 2027"), "the year binding: {:?}", c.texts());
}
