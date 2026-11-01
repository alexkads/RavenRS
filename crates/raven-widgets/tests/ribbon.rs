//! The ribbon driven through the public `Ui` API.

use std::cell::Cell;
use std::rc::Rc;

use raven_core::{Event, Point, PointerButton, Primitive, Size};
use raven_reactive::State;
use raven_style::Theme;
use raven_widgets::*;

fn glyph_runs(ui: &Ui) -> Vec<usize> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.glyphs.len()) } else { None })
        .collect()
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn ribbon(page: &State<usize>, bold: &State<bool>, pasted: &Rc<Cell<bool>>, table: bool) -> Ribbon {
    let p = pasted.clone();
    Ribbon::new()
        .selected_page(page.binding())
        .app_menu("File", vec![MenuItem::new("New"), MenuItem::new("Open")])
        .quick_access(RibbonItem::button("Save", IconName::Save))
        .page(
            RibbonPage::new("Home")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(
                            RibbonItem::button("Paste", IconName::ClipboardPaste)
                                .on_click(move || p.set(true)),
                        )
                        .item(RibbonItem::button("Cut", IconName::Scissors).small())
                        .item(RibbonItem::button("Copy", IconName::Copy).small()),
                )
                .group(
                    RibbonGroup::new("Font")
                        .item(RibbonItem::toggle("Bold", IconName::Bold, bold.binding()).icon_only()),
                ),
        )
        .page(
            RibbonPage::new("Insert")
                .group(RibbonGroup::new("Tables").item(RibbonItem::button("Table", IconName::Table))),
        )
        .page(RibbonPage::new("Layout").contextual("Table Tools", Tone::Warning, table))
}

fn ui_for(r: Ribbon) -> Ui {
    let mut ui = Ui::new(VStack::new((r,)), Theme::raven_light());
    ui.set_viewport(Size::new(1100.0, 600.0), 1.0);
    ui.update();
    ui
}

#[test]
fn large_small_and_toggle_items_work() {
    let (page, bold, pasted) = (State::new(0), State::new(false), Rc::new(Cell::new(false)));
    let mut ui = ui_for(ribbon(&page, &bold, &pasted, false));
    // Body starts under the 40px title row; the first large button is at x ≈ 8 + 2 + 4.
    click(&mut ui, Point::new(30.0, 40.0 + 4.0 + 3.0 + 30.0));
    assert!(pasted.get(), "the large Paste button ran its action");
    // Font group: after Clipboard (Paste + one column of small items).
    let before = bold.get();
    let font_x = (0..1100).step_by(4).map(|x| x as f32).find(|&x| {
        let mut probe = Ui::new(
            VStack::new((ribbon(&State::new(0), &State::new(false), &Rc::new(Cell::new(false)), false),)),
            Theme::raven_light(),
        );
        probe.set_viewport(Size::new(1100.0, 600.0), 1.0);
        probe.update();
        probe.handle_event(&Event::PointerMove { position: Point::new(x, 40.0 + 4.0 + 3.0 + 13.0) });
        probe.cursor() == raven_core::CursorIcon::Pointer && x > 140.0
    });
    click(&mut ui, Point::new(font_x.expect("bold button") + 6.0, 40.0 + 4.0 + 3.0 + 13.0));
    assert_ne!(bold.get(), before, "the toggle flipped");
}

#[test]
fn tabs_switch_pages_and_contextual_pages_appear_on_demand() {
    let (page, bold, pasted) = (State::new(0), State::new(false), Rc::new(Cell::new(false)));
    let ui = ui_for(ribbon(&page, &bold, &pasted, false));
    let without = glyph_runs(&ui).len();
    let ui2 = ui_for(ribbon(&page, &bold, &pasted, true));
    assert_eq!(glyph_runs(&ui2).len(), without + 2, "contextual tab and its category caption");
    page.set(1);
    let mut ui = ui;
    ui.update();
    assert!(glyph_runs(&ui).contains(&5), "the Insert page shows its 'Table' command");
}

#[test]
fn double_click_collapses_and_tabs_open_a_popup() {
    let (page, bold, pasted) = (State::new(0), State::new(false), Rc::new(Cell::new(false)));
    let mut ui = ui_for(ribbon(&page, &bold, &pasted, false));
    let expanded = ui.element_count();
    // The Home tab sits right after the File button and the quick access toolbar.
    let tab = (100..400).step_by(2).map(|x| Point::new(x as f32, 23.0)).find(|&p| {
        ui.handle_event(&Event::PointerMove { position: p });
        ui.cursor() == raven_core::CursorIcon::Pointer && p.x > 110.0
    });
    let tab = tab.expect("home tab");
    ui.handle_event(&Event::DoubleClick { position: tab });
    ui.update();
    assert!(ui.element_count() < expanded, "collapsed: the page body is gone");
    click(&mut ui, tab);
    assert!(ui.element_count() >= expanded, "clicking a tab shows the body as a popup");
}
