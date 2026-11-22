//! The overlays of `ravenrs-widgets-base` keep the testability contract (#737, second lot): they
//! open and close with the actions a screen reader asks for, and their items are found by label.

use ravenrs_core::{AccessAction, Size};
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Button, Command, CommandPalette, ContextMenu, Dialog, DropdownMenu, MenuItem, Menubar, Popover, Sheet,
    Text, Toaster, Toasts, Tone, VStack, View, ViewExt,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(1000.0, 760.0), 1.0)
}

#[test]
fn a_dialog_covers_what_is_behind_it_and_dismiss_closes_it() {
    let open = State::new(true);
    let view = VStack::new((
        Button::new("Atrás").test_id("atras"),
        Dialog::new(open.binding(), "Excluir conta").content(Text::new("Tem certeza?")),
    ));
    let mut e2e = session(view);
    assert_eq!(e2e.by_test_id("atras").click().unwrap_err().code(), ErrorCode::Covered);
    e2e.by_role(ravenrs_core::Role::Dialog).dismiss().unwrap();
    assert!(!open.get());
    e2e.by_test_id("atras").click().unwrap();
}

#[test]
fn a_dialog_and_a_sheet_close_with_their_close_button() {
    let (dialog, sheet) = (State::new(true), State::new(false));
    let view = VStack::new((
        Dialog::new(dialog.binding(), "Aviso").content(Text::new("Pronto.")),
        Sheet::new(sheet.binding(), "Detalhes").content(Text::new("Mais.")),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("close").click().unwrap();
    assert!(!dialog.get());
    sheet.set(true);
    e2e.advance(0.1);
    e2e.by_role(ravenrs_core::Role::Dialog).dismiss().unwrap();
    assert!(!sheet.get());
}

#[test]
fn a_dropdown_menu_opens_by_the_action_and_its_items_are_found_by_label() {
    let picked = State::new(String::new());
    let p = picked.clone();
    let menu = DropdownMenu::new("Arquivo")
        .item(MenuItem::new("Abrir"))
        .item(MenuItem::new("Salvar").on_select(move || p.set("Salvar".into())));
    let mut e2e = session(VStack::new((menu.test_id("arquivo"),)));
    e2e.by_test_id("arquivo").show_menu().unwrap();
    e2e.by_test_id("item:Salvar").click().unwrap();
    assert_eq!(picked.get(), "Salvar");
    e2e.by_test_id("arquivo").expand().unwrap();
    e2e.by_role(ravenrs_core::Role::Menu).dismiss().unwrap();
    assert_eq!(e2e.by_test_id("item:Abrir").count().unwrap(), 0, "the menu closed");
}

#[test]
fn a_disabled_menu_item_refuses_the_click() {
    let mut e2e = session(VStack::new((DropdownMenu::new("Editar")
        .item(MenuItem::new("Colar").disabled("Nada copiado"))
        .test_id("editar"),)));
    e2e.by_test_id("editar").show_menu().unwrap();
    assert_eq!(e2e.by_test_id("item:Colar").click().unwrap_err().code(), ErrorCode::Disabled);
}

#[test]
fn a_context_menu_opens_with_show_menu_below_its_content() {
    let picked = State::new(false);
    let p = picked.clone();
    let view = ContextMenu::new(Button::new("Linha").test_id("linha"))
        .item(MenuItem::new("Copiar").on_select(move || p.set(true)));
    let mut e2e = session(VStack::new((view,)));
    e2e.by_test_id("linha").show_menu().unwrap();
    e2e.by_test_id("item:Copiar").click().unwrap();
    assert!(picked.get());
}

#[test]
fn a_menubar_opens_its_menu_by_title() {
    let saved = State::new(false);
    let s = saved.clone();
    let bar = Menubar::new().menu("Arquivo").item(MenuItem::new("Salvar").on_select(move || s.set(true)));
    let mut e2e = session(VStack::new((bar,)));
    e2e.by_test_id("menu:Arquivo").expand().unwrap();
    e2e.by_test_id("item:Salvar").click().unwrap();
    assert!(saved.get());
}

#[test]
fn a_popover_opens_and_closes_by_the_action_and_keeps_the_anchors_own() {
    let open = State::new(false);
    let view = Popover::new(open.binding(), Button::new("Filtros").test_id("filtros"), Text::new("Por data"));
    let mut e2e = session(VStack::new((view,)));
    e2e.by_test_id("filtros").expand().unwrap();
    assert!(open.get());
    e2e.by_test_id("filtros").collapse().unwrap();
    assert!(!open.get());
}

#[test]
fn a_command_palette_runs_a_command_by_label_and_dismiss_closes_it() {
    let (open, ran) = (State::new(true), State::new(false));
    let r = ran.clone();
    let palette = CommandPalette::new(open.binding())
        .command(Command::new("Abrir projeto", || {}))
        .command(Command::new("Novo arquivo", move || r.set(true)));
    let mut e2e = session(VStack::new((palette,)));
    e2e.by_test_id("option:Novo arquivo").perform(AccessAction::Select).unwrap();
    assert!(ran.get() && !open.get());
    open.set(true);
    e2e.advance(0.1);
    e2e.by_role(ravenrs_core::Role::Dialog).dismiss().unwrap();
    assert!(!open.get());
}

#[test]
fn a_toast_is_found_by_its_id_and_dismissed() {
    let toasts = Toasts::new();
    toasts.show_until_closed("Salvo", "O lançamento foi salvo.", Tone::Success);
    let mut e2e = session(VStack::new((Toaster::new(toasts.clone()),)));
    let id = e2e
        .snapshot()
        .lines()
        .find_map(|l| {
            l.split('#').nth(1).and_then(|r| r.split_whitespace().next()).filter(|i| i.starts_with("toast:"))
        })
        .map(str::to_owned)
        .expect("a toast with its id");
    e2e.by_test_id(id).dismiss().unwrap();
    e2e.advance(0.5);
    assert_eq!(toasts.len(), 0);
}
