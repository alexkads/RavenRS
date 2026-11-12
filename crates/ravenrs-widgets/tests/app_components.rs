//! Trees, tags, colors, periods, steps, files, tiles, cards and row details, driven through
//! the public `Ui` API.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{AccessNode, Color, Event, Key, Modifiers, Point, PointerButton, Role, Size, Toggled};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 700.0), 1.0);
    ui.update();
    ui
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn key_with(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    key_with(ui, key, Modifiers::default());
}

fn typed(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        key(ui, Key::Character(c.to_string()));
    }
}

fn nodes(ui: &mut Ui, role: Role) -> Vec<AccessNode> {
    ui.access_tree().nodes.into_iter().filter(|n| n.role == role).collect()
}

fn named(ui: &mut Ui, role: Role, name: &str) -> Option<AccessNode> {
    nodes(ui, role).into_iter().find(|n| n.name.as_deref() == Some(name))
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let spot = named(ui, role, name).unwrap_or_else(|| panic!("{role:?} {name}")).bounds.center();
    click(ui, spot);
}

fn focus(ui: &mut Ui, role: Role) {
    let spot = nodes(ui, role).first().expect("the control").bounds.center();
    click(ui, spot);
}

/// The name of what the focused control has its keyboard on.
fn active_name(ui: &mut Ui) -> Option<String> {
    let tree = ui.access_tree();
    let active = tree.nodes.iter().find_map(|n| n.active_descendant)?;
    tree.nodes.iter().find(|n| n.id == active).and_then(|n| n.name.clone())
}

fn tree_names(ui: &mut Ui) -> Vec<String> {
    nodes(ui, Role::TreeItem).into_iter().filter_map(|n| n.name).collect()
}

fn sample_tree() -> Vec<TreeNode> {
    vec![
        TreeNode::new("sales", "Sales")
            .child(TreeNode::new("orders", "Orders"))
            .child(TreeNode::new("quotes", "Quotes")),
        TreeNode::new("stock", "Stock"),
    ]
}

#[test]
fn tree_opens_and_moves_with_the_keyboard() {
    let selected = State::new(None::<String>);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let log = opened.clone();
    let mut ui = ui(TreeView::new(sample_tree())
        .selected(selected.binding())
        .on_open(move |key| log.borrow_mut().push(key.to_owned())));
    assert_eq!(tree_names(&mut ui), ["Sales", "Stock"]);
    let sales = named(&mut ui, Role::TreeItem, "Sales").expect("sales");
    assert_eq!(sales.expanded, Some(false));
    assert_eq!(sales.level, Some(1));
    focus(&mut ui, Role::Tree);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(tree_names(&mut ui), ["Sales", "Orders", "Quotes", "Stock"]);
    assert_eq!(named(&mut ui, Role::TreeItem, "Orders").expect("orders").level, Some(2));
    key(&mut ui, Key::ArrowRight);
    assert_eq!(active_name(&mut ui).as_deref(), Some("Orders"), "right goes into the branch");
    key(&mut ui, Key::Space);
    assert_eq!(selected.get().as_deref(), Some("orders"));
    key(&mut ui, Key::Enter);
    assert_eq!(*opened.borrow(), ["orders"]);
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(active_name(&mut ui).as_deref(), Some("Sales"), "left goes to the parent");
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(tree_names(&mut ui), ["Sales", "Stock"], "and then closes it");
    key(&mut ui, Key::End);
    assert_eq!(active_name(&mut ui).as_deref(), Some("Stock"));
}

#[test]
fn tree_clicks_select_and_the_arrow_only_opens() {
    let selected = State::new(None::<String>);
    let mut ui = ui(TreeView::new(sample_tree()).selected(selected.binding()));
    let row = named(&mut ui, Role::TreeItem, "Sales").expect("sales").bounds;
    click(&mut ui, Point::new(row.x() + 12.0, row.center().y));
    assert_eq!(selected.get(), None, "the arrow is not the row");
    assert_eq!(tree_names(&mut ui).len(), 4);
    press(&mut ui, Role::TreeItem, "Quotes");
    assert_eq!(selected.get().as_deref(), Some("quotes"));
    assert!(named(&mut ui, Role::TreeItem, "Quotes").expect("quotes").selected == Some(true));
}

#[test]
fn checking_a_branch_checks_what_is_under_it() {
    let checked = State::new(BTreeSet::new());
    let expanded = State::new(BTreeSet::from(["sales".to_owned()]));
    let mut ui = ui(TreeView::new(sample_tree()).checked(checked.binding()).expanded(expanded.binding()));
    focus(&mut ui, Role::Tree);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::Space);
    assert_eq!(checked.get(), BTreeSet::from(["orders".to_owned(), "quotes".to_owned()]));
    assert_eq!(named(&mut ui, Role::TreeItem, "Sales").expect("sales").toggled, Some(Toggled::True));
    checked.update(|set| {
        set.remove("orders");
    });
    ui.update();
    assert_eq!(named(&mut ui, Role::TreeItem, "Sales").expect("sales").toggled, Some(Toggled::Mixed));
    focus(&mut ui, Role::Tree);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::Space);
    assert_eq!(checked.get().len(), 2, "a partial branch becomes whole");
    key(&mut ui, Key::Space);
    assert!(checked.get().is_empty());
}

#[test]
fn a_lazy_branch_asks_for_its_items_and_a_filter_finds_deep_ones() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = asked.clone();
    let mut ui = ui(TreeView::new(vec![TreeNode::new("far", "Far").lazy()])
        .on_expand(move |key| log.borrow_mut().push(key.to_owned())));
    focus(&mut ui, Role::Tree);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(*asked.borrow(), ["far"]);

    let mut found = self::ui(TreeView::new(sample_tree()).filter("quot"));
    assert_eq!(tree_names(&mut found), ["Sales", "Quotes"]);
}

#[test]
fn a_tall_tree_builds_only_what_is_in_sight() {
    let many: Vec<TreeNode> = (0..5000).map(|i| TreeNode::new(format!("{i}"), format!("Item {i}"))).collect();
    let mut ui = ui(TreeView::new(many).height(320.0));
    assert!(nodes(&mut ui, Role::TreeItem).len() < 40);
    focus(&mut ui, Role::Tree);
    key(&mut ui, Key::End);
    // The scroll follows on the next frame.
    ui.update();
    assert_eq!(active_name(&mut ui).as_deref(), Some("Item 4999"));
}

fn options() -> Vec<LookupItem> {
    vec![LookupItem::new("r", "Red"), LookupItem::new("g", "Green"), LookupItem::new("b", "Blue")]
}

#[test]
fn tags_are_picked_from_the_list_and_taken_off() {
    let tags = State::new(vec!["r".to_owned()]);
    let mut ui = ui(TagBox::new(tags.binding(), options()).label("Colors"));
    let field = named(&mut ui, Role::ComboBox, "Colors").expect("the field");
    assert_eq!(field.value.as_deref(), Some("Red"));
    click(&mut ui, field.bounds.center());
    typed(&mut ui, "blu");
    key(&mut ui, Key::Enter);
    assert_eq!(tags.get(), ["r", "b"]);
    key(&mut ui, Key::Backspace);
    assert_eq!(tags.get(), ["r"], "Backspace on an empty search takes the last");
    key(&mut ui, Key::Escape);
    press(&mut ui, Role::Button, "Remove Red");
    assert!(tags.get().is_empty());
}

#[test]
fn a_tag_box_can_add_what_was_typed_and_keeps_to_its_limit() {
    let tags = State::new(Vec::new());
    let mut ui = ui(TagBox::new(tags.binding(), options()).allow_new(true).most(2).label("Colors"));
    focus(&mut ui, Role::ComboBox);
    typed(&mut ui, "Teal");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "Red");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "Blue");
    key(&mut ui, Key::Enter);
    assert_eq!(tags.get(), ["Teal", "r"]);
}

#[test]
fn read_only_tags_show_but_do_not_change() {
    let tags = State::new(vec!["g".to_owned()]);
    let mut ui = ui(TagBox::new(tags.binding(), options()).read_only(true).label("Colors"));
    assert!(named(&mut ui, Role::Button, "Remove Green").is_none());
    focus(&mut ui, Role::ComboBox);
    assert!(nodes(&mut ui, Role::ListBox).is_empty());
    key(&mut ui, Key::Backspace);
    assert_eq!(tags.get(), ["g"]);
}

#[test]
fn a_color_is_typed_dragged_or_taken_from_the_palette() {
    let color = State::new(Some(Color::hex(0xFF0000)));
    let mut ui =
        ui(ColorPicker::new(color.binding()).label("Accent").palette([(Color::hex(0x00FF00), "Green")]));
    let field = named(&mut ui, Role::ComboBox, "Accent").expect("the field");
    assert_eq!(field.value.as_deref(), Some("#FF0000"));
    click(&mut ui, field.bounds.center());
    press(&mut ui, Role::Button, "Green");
    assert_eq!(color.get(), Some(Color::hex(0x00FF00)));

    let hue = named(&mut ui, Role::Slider, "Hue").expect("the hue bar");
    let spot = Point::new(hue.bounds.x() + hue.bounds.width() * 2.0 / 3.0, hue.bounds.center().y);
    ui.handle_event(&Event::PointerMove { position: spot });
    ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
    let past = Point::new(spot.x + 0.1, spot.y);
    ui.handle_event(&Event::PointerMove { position: past });
    ui.handle_event(&Event::PointerUp { position: past, button: PointerButton::Primary });
    ui.update();
    let moved = color.get().expect("a color");
    assert!(moved.b > 0.9 && moved.g < 0.1, "two thirds along is blue: {moved:?}");

    let field = nodes(&mut ui, Role::TextInput).first().expect("the hex field").bounds.center();
    click(&mut ui, field);
    key_with(
        &mut ui,
        Key::Character("a".into()),
        Modifiers { meta: true, ctrl: true, ..Modifiers::default() },
    );
    key(&mut ui, Key::Backspace);
    typed(&mut ui, "#12345");
    assert_ne!(color.get(), Some(Color::hex(0x123456)), "not a color yet");
    typed(&mut ui, "6");
    assert_eq!(color.get(), Some(Color::hex(0x123456)));

    press(&mut ui, Role::Button, "No color");
    assert_eq!(color.get(), None);
}

#[test]
fn a_shade_moves_with_the_arrows_and_a_gray_keeps_its_hue() {
    let color = State::new(Some(Color::hex(0x0000FF)));
    let mut ui = ui(ColorPicker::new(color.binding()).label("Accent"));
    focus(&mut ui, Role::ComboBox);
    let square = named(&mut ui, Role::Slider, "Saturation and brightness").expect("the square");
    click(&mut ui, square.bounds.center());
    for _ in 0..60 {
        key(&mut ui, Key::ArrowLeft);
    }
    let gray = color.get().expect("a color");
    assert!((gray.r - gray.b).abs() < 0.01, "no saturation left: {gray:?}");
    for _ in 0..60 {
        key(&mut ui, Key::ArrowRight);
    }
    let back = color.get().expect("a color");
    assert!(back.b > back.r + 0.2, "still blue after the gray: {back:?}");
}

fn day(d: u8) -> Date {
    Date::new(2026, 11, d).expect("a date")
}

fn day_button(ui: &mut Ui, d: u8) -> Point {
    let labels = Labels::default();
    let name = labels.long_date(day(d));
    named(ui, Role::Button, &name).unwrap_or_else(|| panic!("no {name}")).bounds.center()
}

#[test]
fn a_period_is_two_clicks_in_either_order() {
    let period = State::new(None::<DateRange>);
    let mut ui = ui(DateRangePicker::new(period.binding(), day(1)).label("Period"));
    focus(&mut ui, Role::ComboBox);
    let (later, sooner) = (day_button(&mut ui, 20), day_button(&mut ui, 5));
    click(&mut ui, later);
    assert_eq!(period.get(), None, "one click only starts it");
    click(&mut ui, sooner);
    assert_eq!(period.get(), Some(DateRange::new(day(5), day(20))));
    assert!(nodes(&mut ui, Role::Calendar).is_empty(), "it closes");
    let range = period.get().expect("a period");
    assert_eq!(range.days(), 16);
    assert!(range.contains(day(5)) && range.contains(day(20)) && !range.contains(day(21)));
    press(&mut ui, Role::Button, "Clear");
    assert_eq!(period.get(), None);
}

#[test]
fn a_ready_made_period_takes_one_click_and_a_phone_shows_one_month() {
    let period = State::new(None::<DateRange>);
    let last_week = DateRange::new(day(1), day(7));
    let mut ui =
        ui(DateRangePicker::new(period.binding(), day(1)).preset("First week", last_week).label("P"));
    focus(&mut ui, Role::ComboBox);
    assert_eq!(nodes(&mut ui, Role::Calendar).len(), 2);
    ui.set_viewport(Size::new(400.0, 700.0), 1.0);
    ui.update();
    assert_eq!(nodes(&mut ui, Role::Calendar).len(), 1);
    press(&mut ui, Role::Button, "First week");
    assert_eq!(period.get(), Some(last_week));
}

#[test]
fn a_wizard_goes_on_only_when_the_step_allows() {
    let step = State::new(0);
    let allowed = State::new(false);
    let finished = State::new(0);
    let (gate, done) = (allowed.clone(), finished.clone());
    let mut ui = ui(Wizard::new(
        step.binding(),
        vec![
            WizardStep::new("Data", Text::new("first"))
                .check(move || if gate.get() { Ok(()) } else { Err("Pick a source".to_owned()) }),
            WizardStep::new("Layout", Text::new("second")),
        ],
    )
    .on_finish(move || done.update(|n| *n += 1))
    .on_cancel(|| {}));
    assert!(named(&mut ui, Role::Button, "Back").is_none(), "nowhere to go back to");
    assert!(named(&mut ui, Role::Button, "Cancel").is_some());
    assert!(nodes(&mut ui, Role::List).iter().any(|n| n.name.as_deref() == Some("Step 1 of 2")));
    press(&mut ui, Role::Button, "Next");
    assert_eq!(step.get(), 0);
    assert!(!nodes(&mut ui, Role::Alert).is_empty(), "the step says why");
    allowed.set(true);
    ui.update();
    press(&mut ui, Role::Button, "Next");
    assert_eq!(step.get(), 1);
    assert!(nodes(&mut ui, Role::Alert).is_empty(), "the error stays behind");
    // A step already done can be reopened from the top.
    press(&mut ui, Role::Button, "Data");
    assert_eq!(step.get(), 0);
    press(&mut ui, Role::Button, "Next");
    press(&mut ui, Role::Button, "Finish");
    assert_eq!(finished.get(), 1);
    press(&mut ui, Role::Button, "Back");
    assert_eq!(step.get(), 0);
}

/// Answers with these files, and remembers what it was asked.
struct FakePicker(Rc<RefCell<Vec<FileRequest>>>, Vec<PickedFile>);

impl FilePickerProvider for FakePicker {
    fn pick(&self, request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>) {
        self.0.borrow_mut().push(request.clone());
        done(self.1.clone());
    }
}

#[test]
fn files_come_from_the_chooser_and_can_be_taken_off() {
    let files = State::new(Vec::new());
    let asked = Rc::new(RefCell::new(Vec::new()));
    let mut ui = ui(FileUpload::new(files.binding()).accept([".PDF"]).most_bytes(10));
    ui.set_file_picker(FakePicker(
        asked.clone(),
        vec![
            PickedFile::new("a.pdf", vec![1, 2, 3]),
            PickedFile::new("b.exe", vec![1]),
            PickedFile { name: "c.pdf".into(), size: 999, bytes: Vec::new().into() },
        ],
    ));
    ui.update();
    press(&mut ui, Role::Button, "Choose files");
    assert_eq!(asked.borrow()[0].extensions, ["pdf"]);
    assert_eq!(asked.borrow()[0].most_bytes, Some(10));
    assert!(asked.borrow()[0].multiple);
    assert_eq!(files.get().len(), 1);
    assert_eq!(&*files.get()[0].bytes, &[1, 2, 3]);
    let alerts: Vec<String> = nodes(&mut ui, Role::Alert).into_iter().filter_map(|n| n.name).collect();
    assert_eq!(alerts.len(), 2);
    assert!(alerts[0].starts_with("b.exe:") && alerts[1].starts_with("c.pdf:"), "{alerts:?}");
    press(&mut ui, Role::Button, "Remove a.pdf");
    assert!(files.get().is_empty());
}

#[test]
fn files_can_be_dropped_on_the_place_or_on_the_only_place_there_is() {
    let files = State::new(Vec::new());
    let mut ui = ui(FileUpload::new(files.binding()));
    let zone = named(&mut ui, Role::Button, "Choose files").expect("the button").bounds.center();
    assert!(ui.drop_files(Some(zone), vec![PickedFile::new("a.txt", vec![1])]));
    assert_eq!(files.get().len(), 1);
    assert!(ui.drop_files(None, vec![PickedFile::new("b.txt", vec![1])]), "a drop with no position");
    assert_eq!(files.get().len(), 2);
    ui.update();
    assert!(named(&mut ui, Role::ListItem, "b.txt").is_some());
}

#[test]
fn a_drop_with_no_position_does_not_guess_between_two_places() {
    let (a, b) = (State::new(Vec::new()), State::new(Vec::new()));
    let mut ui = ui(VStack::new((FileUpload::new(a.binding()), FileUpload::new(b.binding()))));
    assert!(!ui.drop_files(None, vec![PickedFile::new("a.txt", vec![1])]));
    assert!(a.get().is_empty() && b.get().is_empty());
}

#[test]
fn a_disabled_upload_takes_nothing() {
    let files = State::new(Vec::new());
    let mut ui = ui(FileUpload::new(files.binding()).disabled(true));
    assert!(!ui.drop_files(None, vec![PickedFile::new("a.txt", vec![1])]));
    let read_only = State::new(vec![PickedFile::new("kept.txt", vec![1])]);
    let mut shown = self::ui(FileUpload::new(read_only.binding()).read_only(true));
    assert!(named(&mut shown, Role::ListItem, "kept.txt").is_some());
    assert!(named(&mut shown, Role::Button, "Choose files").is_none());
    assert!(named(&mut shown, Role::Button, "Remove kept.txt").is_none());
}

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xde, 0x00,
    0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01,
    0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
    0x82,
];

#[cfg(feature = "decode")]
#[test]
fn an_image_field_shows_what_it_can_and_refuses_the_rest() {
    let photo = State::new(None);
    let mut ui = ui(ImageField::new(photo.binding()).label("Photo"));
    ui.set_file_picker(FakePicker(Rc::default(), vec![PickedFile::new("me.png", PNG.to_vec())]));
    ui.update();
    assert_eq!(nodes(&mut ui, Role::Image)[0].name.as_deref(), Some("Photo"));
    press(&mut ui, Role::Button, "Choose image");
    assert_eq!(photo.get().map(|f: PickedFile| f.name).as_deref(), Some("me.png"));
    assert!(named(&mut ui, Role::Button, "Replace").is_some());
    assert!(ui.drop_files(None, vec![PickedFile::new("notes.txt", vec![1])]));
    ui.update();
    assert_eq!(photo.get().map(|f: PickedFile| f.name).as_deref(), Some("me.png"), "refused");
    assert!(!nodes(&mut ui, Role::Alert).is_empty());
    press(&mut ui, Role::Button, "Remove");
    assert_eq!(photo.get(), None);
    assert!(nodes(&mut ui, Role::Alert).is_empty());
}

#[cfg(feature = "decode")]
#[test]
fn a_file_that_is_not_an_image_says_so_instead_of_breaking() {
    let photo = State::new(Some(PickedFile::new("fake.png", vec![1, 2, 3])));
    let mut ui = ui(ImageField::new(photo.binding()).label("Photo"));
    assert!(nodes(&mut ui, Role::Image)[0].name.as_deref() == Some("Photo"));
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Cannot show this image")));
}

fn tiles(spots: &State<Vec<TileSpot>>) -> TileLayout {
    TileLayout::new(spots.binding())
        .tile(Tile::new("a", "Alpha", Text::new("A")))
        .tile(Tile::new("b", "Beta", Text::new("B")))
        .tile(Tile::new("c", "Gamma", Text::new("C")))
}

fn tile_frame(ui: &mut Ui, name: &str) -> ravenrs_core::Rect {
    named(ui, Role::Group, name).unwrap_or_else(|| panic!("no tile {name}")).bounds
}

#[test]
fn tiles_fill_the_grid_and_use_fewer_columns_when_narrow() {
    let spots = State::new(vec![TileSpot::new("b", 2, 1)]);
    let mut ui = ui(tiles(&spots).columns(4));
    ui.update();
    let (b, a) = (tile_frame(&mut ui, "Beta"), tile_frame(&mut ui, "Alpha"));
    assert_eq!(b.x(), 0.0, "the saved order comes first");
    assert!(b.width() > a.width() * 1.9, "two cells wide");
    assert_eq!(a.y(), b.y(), "the next one fits beside it");
    ui.set_viewport(Size::new(300.0, 700.0), 1.0);
    ui.update();
    ui.update();
    let (b, a) = (tile_frame(&mut ui, "Beta"), tile_frame(&mut ui, "Alpha"));
    assert!(a.y() > b.y(), "one column now");
    assert!((a.width() - b.width()).abs() < 1.0, "a wide tile takes what there is");
}

#[test]
fn tiles_move_and_resize_from_the_keyboard() {
    let spots = State::new(Vec::new());
    let mut ui = ui(tiles(&spots));
    press(&mut ui, Role::Button, "Move Alpha");
    key(&mut ui, Key::ArrowRight);
    let order = |s: &State<Vec<TileSpot>>| s.get().into_iter().map(|t| t.key).collect::<Vec<_>>();
    assert_eq!(order(&spots), ["b", "a", "c"]);
    assert!(tile_frame(&mut ui, "Beta").x() < tile_frame(&mut ui, "Alpha").x());
    press(&mut ui, Role::Button, "Move Alpha");
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    key_with(&mut ui, Key::ArrowRight, shift);
    key_with(&mut ui, Key::ArrowDown, shift);
    let alpha = spots.get().into_iter().find(|t| t.key == "a").expect("alpha");
    assert_eq!((alpha.columns, alpha.rows), (2, 2));
    key_with(&mut ui, Key::ArrowLeft, shift);
    key_with(&mut ui, Key::ArrowLeft, shift);
    assert_eq!(spots.get().into_iter().find(|t| t.key == "a").expect("alpha").columns, 1, "never below one");
}

#[test]
fn a_tile_moves_by_dragging_its_handle() {
    let spots = State::new(Vec::new());
    let mut ui = ui(tiles(&spots));
    ui.update();
    let start = named(&mut ui, Role::Button, "Move Alpha").expect("the handle").bounds.center();
    let target = tile_frame(&mut ui, "Gamma").center();
    ui.handle_event(&Event::PointerMove { position: start });
    ui.handle_event(&Event::PointerDown { position: start, button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: Point::new(start.x + 8.0, start.y) });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: target });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: target, button: PointerButton::Primary });
    ui.update();
    let order: Vec<String> = spots.get().into_iter().map(|t| t.key).collect();
    assert_eq!(order.last().map(String::as_str), Some("a"), "{order:?}");
}

#[test]
fn a_tile_puts_its_accessory_in_the_header_after_the_title() {
    let spots = State::new(Vec::new());
    let clicked = State::new(false);
    let log = clicked.clone();
    let mut ui = ui(TileLayout::new(spots.binding()).tile(
        Tile::new("a", "Alpha", Text::new("A"))
            .accessory(Button::new("Help").on_click(move || log.set(true))),
    ));
    ui.update();
    let help = named(&mut ui, Role::Button, "Help").expect("the accessory").bounds;
    let handle = named(&mut ui, Role::Button, "Move Alpha").expect("the handle").bounds;
    let tile = tile_frame(&mut ui, "Alpha");
    assert!(help.x() > handle.x(), "after the handle and the title");
    assert!(help.y() < tile.y() + 36.0, "in the header, above the content");
    press(&mut ui, Role::Button, "Help");
    assert!(clicked.get(), "and it works like any control");
}

#[test]
fn a_tile_keeps_the_same_margin_on_both_sides_and_room_for_its_corner() {
    let spots = State::new(Vec::new());
    let mut ui = ui(TileLayout::new(spots.binding()).tile(
        Tile::new("a", "Alpha", Text::new("A")).accessory(Button::new("Help").size(ButtonSize::Small)),
    ));
    ui.update();
    let tile = tile_frame(&mut ui, "Alpha");
    let help = named(&mut ui, Role::Button, "Help").expect("the accessory").bounds;
    let text = named(&mut ui, Role::Label, "A").expect("the content").bounds;
    let right = tile.max_x() - help.max_x();
    assert!((text.x() - tile.x() - right).abs() < 0.5, "the content's margin is the header's: {right}");
    assert!(help.y() - tile.y() >= 8.0, "a small control does not touch the top edge");
    // The corner that resizes stands in the margin, past the content.
    assert!(tile.max_x() - text.max_x() >= 16.0, "{text:?} in {tile:?}");
}

#[test]
fn locked_tiles_have_no_handles() {
    let spots = State::new(Vec::new());
    let mut ui = ui(tiles(&spots).locked(true));
    assert!(named(&mut ui, Role::Button, "Move Alpha").is_none());
    assert!(named(&mut ui, Role::Group, "Alpha").is_some());
}

fn cards() -> CardView {
    CardView::new(10_000, |i| Box::new(Text::new(format!("Card {i}"))) as AnyView)
        .card_name(|i| format!("Card {i}"))
        .card_width(200.0)
        .card_height(100.0)
        .height(500.0)
}

#[test]
fn only_the_cards_in_sight_are_built_and_the_keyboard_walks_them() {
    let opened = State::new(None);
    let log = opened.clone();
    let mut ui = ui(cards().on_open(move |i| log.set(Some(i))));
    ui.update();
    let seen = nodes(&mut ui, Role::ListItem).len();
    assert!(seen > 4 && seen < 60, "{seen} cards built of 10000");
    // Four fit in 1000 wide: 200 each and the gaps.
    focus(&mut ui, Role::List);
    key(&mut ui, Key::Home);
    assert_eq!(active_name(&mut ui).as_deref(), Some("Card 0"));
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::ArrowDown);
    assert_eq!(active_name(&mut ui).as_deref(), Some("Card 5"));
    key(&mut ui, Key::Enter);
    assert_eq!(opened.get(), Some(5));
    key(&mut ui, Key::End);
    ui.update();
    assert_eq!(active_name(&mut ui).as_deref(), Some("Card 9999"), "it scrolls to what the keyboard reached");
}

#[test]
fn cards_are_selected_as_the_mode_says() {
    let selected = State::new(BTreeSet::new());
    let mut ui = ui(cards().selection(selected.binding()).selection_mode(SelectionMode::Multiple));
    ui.update();
    press(&mut ui, Role::ListItem, "Card 1");
    press(&mut ui, Role::ListItem, "Card 2");
    assert_eq!(selected.get(), BTreeSet::from([1, 2]));
    press(&mut ui, Role::ListItem, "Card 1");
    assert_eq!(selected.get(), BTreeSet::from([2]));
    assert_eq!(named(&mut ui, Role::ListItem, "Card 2").expect("card").selected, Some(true));
}

fn grid_rows(count: usize) -> GridData {
    GridData::new((0..count).map(|i| vec![format!("Row {i}").into(), (i as f64).into()]).collect())
}

fn detail_grid() -> DataGrid {
    DataGrid::new(grid_rows(1000), [GridColumn::text("Name"), GridColumn::number("N")])
        .row_detail(|row, index| Box::new(Text::new(format!("Details of {} at {index}", row[0].key_text()))))
        .detail_height(120.0)
        .height(500.0)
}

fn has_text(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
}

#[test]
fn a_row_opens_its_details_and_the_rows_below_make_room() {
    let mut ui = ui(detail_grid());
    ui.update();
    assert!(!has_text(&mut ui, "Details of Row 0 at 0"), "built only while open");
    let row_y = |ui: &mut Ui, n: usize| {
        let rows = nodes(ui, Role::Row);
        rows.iter().find(|r| ui_row_name(ui, r) == format!("Row {n}")).map(|r| r.bounds.y()).expect("the row")
    };
    let before = row_y(&mut ui, 1);
    let arrows = nodes(&mut ui, Role::Button);
    let arrow = arrows.iter().find(|n| n.name.as_deref() == Some("Show details")).expect("an arrow");
    assert_eq!(arrow.expanded, Some(false));
    click(&mut ui, arrow.bounds.center());
    assert!(has_text(&mut ui, "Details of Row 0 at 0"));
    assert_eq!(row_y(&mut ui, 1), before + 120.0);
    assert!(named(&mut ui, Role::Button, "Hide details").is_some());
    press(&mut ui, Role::Button, "Hide details");
    assert!(!has_text(&mut ui, "Details of Row 0 at 0"));
    assert_eq!(row_y(&mut ui, 1), before);
}

fn ui_row_name(ui: &mut Ui, row: &AccessNode) -> String {
    let tree = ui.access_tree();
    tree.nodes
        .iter()
        .filter(|n| n.role == Role::Cell && row.bounds.contains(n.bounds.center()))
        .find_map(|n| n.name.clone())
        .unwrap_or_default()
}

#[test]
fn details_open_from_the_keyboard_and_can_be_kept_from_outside() {
    let open = State::new(BTreeSet::from([2]));
    let mut ui = ui(detail_grid().expanded_rows(open.binding()));
    ui.update();
    assert!(has_text(&mut ui, "Details of Row 2 at 2"));
    focus(&mut ui, Role::Grid);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(open.get(), BTreeSet::from([0, 2]));
    assert!(has_text(&mut ui, "Details of Row 0 at 0"));
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(open.get(), BTreeSet::from([2]));
}

#[test]
fn with_no_details_the_grid_has_no_arrows_and_rows_start_where_they_did() {
    let plain = DataGrid::new(grid_rows(5), [GridColumn::text("Name")]).height(300.0);
    let mut ui = ui(plain);
    ui.update();
    assert!(named(&mut ui, Role::Button, "Show details").is_none());
    let row = nodes(&mut ui, Role::Row).into_iter().find(|n| n.bounds.height() == 36.0);
    assert!(row.is_some());
}

#[test]
fn the_export_menu_hands_over_what_the_grid_shows() {
    let got = Rc::new(RefCell::new(Vec::new()));
    let log = got.clone();
    let grid = DataGrid::new(
        grid_rows(3),
        [GridColumn::text("Name"), GridColumn::currency("N", "R$ ", 2).summary(Summary::Sum)],
    )
    .sort_by(1, SortDirection::Descending)
    .export_title("Rows")
    .on_export_file(move |format, table| log.borrow_mut().push((format, table)));
    let mut ui = ui(grid);
    press(&mut ui, Role::Button, "Export");
    press(&mut ui, Role::MenuItem, "PDF (.pdf)");
    let got = got.borrow();
    let (format, table) = &got[0];
    assert_eq!(*format, GridExportFormat::Pdf);
    assert_eq!(table.title, "Rows");
    assert_eq!(table.culture, "en-US");
    assert_eq!(table.rows[0][0], Value::from("Row 2"), "as sorted");
    assert_eq!(table.columns[1].title, "N");
    assert!(table.columns[1].total);
    assert_eq!(
        table.columns[1].kind,
        GridTableKind::Number { decimals: 2, prefix: "R$ ".into(), suffix: String::new() }
    );
}
