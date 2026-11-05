//! `RichText`, `AttachmentStrip`, `AudioPlayerButton`, `SelectionBar` and media in `Chat`.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn running(root: impl View + 'static, width: f32) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(width, 700.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..4 {
        ui.update();
    }
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    settle(ui);
}

fn click_at(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

fn node(ui: &mut Ui, role: Role, name: &str) -> (u64, Rect) {
    let tree = ui.access_tree();
    let found = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} called {name}: {:#?}", names(&tree)));
    (found.id, found.bounds)
}

fn names(tree: &ravenrs_core::AccessTree) -> Vec<(Role, String)> {
    tree.nodes.iter().filter_map(|n| n.name.clone().map(|name| (n.role, name))).collect()
}

fn has(ui: &mut Ui, role: Role, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.role == role && n.name.as_deref() == Some(name))
}

fn click(ui: &mut Ui, role: Role, name: &str) {
    let (_, bounds) = node(ui, role, name);
    click_at(ui, Point::new(bounds.x() + bounds.width() / 2.0, bounds.y() + bounds.height() / 2.0));
}

fn tab_to(ui: &mut Ui, name: &str) {
    for _ in 0..40 {
        press(ui, Key::Tab);
        if ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref() == Some(name) {
            return;
        }
    }
    panic!("no focus stop called {name}");
}

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> (Log, Log) {
    let log = Log::default();
    (log.clone(), log)
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

mod rich_text {
    use super::*;

    #[test]
    fn plain_text_costs_what_a_text_costs() {
        let words = "Nothing to format here,\nnot even on the second line";
        let (plain, text) = (running(RichText::new(words), 400.0), running(Text::new(words), 400.0));
        assert_eq!(plain.element_count(), text.element_count());
        assert_eq!(glyph_runs(&plain), glyph_runs(&text));
    }

    #[test]
    fn a_screen_reader_gets_the_sentence_without_marks_and_not_word_by_word() {
        let mut ui = running(RichText::new("*Ready* to _go_ `now`"), 400.0);
        assert!(has(&mut ui, Role::Label, "Ready to go now"));
        assert!(!has(&mut ui, Role::Label, "Ready"), "{:?}", names(&ui.access_tree()));
    }

    #[test]
    fn a_link_opens_by_click_enter_and_space() {
        let (opened, seen) = log();
        let address = "https://example.com/a";
        let text = RichText::new(format!("*See* {address}.")).on_link(move |a| seen.borrow_mut().push(a));
        let mut ui = running(text, 500.0);
        click(&mut ui, Role::Link, address);
        assert_eq!(*opened.borrow(), [address]);
        tab_to(&mut ui, address);
        press(&mut ui, Key::Enter);
        press(&mut ui, Key::Space);
        assert_eq!(opened.borrow().len(), 3);
    }

    #[test]
    fn without_a_handler_a_link_is_not_a_control() {
        let mut ui = running(RichText::new("see https://example.com"), 500.0);
        assert!(!ui.access_tree().nodes.iter().any(|n| n.role == Role::Link));
    }

    #[test]
    fn two_links_in_the_same_text_are_two_stops() {
        let (opened, seen) = log();
        let text =
            RichText::new("https://a.io and\n* https://b.io").on_link(move |a| seen.borrow_mut().push(a));
        let mut ui = running(text, 500.0);
        tab_to(&mut ui, "https://a.io");
        press(&mut ui, Key::Tab);
        press(&mut ui, Key::Enter);
        assert_eq!(*opened.borrow(), ["https://b.io"]);
    }

    #[test]
    fn it_wraps_to_the_width_it_is_offered() {
        let words = "*Bold words* and many more plain words that have to go on to other lines ".repeat(3);
        let height = |width: f32| {
            let mut ui = running(RichText::new(words.clone()), width);
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::Label).expect("the text").bounds
        };
        let (wide, narrow) = (height(900.0), height(220.0));
        assert!(narrow.height() > wide.height() * 2.0, "{wide:?} {narrow:?}");
        assert!(narrow.width() <= 220.0, "{narrow:?}");
    }

    #[test]
    fn line_breaks_lists_and_quotes_each_take_a_line() {
        let bounds = |text: &str| {
            let mut ui = running(RichText::new(text), 500.0);
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::Label).expect("the text").bounds
        };
        let one = bounds("*a*").height();
        assert_eq!(bounds("*a*\nb\n\n* c\n> d").height(), one * 5.0);
    }

    #[test]
    fn links_only_shows_the_marks() {
        let mut ui = running(RichText::new("*a* https://a.io").links_only().on_link(|_| {}), 500.0);
        assert!(has(&mut ui, Role::Label, "*a* https://a.io"));
        assert!(has(&mut ui, Role::Link, "https://a.io"));
    }
}
