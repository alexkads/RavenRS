//! Filling, picking, opening and scrolling, as a person does.

use std::collections::BTreeSet;

use ravenrs_core::Size;
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    Button, Select, Slider, Text, TextField, TreeNode, TreeView, VStack, View, ViewExt, VirtualList,
};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark())
}

#[test]
fn fill_replaces_what_a_field_has() {
    let text = State::new("velho".to_owned());
    let mut e2e = session(VStack::new((TextField::new(text.binding()).test_id("nome"),)));
    e2e.by_test_id("nome").fill("novo").unwrap();
    e2e.by_test_id("nome").expect().to_have_value("novo").unwrap();
    assert_eq!(text.get(), "novo");
}

#[test]
fn fill_moves_a_slider_with_the_arrow_keys() {
    let value = State::new(0.0_f32);
    let mut e2e = session(VStack::new((Slider::new(value.binding()).test_id("volume"),)));
    let before = e2e.by_test_id("volume").node().unwrap().numeric.unwrap();
    let step = before.step.unwrap_or(0.01).max(0.01);
    let target = before.min + step * 3.0;
    e2e.by_test_id("volume").fill(&target.to_string()).unwrap();
    let after = e2e.by_test_id("volume").node().unwrap().numeric.unwrap();
    assert!((after.value - target).abs() <= step, "{after:?}");
    let refused = e2e.by_test_id("volume").fill("muito").unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Expectation);
}

#[test]
fn select_opens_a_combo_box_and_picks_the_option() {
    let chosen = State::new(0_usize);
    let mut e2e = session(VStack::new((
        Select::new(chosen.binding(), ["Aluguel", "Mercado", "Lazer"]).test_id("categoria"),
    )));
    e2e.by_test_id("categoria").select("Mercado").unwrap();
    assert_eq!(chosen.get(), 1);
    e2e.by_test_id("categoria").expect().to_have_value("Mercado").unwrap();
    let missing = e2e.by_test_id("categoria").select("Viagem").unwrap_err();
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert!(missing.message().contains("Viagem"), "{missing}");
}

#[test]
fn expand_and_collapse_open_a_tree_item_by_its_key() {
    let open = State::new(BTreeSet::<String>::new());
    let tree = TreeView::new(vec![
        TreeNode::new("docs", "Documentos").child(TreeNode::new("nota", "Nota fiscal")),
        TreeNode::new("fotos", "Fotos"),
    ])
    .expanded(open.binding());
    let mut e2e = session(VStack::new((tree,)));
    assert!(e2e.by_test_id("item:nota").node().is_err(), "closed at first");
    e2e.by_test_id("item:docs").expand().unwrap();
    e2e.by_test_id("item:nota").expect().to_have_name("Nota fiscal").unwrap();
    e2e.by_test_id("item:docs").expand().unwrap();
    e2e.by_test_id("item:docs").collapse().unwrap();
    assert!(e2e.by_test_id("item:nota").node().is_err(), "closed again");
    let leaf = e2e.by_test_id("item:fotos").expand().unwrap_err();
    assert_eq!(leaf.code(), ErrorCode::Expectation);
    assert!(leaf.message().contains("does not open"), "{leaf}");
}

#[test]
fn a_row_a_long_list_has_not_built_yet_is_scrolled_to_by_its_key() {
    let picked = State::new(None::<usize>);
    let p = picked.clone();
    let list = VirtualList::new(500, |i| Text::new(format!("Linha {i}")))
        .row_height(28.0)
        .key(|i| i as u64 + 1000)
        .on_activate(move |i| p.set(Some(i)));
    let mut e2e = session(VStack::new((Button::new("Topo"), list.height(200.0))))
        .viewport(Size::new(400.0, 300.0), 1.0);
    assert!(e2e.by_test_id("row:1042").node().is_err(), "not built while out of sight");
    e2e.by_test_id("row:1042").scroll_into_view().unwrap();
    e2e.by_test_id("row:1042").expect().to_be_visible().unwrap();
    e2e.by_test_id("row:1003").scroll_into_view().unwrap();
    e2e.by_test_id("row:1003").expect().to_be_visible().unwrap();
    let gone = e2e.by_test_id("row:9999").scroll_into_view().unwrap_err();
    assert_eq!(gone.code(), ErrorCode::NotFound);
}
