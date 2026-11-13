//! Geometry and behavior of layout containers, modifiers, cards, images,
//! navigation and view utilities, checked through the painted scene.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Brush, Color, Event, ImageData, Point, PointerButton, Primitive, Quad, Rect, Size};
use ravenrs_layout::{Align, Justify};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

const RED: u32 = 0xff0000;

fn ui_at(view: impl View + 'static, w: f32, h: f32) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(w, h), 1.0);
    ui.update();
    ui
}

fn ui(view: impl View + 'static) -> Ui {
    ui_at(view, 800.0, 600.0)
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn fill_rect(ui: &Ui, color: Color) -> Rect {
    quads(ui)
        .into_iter()
        .find(|q| q.fill == Brush::Solid(color))
        .map(|q| q.rect)
        .expect("quad with that fill")
}

/// A fixed-size colored block, so positions can be read back from the scene.
fn block(color: u32, w: f32, h: f32) -> Styled<Spacer> {
    Spacer::new().background(Color::hex(color)).frame(Some(w), Some(h))
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

#[test]
fn hstack_spacing_justify_and_alignment() {
    let ui = ui(HStack::new((block(RED, 40.0, 20.0), block(0x00ff00, 40.0, 60.0)))
        .spacing(10.0)
        .padding(5.0)
        .align(Align::End)
        .width(300.0)
        .justify(Justify::End));
    let (a, b) = (fill_rect(&ui, Color::hex(RED)), fill_rect(&ui, Color::hex(0x00ff00)));
    assert_eq!(b.max_x(), 295.0, "justified to the end inside the padding");
    assert_eq!(b.x() - a.max_x(), 10.0);
    assert_eq!(a.max_y(), b.max_y(), "bottom-aligned");
}

#[test]
fn space_between_and_wrapping() {
    let ui1 = ui(HStack::new((block(RED, 40.0, 20.0), block(0x00ff00, 40.0, 20.0)))
        .width(300.0)
        .justify(Justify::SpaceBetween));
    assert_eq!(fill_rect(&ui1, Color::hex(0x00ff00)).max_x(), 300.0);
    let ui2 = ui(HStack::new((block(RED, 120.0, 20.0), block(0x00ff00, 120.0, 20.0)))
        .width(200.0)
        .spacing(0.0)
        .wrap());
    assert_eq!(fill_rect(&ui2, Color::hex(0x00ff00)).y(), 20.0, "the second block wraps to a new line");
}

#[test]
fn modifiers_paint_and_size() {
    let ui = ui(VStack::new((Text::new("x")
        .padding((4.0, 8.0))
        .background(Color::hex(RED))
        .corner_radius(6.0)
        .border(2.0, Semantic::Border)
        .shadow(ravenrs_core::Shadow::soft())
        .width(120.0)
        .height(40.0),)));
    let q = quads(&ui).into_iter().find(|q| q.fill == Brush::Solid(Color::hex(RED))).expect("styled box");
    assert_eq!(q.rect.size, Size::new(120.0, 40.0));
    assert_eq!(q.corner_radii, [6.0; 4]);
    assert_eq!(q.border.map(|b| b.width), Some(2.0));
    assert!(q.shadow.is_some());
}

#[test]
fn stretch_centered_grow_and_max_width() {
    let ui = ui(VStack::new((
        // No explicit width: `stretch` decides (an explicit width would win, as in CSS).
        Spacer::new().background(Color::hex(RED)).frame(None, Some(10.0)).stretch(),
        HStack::new((block(0x00ff00, 10.0, 10.0).grow(1.0), block(0x0000ff, 10.0, 10.0))).width(200.0),
        Text::new("centered").centered().background(Color::hex(0xffff00)).width(300.0).max_width(250.0),
    )));
    assert_eq!(fill_rect(&ui, Color::hex(RED)).width(), 800.0, "stretch fills the stack's width");
    // 200 − 10 for the other block − 16 default spacing.
    assert_eq!(fill_rect(&ui, Color::hex(0x00ff00)).width(), 174.0, "grow takes the free space");
    assert_eq!(fill_rect(&ui, Color::hex(0xffff00)).width(), 250.0, "max_width caps width");
}

#[test]
fn cards_follow_their_options() {
    let theme = Theme::raven_dark();
    let plain = ui(VStack::new((Card::new(Text::new("a"))
        .no_shadow()
        .border(false)
        .padding(4.0)
        .fill(Color::hex(RED)),)));
    let q = quads(&plain).into_iter().find(|q| q.fill == Brush::Solid(Color::hex(RED))).expect("card");
    assert!(q.shadow.is_none() && q.border.is_none());
    assert_eq!(q.corner_radii, [theme.radii.xl; 4], "default radius is radius-xl");
    let glass = ui(VStack::new((GlassCard::new(Text::new("g"))
        .corner_radius(10.0)
        .padding(8.0)
        .shadow(ravenrs_core::Shadow::raised()),)));
    let g = quads(&glass)
        .into_iter()
        .find(|q| matches!(q.fill, Brush::Solid(c) if c.a < 0.5 && c.a > 0.0))
        .expect("translucent glass");
    assert_eq!(g.corner_radii, [10.0; 4]);
}

#[test]
fn images_size_themselves_and_honor_modes() {
    let img = ImageData::from_fn(200, 100, |_, _| Color::WHITE).expect("image");
    let images = |ui: &Ui| -> Vec<ravenrs_core::ImageQuad> {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| if let Primitive::Image(i) = p { Some(i.clone()) } else { None })
            .collect()
    };
    let natural = ui(VStack::new((Image::new(img.clone()),)));
    assert_eq!(images(&natural)[0].rect.size, Size::new(200.0, 100.0), "natural size in logical pixels");
    let by_width = ui(VStack::new((Image::new(img.clone()).width(100.0).opacity(0.5),)));
    let i = &images(&by_width)[0];
    assert_eq!(i.rect.size, Size::new(100.0, 50.0), "height follows the aspect ratio");
    assert_eq!(i.opacity, 0.5);
    let fit = ui(VStack::new((Image::new(img.clone())
        .size(100.0, 100.0)
        .content_mode(ContentMode::Fit)
        .corner_radius(8.0),)));
    let f = &images(&fit)[0];
    assert_eq!((f.rect.height(), f.corner_radii[0]), (50.0, 8.0));
    let fill = ui(VStack::new((Image::new(img).size(100.0, 100.0).content_mode(ContentMode::Fill),)));
    assert_eq!(images(&fill)[0].uv, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn tabs_switch_on_click_and_keep_the_bar() {
    let sel = State::new(0_usize);
    let mut ui = ui(VStack::new((Tabs::new(sel.binding())
        .tab("One", Text::new("first"))
        .tab("Two", Text::new("second")),)));
    // The second tab label starts after "One" (~30px) + 24px spacing.
    let x = (40..200).step_by(2).map(|x| x as f32).find(|&x| {
        ui.handle_event(&Event::PointerMove { position: Point::new(x, 15.0) });
        ui.cursor() == ravenrs_core::CursorIcon::Pointer && x > 45.0
    });
    click(&mut ui, Point::new(x.expect("second tab") + 4.0, 15.0));
    assert_eq!(sel.get(), 1);
}

#[test]
fn sidebar_items_breadcrumb_and_accordion() {
    let picked = Rc::new(Cell::new(false));
    let p = picked.clone();
    let mut ui = ui_at(
        HStack::new((
            Sidebar::new((
                SidebarSection::new("Main"),
                SidebarItem::new(IconName::House, "Home").count("3").selected(true),
                SidebarItem::new(IconName::Settings, "Settings").on_click(move || p.set(true)),
            ))
            .width(200.0),
            VStack::new((
                Breadcrumb::new(["A", "B", "C"]),
                Accordion::new(vec![
                    Disclosure::new("One", Text::new("1")).open(true),
                    Disclosure::new("Two", Text::new("2")),
                ]),
            ))
            .grow(1.0),
        )),
        800.0,
        400.0,
    );
    // Section caption (16 top padding + caption + 6) then two 36px rows with 2px spacing.
    let settings_y = quads(&ui)
        .iter()
        .filter(|q| q.rect.height() == 36.0 && q.rect.x() < 200.0)
        .map(|q| q.rect.center().y)
        .fold(0.0, f32::max);
    click(&mut ui, Point::new(100.0, settings_y));
    assert!(picked.get());
}

#[test]
fn view_utilities() {
    // Option<V>, Vec<V> sequences, boxed views, Keyed resets state, responsive switches.
    let show = State::new(true);
    let key = State::new(1_u64);
    let (s, k) = (show.clone(), key.clone());
    let mut ui = ui_at(
        view_fn(move || {
            let items: Vec<AnyView> =
                vec![Box::new(Text::new("boxed")), Box::new(s.get().then(|| Text::new("optional")))];
            VStack::new((
                VStack::new(items),
                Keyed::new(k.get(), Disclosure::new("Keyed", Text::new("inside"))),
                responsive(|size| -> AnyView {
                    if size.width > 500.0 {
                        Box::new(Text::new("wide"))
                    } else {
                        Box::new(Text::new("narrow!"))
                    }
                }),
            ))
        }),
        800.0,
        600.0,
    );
    let runs = |ui: &Ui| ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count();
    let with_optional = runs(&ui);
    show.set(false);
    ui.update();
    assert_eq!(runs(&ui), with_optional - 1, "None renders nothing");
    // Open the keyed disclosure, then change its key: it starts closed again.
    let header = quads(&ui)
        .into_iter()
        .map(|q| q.rect)
        .find(|r| r.height() > 30.0 && r.height() < 50.0)
        .expect("disclosure header");
    click(&mut ui, header.center());
    let open = runs(&ui);
    key.set(2);
    ui.update();
    assert_eq!(runs(&ui), open - 1, "a new key is a new view with fresh state");
    ui.set_viewport(Size::new(400.0, 600.0), 1.0);
    ui.update();
    let narrow: Vec<usize> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.glyphs.len()) } else { None })
        .collect();
    assert!(narrow.contains(&7), "responsive layout switched to the narrow branch");
}

#[test]
fn canvas_icon_fill_and_grid() {
    let drawn = Rc::new(Cell::new(Rect::ZERO));
    let d = drawn.clone();
    let ui = ui(VStack::new((
        HStack::new((Canvas::new(move |f, _| d.set(f)).grow(1.0), block(RED, 50.0, 10.0))).width(300.0),
        Icon::new(IconName::Search).size(24.0).color(Color::hex(0x00ff00)),
        Grid::new(
            3,
            vec![
                block(0x0000ff, 10.0, 10.0),
                block(0x0000ff, 10.0, 10.0),
                block(0x0000ff, 10.0, 10.0),
                block(0xffff00, 10.0, 10.0),
            ],
        )
        .spacing(6.0),
    )));
    // 300 wide − 50 for the block − 16 default spacing.
    assert_eq!(drawn.get().width(), 234.0, "a growing canvas fills the free space");
    let icon = ui
        .scene()
        .primitives()
        .iter()
        .find_map(|p| if let Primitive::Glyphs(g) = p { Some(g.clone()) } else { None })
        .expect("icon glyph");
    assert_eq!((icon.size, icon.color), (24.0, Color::hex(0x00ff00)));
    assert!(
        fill_rect(&ui, Color::hex(0xffff00)).y() > fill_rect(&ui, Color::hex(0x0000ff)).y(),
        "the fourth cell starts a new row"
    );
    assert_eq!(
        Fill::from(Semantic::Accent).resolve(&Theme::raven_dark()),
        Theme::raven_dark().color(Semantic::Accent)
    );
}
