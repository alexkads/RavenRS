//! Behavior of the component library, driven through the public `Ui` API.

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Size};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn find_fill(ui: &Ui, role: Semantic) -> Option<Rect> {
    let c = Theme::raven_dark().color(role);
    quads(ui).into_iter().find(|q| q.fill == c.into()).map(|q| q.rect)
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

#[test]
fn toggle_and_checkbox_flip_their_binding() {
    let (a, b) = (State::new(false), State::new(true));
    let mut ui =
        ui(VStack::new((Toggle::new(a.binding()).label("Wi-Fi"), Checkbox::new("Remember me", b.binding()))));
    // The toggle's label is right of the switch; click the row's left part.
    click(&mut ui, Point::new(20.0, 12.0));
    assert!(a.get());
    // Second row starts below the first (24px) plus spacing (16px).
    click(&mut ui, Point::new(9.0, 49.0));
    assert!(!b.get());
}

#[test]
fn slider_follows_drag_and_arrow_keys() {
    let v = State::new(0.0_f32);
    let mut ui = ui(VStack::new((Slider::new(v.binding()).range(0.0, 100.0).step(1.0).width(218.0),)));
    // Usable track is 200px starting 9px in.
    ui.handle_event(&Event::PointerMove { position: Point::new(109.0, 12.0) });
    ui.handle_event(&Event::PointerDown {
        position: Point::new(109.0, 12.0),
        button: PointerButton::Primary,
    });
    assert_eq!(v.get(), 50.0);
    ui.handle_event(&Event::PointerMove { position: Point::new(400.0, 12.0) });
    assert_eq!(v.get(), 100.0, "dragging past the end clamps");
    ui.handle_event(&Event::PointerUp { position: Point::new(400.0, 12.0), button: PointerButton::Primary });
    ui.update();
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(v.get(), 99.0, "the slider kept focus from the click");
}

#[test]
fn text_field_types_and_submits() {
    let text = State::new(String::new());
    let submitted = State::new(String::new());
    let s = submitted.clone();
    let mut ui =
        ui(VStack::new((TextField::new(text.binding()).placeholder("Search").on_submit(move |t| s.set(t)),)));
    click(&mut ui, Point::new(50.0, 20.0));
    for k in [Key::Character("R".into()), Key::Character("a".into()), Key::Space, Key::Character("x".into())]
    {
        key(&mut ui, k);
    }
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Character("v".into()));
    assert_eq!(text.get(), "Ra v");
    key(&mut ui, Key::Tab);
    assert_eq!(text.get(), "Ra v", "Tab moves focus instead of typing");
    click(&mut ui, Point::new(50.0, 20.0));
    key(&mut ui, Key::Enter);
    assert_eq!(submitted.get(), "Ra v");
}

#[test]
fn disclosure_remembers_open_state_across_rebuilds() {
    let unrelated = State::new(0);
    let u = unrelated.clone();
    let mut ui = ui(view_fn(move || {
        u.get();
        VStack::new((Disclosure::new("Details", Text::new("Hidden content")),))
    }));
    assert_eq!(glyph_runs(&ui), 2, "chevron and title");
    click(&mut ui, Point::new(30.0, 20.0));
    assert_eq!(glyph_runs(&ui), 3, "content shown");
    unrelated.set(1);
    ui.update();
    assert_eq!(glyph_runs(&ui), 3, "local state survived a rebuild of the parent");
}

#[test]
fn dropdown_menu_opens_runs_action_and_closes() {
    let picked = State::new(0);
    let p = picked.clone();
    let mut ui = ui(VStack::new((DropdownMenu::new("Actions")
        .item(MenuItem::new("Duplicate").icon(IconName::Copy))
        .separator()
        .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive().on_select(move || p.set(7))),))
    .padding(20.0));
    let trigger = find_fill(&ui, Semantic::Control).expect("secondary trigger");
    click(&mut ui, trigger.center());
    let panel = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("menu panel").rect;
    assert!(panel.y() > trigger.max_y(), "menu opens below its trigger");
    // Rows are 34px; Delete is the third entry after a 9px separator block.
    click(&mut ui, Point::new(panel.x() + 40.0, panel.y() + 6.0 + 34.0 + 9.0 + 17.0));
    assert_eq!(picked.get(), 7);
    assert!(quads(&ui).iter().all(|q| q.shadow.is_none()), "menu closed after selecting");
}

#[test]
fn clicking_outside_or_escape_dismisses_overlays() {
    let mut ui = ui(VStack::new((Select::new(State::new(0).binding(), ["One", "Two"]),)).padding(20.0));
    let field = Rect::new(20.0, 20.0, 220.0, 40.0);
    click(&mut ui, field.center());
    assert!(quads(&ui).iter().any(|q| q.shadow.is_some()));
    click(&mut ui, Point::new(700.0, 500.0));
    assert!(quads(&ui).iter().all(|q| q.shadow.is_none()), "outside click closes");
    click(&mut ui, field.center());
    key(&mut ui, Key::Escape);
    assert!(quads(&ui).iter().all(|q| q.shadow.is_none()), "Escape closes");
}

#[test]
fn select_changes_selection() {
    let sel = State::new(0_usize);
    let mut ui = ui(VStack::new((Select::new(sel.binding(), ["Daily", "Weekly", "Monthly"]),)).padding(20.0));
    click(&mut ui, Point::new(100.0, 40.0));
    let panel = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("list").rect;
    click(&mut ui, Point::new(panel.x() + 30.0, panel.y() + 6.0 + 34.0 * 2.0 + 17.0));
    assert_eq!(sel.get(), 2);
}

#[test]
fn dialog_is_modal_and_closes_from_backdrop() {
    let open = State::new(true);
    let mut ui = ui(VStack::new((
        Dialog::new(open.binding(), "Delete project?").description("This cannot be undone."),
    )));
    let backdrop = quads(&ui).into_iter().find(|q| q.rect.size == Size::new(800.0, 600.0)).expect("backdrop");
    assert!(matches!(backdrop.fill, ravenrs_core::Brush::Solid(c) if c.a > 0.4));
    click(&mut ui, Point::new(400.0, 300.0));
    assert!(open.get(), "clicking the card keeps it open");
    click(&mut ui, Point::new(10.0, 10.0));
    assert!(!open.get(), "clicking the backdrop closes it");
}

#[test]
fn scroll_view_scrolls_clamps_and_clips() {
    let rows: Vec<Text> = (0..40).map(|i| Text::new(format!("Row {i}"))).collect();
    let mut ui = ui(VStack::new((ScrollView::new(rows).height(200.0),)));
    let first_y = |ui: &Ui| {
        ui.scene().primitives().iter().find_map(|p| match p {
            Primitive::Glyphs(g) => Some(g.glyphs[0].y),
            _ => None,
        })
    };
    let before = first_y(&ui).expect("glyphs");
    assert!(ui.scene().primitives().iter().any(|p| matches!(p, Primitive::PushClip(_))));
    assert!(
        ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, -20.0) })
    );
    assert!((first_y(&ui).expect("glyphs") - (before - 20.0)).abs() < 0.01);
    ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, -100_000.0) });
    ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, 100_000.0) });
    assert!((first_y(&ui).expect("glyphs") - before).abs() < 0.01, "clamped back to the top");
    assert!(
        !ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, 50.0) })
    );
}

#[test]
fn what_is_scrolled_out_of_sight_is_not_painted() {
    let few: Vec<Text> = (0..10).map(|i| Text::new(format!("Row {i}"))).collect();
    let many: Vec<Text> = (0..1000).map(|i| Text::new(format!("Row {i}"))).collect();
    let small = ui(VStack::new((ScrollView::new(few).height(200.0),)));
    let large = ui(VStack::new((ScrollView::new(many).height(200.0),)));
    assert!(large.element_count() > 50 * small.element_count());
    assert_eq!(glyph_runs(&large), glyph_runs(&small), "only what can be seen costs anything");
}

#[test]
fn something_scrolled_out_of_sight_can_still_take_focus() {
    let buttons: Vec<Button> = (0..100).map(|i| Button::new(format!("Button {i}"))).collect();
    let mut ui = ui(VStack::new((ScrollView::new(buttons).height(200.0),)));
    for _ in 0..50 {
        ui.handle_event(&Event::KeyDown { key: Key::Tab, modifiers: Modifiers::default() });
        ui.update();
    }
    let tree = ui.access_tree();
    let focused = tree.nodes.iter().find(|n| Some(n.id) == tree.focus).and_then(|n| n.name.clone());
    assert_eq!(focused.as_deref(), Some("Button 49"));
}

#[test]
fn tooltip_appears_on_hover() {
    let mut ui = ui(VStack::new((Tooltip::new(Text::new("Hover me"), "Explains things"),)).padding(80.0));
    let runs = glyph_runs(&ui);
    ui.handle_event(&Event::PointerMove { position: Point::new(90.0, 90.0) });
    ui.update();
    assert_eq!(glyph_runs(&ui), runs + 1);
}

#[test]
fn theme_switcher_changes_app_theme() {
    let mut ui = ui(VStack::new((ThemeSwitcher,)));
    let light = Theme::raven_light();
    // Segments: Dark, Light, Glass; each ~80px wide starting at x=3.
    let segments: Vec<Rect> = quads(&ui).into_iter().map(|q| q.rect).collect();
    let control = segments[0];
    click(&mut ui, Point::new(control.x() + control.width() / 2.0, control.center().y));
    assert_eq!(ui.theme().name, light.name);
    assert_eq!(ui.scene().clear_color(), light.color(Semantic::Canvas));
}

#[test]
fn tabs_show_only_the_selected_content() {
    let sel = State::new(0_usize);
    let mut ui = ui(VStack::new((Tabs::new(sel.binding())
        .tab("One", Text::new("first"))
        .tab("Two", Text::new("second body")),)));
    let before = glyph_runs(&ui);
    sel.set(1);
    ui.update();
    assert_eq!(glyph_runs(&ui), before);
    let texts: Vec<usize> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.glyphs.len()) } else { None })
        .collect();
    assert!(texts.contains(&11), "\"second body\" is shown: {texts:?}");
}

#[test]
fn table_list_and_charts_build() {
    let mut ui = ui(ScrollView::new((
        Table::new([Column::new("Name").weight(2.0), Column::new("MRR").numeric()])
            .row(["Acme", "$12k"])
            .row(["Globex", "$8k"]),
        List::new((
            ListItem::new("Ana").avatar("Ana Lima").trailing("Admin"),
            ListItem::new("Docs").icon(IconName::FileText),
        )),
        BarChart::new([("Mon", 3.0), ("Tue", 5.0), ("Wed", 2.0)]),
        AreaChart::new([1.0, 4.0, 2.0, 6.0, 5.0]),
        HStack::new((
            Badge::new("Live").tone(Tone::Success),
            Avatar::new("Raven RS"),
            Kbd::new("⌘K"),
            Icon::new(IconName::Search),
        )),
        Alert::new("Heads up", "Something happened").tone(Tone::Warning),
        Stat::new("Revenue", "$48k").delta("+4%", true),
        ProgressBar::new(0.4),
        Skeleton::line(12.0),
    )));
    // Tall enough for everything to be on screen, and so painted.
    ui.set_viewport(Size::new(800.0, 2000.0), 1.0);
    ui.update();
    ui.update();
    assert!(ui.element_count() > 40, "{}", ui.element_count());
    assert!(quads(&ui).len() > 19, "everything but the charts paints quads");
    let images = ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Image(_))).count();
    assert!(images >= 2, "each chart paints a picture");
}

#[test]
fn splitter_panes_show_their_content() {
    let mut ui = ui(Splitter::horizontal(Text::new("Left pane"), Text::new("Right pane")).height(200.0));
    let tree = ui.access_tree();
    for name in ["Left pane", "Right pane"] {
        let label = tree.find(ravenrs_core::Role::Label, name).expect("pane text");
        assert!(label.bounds.height() > 0.0 && label.bounds.width() > 0.0, "{name}: {:?}", label.bounds);
    }
}

#[test]
fn long_sidebar_items_stay_on_one_line_and_keep_their_full_name() {
    let long = "A very long label that could never fit inside this narrow panel at all";
    let mut ui = ui(VStack::new((SidebarItem::new(IconName::Rows3, long).compact(),)).width(160.0));
    let tree = ui.access_tree();
    let item = tree.nodes.iter().find(|n| n.name.as_deref() == Some(long)).expect("whole name is announced");
    assert!(item.bounds.height() <= 32.0, "{:?}", item.bounds);
}

#[test]
fn text_fields_use_the_control_fill() {
    let ui = ui(TextField::new(State::new(String::new()).binding()).placeholder("Name"));
    assert!(find_fill(&ui, Semantic::Control).is_some());
}

#[test]
fn a_clickable_card_is_a_named_button() {
    let clicks = State::new(0);
    let counter = clicks.clone();
    let mut ui = ui(Card::new(Text::new("Invoice")).on_click(move || counter.update(|c| *c += 1)));
    let at = ui.access_tree().find(ravenrs_core::Role::Button, "Invoice").expect("button").bounds.center();
    click(&mut ui, at);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn color_swatch_reports_its_name_and_click() {
    let clicks = State::new(0);
    let counter = clicks.clone();
    let mut ui = ui(ColorSwatch::new(ravenrs_core::Color::WHITE, "White")
        .selected(true)
        .on_click(move || counter.update(|c| *c += 1)));
    let tree = ui.access_tree();
    let node = tree.find(ravenrs_core::Role::Button, "White").expect("swatch");
    assert_eq!((node.bounds.width(), node.bounds.height()), (32.0, 32.0));
    let at = node.bounds.center();
    click(&mut ui, at);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn pagination_labels_can_be_translated() {
    let mut ui = ui(Pagination::new(State::new(1usize).binding(), 3).labels("Anterior", "Próxima", "Página"));
    let tree = ui.access_tree();
    for name in ["Anterior", "Próxima", "Página 2"] {
        assert!(tree.find(ravenrs_core::Role::Button, name).is_some(), "{name}");
    }
}

#[test]
fn dialog_width_is_configurable_and_fits_the_window() {
    let mut ui = ui(Dialog::new(State::new(true).binding(), "Wide").width(10_000.0).close_label("Fechar"));
    ui.update();
    let tree = ui.access_tree();
    assert!(tree.find(ravenrs_core::Role::Button, "Fechar").is_some());
    let dialog = tree.find(ravenrs_core::Role::Dialog, "Wide").expect("dialog");
    assert_eq!(dialog.bounds.width(), 768.0, "the 800 px window less its margins");
}

mod from_the_family_finance_example {
    //! What building a real app found wrong, kept fixed.
    use ravenrs_core::{Role, Size};
    use ravenrs_layout::Align;
    use ravenrs_reactive::State;
    use ravenrs_style::Theme;
    use ravenrs_widgets::{
        Badge, Button, Date, DatePicker, HStack, Labels, Localized, Text, Ui, VStack, View,
    };

    fn tree_of(view: impl View + 'static) -> Ui {
        let mut ui = Ui::new(view, Theme::raven_light());
        ui.set_viewport(Size::new(800.0, 600.0), 1.0);
        ui.update();
        ui.update();
        ui
    }

    fn frame_of(ui: &mut Ui, role: Role, name: &str) -> ravenrs_core::Rect {
        ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == role && n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"))
            .bounds
    }

    #[test]
    fn a_button_follows_a_row_that_centers_its_children() {
        let mut ui = tree_of(
            HStack::new((
                VStack::new((Text::new("Tall"),)).height(64.0),
                Button::new("Short"),
                Badge::new("Tag"),
            ))
            .align(Align::Center),
        );
        let short = frame_of(&mut ui, Role::Button, "Short");
        assert!((short.center().y - 32.0).abs() < 0.5, "in the middle of a 64 px row: {short:?}");
    }

    #[test]
    fn a_button_keeps_its_width_in_a_column_that_stretches() {
        let mut ui = tree_of(VStack::new((Button::new("Save"),)).align(Align::Stretch).width(500.0));
        assert!(frame_of(&mut ui, Role::Button, "Save").width() < 200.0);
    }

    #[test]
    fn none_takes_no_gap_in_a_stack() {
        let mut ui = tree_of(
            VStack::new((Button::new("A"), None::<Text>, None::<Text>, Button::new("B"))).spacing(20.0),
        );
        let (a, b) = (frame_of(&mut ui, Role::Button, "A"), frame_of(&mut ui, Role::Button, "B"));
        assert!((b.y() - a.max_y() - 20.0).abs() < 0.5, "one gap between them: {a:?} {b:?}");
    }

    #[test]
    fn localized_reaches_the_components_inside_and_only_those() {
        let day = State::new(Date::new(2026, 3, 9));
        let month = Date::new(2026, 3, 1).expect("valid");
        let (inside, outside) = (day.clone(), day);
        let mut ui = tree_of(VStack::new((
            Localized::new(Labels::pt_br(), DatePicker::new(inside.binding(), month).label("Dentro")),
            DatePicker::new(outside.binding(), month).label("Outside"),
        )));
        let value = |ui: &mut Ui, name: &str| {
            ui.access_tree()
                .nodes
                .iter()
                .find(|n| n.name.as_deref() == Some(name))
                .and_then(|n| n.value.clone())
                .unwrap_or_default()
        };
        assert_eq!(value(&mut ui, "Dentro"), "9 março de 2026");
        assert_eq!(value(&mut ui, "Outside"), "9 March 2026");
    }

    #[test]
    fn labels_write_dates_and_summaries_their_way() {
        let date = Date::new(2026, 11, 5).expect("valid");
        let pt = Labels::pt_br();
        assert_eq!(pt.month_title(date), "novembro de 2026");
        assert_eq!((pt.short_date)(date), "05/11/2026");
        assert_eq!(pt.summary(ravenrs_widgets::Summary::Sum), "Soma");
        assert_eq!(Labels::default().month_title(date), date.month_title());
    }
}

mod waking_from_another_thread {
    use ravenrs_core::Size;
    use ravenrs_style::Theme;
    use ravenrs_widgets::{BuildCx, NodeId, Ui, View};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counting(Rc<Cell<u32>>);

    impl View for Counting {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            self.0.set(self.0.get() + 1);
            cx.empty()
        }
    }

    #[test]
    fn a_wake_from_a_thread_rebuilds_once_and_tells_the_platform() {
        let builds = Rc::new(Cell::new(0));
        let mut ui = Ui::new(Counting(builds.clone()), Theme::raven_light());
        ui.set_viewport(Size::new(100.0, 100.0), 1.0);
        let told = Arc::new(AtomicUsize::new(0));
        let counter = told.clone();
        ui.set_remote_waker(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        ui.update();
        let settled = builds.get();
        assert!(!ui.needs_redraw(), "asleep");

        let waker = ui.ui_waker();
        let thread = std::thread::spawn(move || {
            waker.wake();
            waker.wake();
        });
        thread.join().expect("the thread ran");
        assert_eq!(told.load(Ordering::SeqCst), 2, "the platform hears each one");
        assert!(ui.needs_redraw());

        ui.update();
        assert_eq!(builds.get(), settled + 1, "together they cost one build");
        ui.update();
        assert_eq!(builds.get(), settled + 1, "and it sleeps again");
        assert!(!ui.needs_redraw());
    }
}

mod overlays_opened_from_inside_another {
    use super::{click, key};
    use ravenrs_core::{AccessAction, Key, Point, Rect, Role, Size};
    use ravenrs_reactive::State;
    use ravenrs_style::Theme;
    use ravenrs_widgets::{Button, Dialog, HStack, Popover, Select, Ui, VStack};

    fn select() -> Select {
        Select::new(State::new(0).binding(), ["Um", "Dois", "Três"]).label("Conta").width(200.0)
    }

    fn opened(view: impl ravenrs_widgets::View + 'static) -> Ui {
        let mut ui = Ui::new(view, Theme::raven_light());
        ui.set_viewport(Size::new(1000.0, 800.0), 1.0);
        ui.update();
        ui.update();
        open(&mut ui);
        ui
    }

    fn open(ui: &mut Ui) {
        let id = node(ui, Role::ComboBox, "Conta").1;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    }

    fn node(ui: &mut Ui, role: Role, name: &str) -> (Rect, u64) {
        find(ui, role, name).unwrap_or_else(|| panic!("no {role:?} {name}"))
    }

    fn find(ui: &mut Ui, role: Role, name: &str) -> Option<(Rect, u64)> {
        ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == role && n.name.as_deref() == Some(name))
            .map(|n| (n.bounds, n.id))
    }

    fn in_dialog(selected: State<usize>) -> impl ravenrs_widgets::View {
        Dialog::new(State::new(true).binding(), "Pagar")
            .content(Select::new(selected.binding(), ["Um", "Dois", "Três"]).label("Conta").width(200.0))
    }

    #[test]
    fn a_select_in_a_dialog_opens_under_its_field_and_works() {
        let selected = State::new(0_usize);
        let mut ui = opened(in_dialog(selected.clone()));
        let field = node(&mut ui, Role::ComboBox, "Conta").0;
        let option = node(&mut ui, Role::ListBoxOption, "Dois").0;
        assert!(option.y() >= field.max_y(), "under the field: {field:?} {option:?}");
        assert!((option.x() - field.x()).abs() < 16.0, "beside it, not in a corner: {field:?} {option:?}");

        click(&mut ui, option.center());
        assert_eq!(selected.get(), 1, "the click reaches it, above the backdrop");
        assert!(find(&mut ui, Role::ListBoxOption, "Dois").is_none(), "the menu closed");
        assert!(find(&mut ui, Role::ComboBox, "Conta").is_some(), "and the dialog stayed");
    }

    #[test]
    fn escape_closes_the_menu_and_then_the_dialog() {
        let open_dialog = State::new(true);
        let mut ui = opened(Dialog::new(open_dialog.binding(), "Pagar").content(select()));
        assert!(find(&mut ui, Role::ListBoxOption, "Dois").is_some());
        key(&mut ui, Key::Escape);
        ui.update();
        assert!(find(&mut ui, Role::ListBoxOption, "Dois").is_none(), "the menu first");
        assert!(open_dialog.get(), "the dialog stays");
        key(&mut ui, Key::Escape);
        assert!(!open_dialog.get(), "then the dialog");
    }

    #[test]
    fn a_menu_that_sticks_out_of_a_popover_does_not_close_it() {
        let open_popover = State::new(true);
        let selected = State::new(0_usize);
        let mut ui = opened(VStack::new((HStack::new((Popover::new(
            open_popover.binding(),
            Button::new("Abrir"),
            Select::new(selected.binding(), ["Um", "Dois", "Três"]).label("Conta").width(200.0),
        ),)),)));
        let option = node(&mut ui, Role::ListBoxOption, "Três").0;
        click(&mut ui, option.center());
        assert_eq!(selected.get(), 2);
        assert!(open_popover.get(), "the popover stays open");
        click(&mut ui, Point::new(900.0, 700.0));
        assert!(!open_popover.get(), "a click truly outside still closes it");
    }
}

mod glass_tokens_stay_translucent {
    use ravenrs_core::{Primitive, Size};
    use ravenrs_style::{Semantic, Theme};
    use ravenrs_widgets::{DataGrid, GridColumn, GridData, Summary, Ui};

    #[test]
    fn a_grids_header_and_footer_keep_the_glass_look() {
        let theme = Theme::raven_glass();
        let data = GridData::new(vec![vec!["Ana".into(), 10.0.into()]]);
        let grid = DataGrid::new(
            data,
            [GridColumn::text("Nome"), GridColumn::number("Valor").summary(Summary::Sum)],
        );
        let mut ui = Ui::new(grid, theme);
        ui.set_viewport(Size::new(800.0, 500.0), 1.0);
        ui.update();
        ui.update();
        let control = theme.color(Semantic::Control);
        let bands = ui
            .scene()
            .primitives()
            .iter()
            .filter(|p| matches!(p, Primitive::Quad(q) if q.fill == control.faded(0.6).into()))
            .count();
        assert_eq!(bands, 2, "the header and the footer");
        let too_opaque = ui
            .scene()
            .primitives()
            .iter()
            .filter(|p| matches!(p, Primitive::Quad(q) if q.fill == control.with_alpha(0.6).into()))
            .count();
        assert_eq!(too_opaque, 0, "a token's own transparency is kept");
    }
}

#[test]
fn a_modal_keeps_the_wheel_from_the_page_behind_it() {
    let open = State::new(false);
    let lines = |prefix: &str, count: usize| -> Vec<Text> {
        (0..count).map(|i| Text::new(format!("{prefix} {i}"))).collect()
    };
    let root = VStack::new((
        ScrollView::new((VStack::new(lines("Page", 200)),)),
        Dialog::new(open.binding(), "Notes")
            .content(ScrollView::new((VStack::new(lines("Note", 200)),)).height(200.0)),
    ))
    .grow(1.0);
    let mut ui = ui(root);
    let y_of = |ui: &mut Ui, name: &str| {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).map(|n| n.bounds.y()).expect(name)
    };
    let wheel = |ui: &mut Ui, x: f32, y: f32, dy: f32| {
        ui.handle_event(&Event::Scroll { position: Point::new(x, y), delta: Point::new(0.0, dy) })
    };
    let page = y_of(&mut ui, "Page 1");
    assert!(wheel(&mut ui, 50.0, 100.0, -40.0), "the page scrolls while nothing is open");
    let page = {
        let now = y_of(&mut ui, "Page 1");
        assert!(now < page);
        now
    };

    open.set(true);
    ui.update();
    ui.update();
    let note = y_of(&mut ui, "Note 1");
    let center = Point::new(400.0, 300.0);
    assert!(wheel(&mut ui, center.x, center.y, -40.0), "the dialog's own content scrolls");
    assert!(y_of(&mut ui, "Note 1") < note);

    // Past the end of its content, the wheel does not carry over to the page.
    wheel(&mut ui, center.x, center.y, -100_000.0);
    assert!(!wheel(&mut ui, center.x, center.y, -40.0));
    // Nor over the backdrop, beside the dialog.
    assert!(!wheel(&mut ui, 4.0, 4.0, -40.0));

    // The modal hides the page from the tree, so it is measured once it closes.
    open.set(false);
    ui.update();
    ui.update();
    assert_eq!(y_of(&mut ui, "Page 1"), page, "the page never moved");
}

#[test]
fn an_alert_can_offer_what_it_asks_for() {
    let clicks = State::new(0);
    let counted = clicks.clone();
    let alert = Alert::new("Late", "Two bills").action("See bills", move || counted.update(|n| *n += 1));
    let mut ui = ui(alert);
    let tree = ui.access_tree();
    let button = tree.nodes.iter().find(|n| n.name.as_deref() == Some("See bills")).expect("the button");
    ui.perform_action(button.id, ravenrs_core::AccessAction::Click);
    ui.update();
    assert_eq!(clicks.get(), 1);
}

#[test]
fn a_help_tip_opens_under_the_pointer_and_a_click_keeps_it_open() {
    let tip =
        VStack::new((HelpTip::new("Saldo", "A soma das contas.").how("Veja o gráfico."),)).padding(40.0);
    let mut ui = Ui::new(tip, Theme::raven_light());
    ui.set_viewport(Size::new(600.0, 400.0), 1.0);
    ui.update();
    let says = |ui: &mut Ui, wanted: &str| {
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|name| name.starts_with(wanted)))
    };
    assert!(says(&mut ui, "Help: Saldo"));
    assert!(!says(&mut ui, "A soma das contas."));
    let button = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Help: Saldo"))
        .map(|n| (n.id, n.bounds))
        .expect("the button");
    assert_eq!(
        ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.id == button.0)
            .and_then(|n| n.description.clone())
            .as_deref(),
        Some("A soma das contas. Veja o gráfico."),
        "screen readers get the same words"
    );
    ui.handle_event(&Event::PointerMove { position: button.1.center() });
    ui.update();
    assert!(says(&mut ui, "A soma das contas."));
    ui.handle_event(&Event::PointerMove { position: Point::new(500.0, 350.0) });
    ui.update();
    assert!(!says(&mut ui, "A soma das contas."), "it leaves with the pointer");
    ui.perform_action(button.0, ravenrs_core::AccessAction::Click);
    ui.update();
    assert!(says(&mut ui, "A soma das contas."), "a click keeps it");
    ui.handle_event(&Event::PointerDown {
        position: Point::new(500.0, 350.0),
        button: PointerButton::Primary,
    });
    ui.update();
    assert!(!says(&mut ui, "A soma das contas."), "until a click elsewhere");
}

#[allow(clippy::expect_used)]
mod editor_states {
    use std::cell::Cell;
    use std::rc::Rc;

    use ravenrs_core::Role;

    use super::*;

    fn node_of(ui: &mut Ui, role: Role) -> ravenrs_core::AccessNode {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == role).cloned().expect("the node exists")
    }

    fn press_everything(ui: &mut Ui) {
        click(ui, Point::new(20.0, 12.0));
        key(ui, Key::Tab);
        key(ui, Key::Space);
        key(ui, Key::Enter);
        key(ui, Key::ArrowUp);
        key(ui, Key::Character("x".into()));
    }

    #[test]
    fn disabled_controls_ignore_input_and_leave_the_tab_order() {
        let (on, count, text) = (State::new(false), State::new(3), State::new("draft".to_owned()));
        let mut ui = ui(VStack::new((
            Toggle::new(on.binding()).disabled(true),
            Checkbox::new("Remember", on.binding()).disabled(true),
            Stepper::new(count.binding()).disabled(true),
            TextField::new(text.binding()).disabled(true),
        )));
        press_everything(&mut ui);
        assert!(!on.get());
        assert_eq!((count.get(), text.get().as_str()), (3, "draft"));
        for role in [Role::Switch, Role::CheckBox, Role::SpinButton, Role::TextInput] {
            let node = node_of(&mut ui, role);
            assert!(node.disabled, "{role:?} says it is disabled");
            assert!(!node.focusable, "{role:?} is out of the tab order");
        }
    }

    #[test]
    fn read_only_controls_stay_readable_and_keep_their_value() {
        let (on, count, text) = (State::new(false), State::new(3), State::new("draft".to_owned()));
        let mut ui = ui(VStack::new((
            Checkbox::new("Remember", on.binding()).read_only(true),
            Stepper::new(count.binding()).read_only(true),
            TextField::new(text.binding()).read_only(true),
        )));
        press_everything(&mut ui);
        key(&mut ui, Key::Tab);
        key(&mut ui, Key::ArrowUp);
        key(&mut ui, Key::Tab);
        key(&mut ui, Key::Backspace);
        key(&mut ui, Key::Character("y".into()));
        assert!(!on.get());
        assert_eq!((count.get(), text.get().as_str()), (3, "draft"));
        for role in [Role::CheckBox, Role::SpinButton, Role::TextInput] {
            let node = node_of(&mut ui, role);
            assert!(node.read_only && !node.disabled, "{role:?} says it is read-only");
            assert!(node.focusable, "{role:?} can still be reached");
        }
    }

    #[test]
    fn a_read_only_field_copies_and_does_not_submit() {
        let (text, submitted) = (State::new("draft".to_owned()), Rc::new(Cell::new(false)));
        let flag = submitted.clone();
        let mut ui = ui(TextField::new(text.binding()).read_only(true).on_submit(move |_| flag.set(true)));
        click(&mut ui, Point::new(40.0, 20.0));
        key(&mut ui, Key::Enter);
        ui.handle_event(&Event::KeyDown {
            key: Key::Character("x".into()),
            modifiers: Modifiers { meta: true, ..Modifiers::default() },
        });
        ui.update();
        assert!(!submitted.get());
        assert_eq!(text.get(), "draft", "cut does not remove anything");
    }

    #[test]
    fn pickers_open_only_when_they_accept_input() {
        let (index, pick, date) = (State::new(0usize), State::new(None::<usize>), State::new(None));
        let month = Date::new(2026, 11, 1).expect("a real date");
        // Neither, disabled, read-only: only the first opens.
        for (disabled, read_only) in [(false, false), (true, false), (false, true)] {
            let mut ui = ui(VStack::new((
                Select::new(index.binding(), ["A", "B"]).disabled(disabled).read_only(read_only),
                Combobox::new(pick.binding(), ["A", "B"]).disabled(disabled).read_only(read_only),
                DatePicker::new(date.binding(), month).disabled(disabled).read_only(read_only),
            )));
            for n in 0..3 {
                let spot = ui
                    .access_tree()
                    .nodes
                    .iter()
                    .filter(|n| n.role == Role::ComboBox)
                    .nth(n)
                    .map(|n| n.bounds.center());
                click(&mut ui, spot.expect("three pickers"));
                let tree = ui.access_tree();
                let open = tree.nodes.iter().filter(|n| n.role == Role::ComboBox && n.expanded == Some(true));
                assert_eq!(open.count(), usize::from(!disabled && !read_only), "picker {n}");
                key(&mut ui, Key::Escape);
            }
        }
    }

    #[test]
    fn plain_controls_keep_their_look() {
        let on = State::new(true);
        let plain = ui(Toggle::new(on.binding()));
        let off = ui(Toggle::new(on.binding()).disabled(false).read_only(false));
        assert_eq!(quads(&plain), quads(&off));
    }
}

#[allow(clippy::expect_used)]
mod commands {
    use std::cell::Cell;
    use std::rc::Rc;

    use ravenrs_core::Role;

    use super::*;

    fn button(ui: &mut Ui, name: &str) -> Option<Point> {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
            .map(|n| n.bounds.center())
    }

    fn bar(hits: Rc<Cell<u32>>) -> Toolbar {
        let mut bar = Toolbar::new().label("Commands");
        for name in ["Alpha", "Beta", "Gamma", "Delta"] {
            let hits = hits.clone();
            bar = bar.action(ToolbarAction::new(name).on_click(move || hits.set(hits.get() + 1)));
        }
        bar.action(ToolbarAction::new("Off").disabled(true))
    }

    #[test]
    fn a_wide_toolbar_shows_everything() {
        let hits = Rc::new(Cell::new(0));
        let mut ui = ui(bar(hits.clone()));
        ui.update();
        for name in ["Alpha", "Beta", "Gamma", "Delta", "Off"] {
            assert!(button(&mut ui, name).is_some(), "{name}");
        }
        assert!(button(&mut ui, "More").is_none());
        let spot = button(&mut ui, "Gamma").expect("Gamma");
        click(&mut ui, spot);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn a_narrow_toolbar_folds_into_a_menu_that_still_runs_the_commands() {
        let hits = Rc::new(Cell::new(0));
        let mut ui = ui(VStack::new((bar(hits.clone()),)).align(ravenrs_layout::Align::Stretch).width(150.0));
        ui.update();
        assert!(button(&mut ui, "Alpha").is_some(), "the first stays");
        assert!(button(&mut ui, "Delta").is_none(), "the last goes");
        let more = button(&mut ui, "More").expect("the menu");
        click(&mut ui, more);
        let item = {
            let tree = ui.access_tree();
            tree.nodes
                .iter()
                .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some("Delta"))
                .map(|n| n.bounds.center())
                .expect("Delta is in the menu")
        };
        let off = {
            let tree = ui.access_tree();
            tree.nodes.iter().any(|n| n.role == Role::MenuItem && n.name.as_deref() == Some("Off"))
        };
        assert!(!off, "a disabled command does not offer itself");
        click(&mut ui, item);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn split_button_runs_the_main_command_and_the_alternatives() {
        let (main, other) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let (m, o) = (main.clone(), other.clone());
        let mut ui = ui(SplitButton::new("Save")
            .on_click(move || m.set(m.get() + 1))
            .item(MenuItem::new("Save and close").on_select(move || o.set(o.get() + 1))));
        let spot = button(&mut ui, "Save").expect("main");
        click(&mut ui, spot);
        assert_eq!(main.get(), 1);
        let spot = button(&mut ui, "Save: More").expect("menu");
        click(&mut ui, spot);
        let item = {
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::MenuItem).map(|n| n.bounds.center()).expect("the item")
        };
        click(&mut ui, item);
        assert_eq!((main.get(), other.get()), (1, 1));
    }

    #[test]
    fn a_disabled_split_button_runs_nothing() {
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        let mut ui = ui(SplitButton::new("Save").disabled(true).on_click(move || h.set(1)));
        let spot = button(&mut ui, "Save").expect("main");
        click(&mut ui, spot);
        let spot = button(&mut ui, "Save: More").expect("menu");
        click(&mut ui, spot);
        assert_eq!(hits.get(), 0);
        let tree = ui.access_tree();
        assert!(!tree.nodes.iter().any(|n| n.role == Role::MenuItem), "the menu stays closed");
    }

    #[test]
    fn confirm_dialog_asks_and_runs_only_on_yes() {
        let confirm = ConfirmDialog::new();
        let hits = Rc::new(Cell::new(0));
        let mut ui = ui(confirm.clone());
        assert!(button(&mut ui, "Delete").is_none(), "nothing asked yet");
        let h = hits.clone();
        confirm.ask("Delete this?", "It cannot be undone.", "Delete", move || h.set(h.get() + 1));
        ui.update();
        let spot = button(&mut ui, "Cancel").expect("cancel");
        click(&mut ui, spot);
        assert_eq!(hits.get(), 0);
        assert!(button(&mut ui, "Delete").is_none(), "it went away");
        let h = hits.clone();
        confirm.ask("Delete this?", "It cannot be undone.", "Delete", move || h.set(h.get() + 1));
        ui.update();
        let spot = button(&mut ui, "Delete").expect("the action");
        click(&mut ui, spot);
        assert_eq!(hits.get(), 1);
        assert!(button(&mut ui, "Delete").is_none());
    }
}

#[allow(clippy::expect_used)]
mod forms {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ravenrs_core::Role;

    use super::*;

    fn options(ui: &mut Ui) -> Vec<String> {
        let tree = ui.access_tree();
        tree.nodes.iter().filter(|n| n.role == Role::ListBoxOption).filter_map(|n| n.name.clone()).collect()
    }

    fn open_field(ui: &mut Ui) {
        let spot = {
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::ComboBox).expect("the field").bounds.center()
        };
        click(ui, spot);
    }

    fn cities() -> Vec<LookupItem> {
        ["Belo Horizonte", "Recife", "Salvador"]
            .iter()
            .enumerate()
            .map(|(i, c)| LookupItem::new(i.to_string(), *c))
            .collect()
    }

    #[test]
    fn lookup_searches_picks_and_clears() {
        let city = State::new(None);
        let mut ui = ui(LookupField::new(city.binding(), cities()));
        open_field(&mut ui);
        assert_eq!(options(&mut ui).len(), 3);
        key(&mut ui, Key::Character("rec".to_owned()));
        assert_eq!(options(&mut ui), ["Recife"]);
        key(&mut ui, Key::Enter);
        assert_eq!(city.get().map(|c: LookupItem| c.key), Some("1".to_owned()));
        assert!(options(&mut ui).is_empty(), "it closed");
        let clear = {
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.name.as_deref() == Some("Clear")).expect("clear").bounds.center()
        };
        click(&mut ui, clear);
        assert_eq!(city.get(), None);
    }

    #[test]
    fn a_remote_lookup_shows_it_is_waiting_and_ignores_stale_answers() {
        let city = State::new(None);
        let asked: Rc<RefCell<Vec<(String, LookupResponder)>>> = Rc::default();
        let store = asked.clone();
        let mut ui = ui(LookupField::remote(city.binding(), move |request, responder| {
            store.borrow_mut().push((request.query.clone(), responder));
        })
        .limit(5));
        open_field(&mut ui);
        assert_eq!(asked.borrow().len(), 1, "asks once when it opens");
        let shows = |ui: &mut Ui, text: &str| {
            let tree = ui.access_tree();
            tree.nodes.iter().any(|n| n.name.as_deref() == Some(text))
        };
        assert!(shows(&mut ui, "Loading"), "says it is waiting");
        key(&mut ui, Key::Character("s".to_owned()));
        assert_eq!(asked.borrow().len(), 2, "asks again on a search");
        asked.borrow()[0].1.respond(vec![LookupItem::new("old", "Old answer")]);
        ui.update();
        assert!(options(&mut ui).is_empty(), "the first answer is out of date");
        asked.borrow()[1].1.respond(vec![LookupItem::new("2", "Salvador")]);
        ui.update();
        assert_eq!(options(&mut ui), ["Salvador"]);
        key(&mut ui, Key::Character("x".to_owned()));
        asked.borrow()[2].1.fail("No connection");
        ui.update();
        assert!(shows(&mut ui, "No connection"), "says what went wrong");
    }

    #[test]
    fn a_lookup_that_cannot_change_does_not_open() {
        let city = State::new(Some(LookupItem::new("1", "Recife")));
        for (disabled, read_only) in [(true, false), (false, true)] {
            let mut ui =
                ui(LookupField::new(city.binding(), cities()).disabled(disabled).read_only(read_only));
            open_field(&mut ui);
            assert!(options(&mut ui).is_empty());
            let tree = ui.access_tree();
            assert!(!tree.nodes.iter().any(|n| n.name.as_deref() == Some("Clear")), "nothing to clear with");
        }
    }

    fn field(name: &'static str, text: &State<String>) -> FormField<TextField> {
        FormField::new(name, TextField::new(text.binding()))
    }

    fn input_center(ui: &mut Ui, name: &str) -> Point {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some(name))
            .expect("the field")
            .bounds
            .center()
    }

    #[test]
    fn form_fields_share_rows_and_wide_ones_do_not() {
        let t = State::new(String::new());
        let mut ui = ui(FormLayout::new().columns(2).group(
            FormGroup::titled("Contact")
                .field(field("A", &t))
                .field(field("B", &t))
                .field(field("C", &t))
                .wide(field("D", &t)),
        ));
        let (a, b, c, d) = (
            input_center(&mut ui, "A"),
            input_center(&mut ui, "B"),
            input_center(&mut ui, "C"),
            input_center(&mut ui, "D"),
        );
        assert_eq!(a.y, b.y, "A and B on one row");
        assert!(a.x < b.x);
        assert!(c.y > a.y, "C on the next");
        assert_eq!(c.x, a.x, "C keeps A's column, though alone on its row");
        assert!(d.y > c.y);
        assert!(d.x > c.x, "D is centered on the whole width");
    }

    #[test]
    fn a_narrow_form_collapses_the_columns() {
        let t = State::new(String::new());
        let mut ui = ui(FormLayout::new()
            .columns(2)
            .group(FormGroup::new().field(field("A", &t)).field(field("B", &t))));
        ui.set_viewport(Size::new(300.0, 600.0), 1.0);
        ui.update();
        let (a, b) = (input_center(&mut ui, "A"), input_center(&mut ui, "B"));
        assert!(b.y > a.y, "one over the other");
        let wide = {
            let tree = ui.access_tree();
            tree.nodes
                .iter()
                .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some("A"))
                .map(|n| n.bounds.size.width)
        };
        assert!(wide.is_some_and(|w| w > 280.0), "each takes the whole width: {wide:?}");
    }

    #[test]
    fn form_errors_and_tabs_show() {
        let t = State::new(String::new());
        let mut ui = ui(FormLayout::new()
            .errors(["Name is required."])
            .group(FormGroup::new().field(field("A", &t)))
            .tab("First", Text::new("first body"))
            .tab("Second", Text::new("second body")));
        let tree = ui.access_tree();
        let has = |text: &str| tree.nodes.iter().any(|n| n.name.as_deref() == Some(text));
        assert!(has("Please review the form"));
        assert!(has("first body"));
        assert!(!has("second body"));
    }
}

#[allow(clippy::expect_used)]
mod filter_builder {
    use ravenrs_core::Role;

    use super::*;

    fn fields() -> Vec<FilterField> {
        vec![
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
        ]
    }

    fn press(ui: &mut Ui, name: &str) {
        let spot = {
            let tree = ui.access_tree();
            tree.nodes
                .iter()
                .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
                .expect("the button")
                .bounds
                .center()
        };
        click(ui, spot);
    }

    fn focus_value(ui: &mut Ui) {
        let spot = {
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the value").bounds.center()
        };
        click(ui, spot);
    }

    #[test]
    fn adding_a_line_writes_the_expression_when_it_has_a_value() {
        let filter = State::new(String::new());
        let mut ui = ui(FilterBuilder::new(filter.binding(), fields()));
        press(&mut ui, "Condition");
        assert_eq!(filter.get(), "", "no value yet");
        focus_value(&mut ui);
        key(&mut ui, Key::Character("ac".to_owned()));
        assert_eq!(filter.get(), "Contains([Name], 'ac')");
        press(&mut ui, "Remove");
        assert_eq!(filter.get(), "");
    }

    #[test]
    fn it_shows_an_expression_that_changes_from_outside() {
        let filter = State::new("[Price] > 10".to_owned());
        let mut ui = ui(FilterBuilder::new(filter.binding(), fields()));
        let shown = |ui: &mut Ui| {
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::TextInput).and_then(|n| n.value.clone())
        };
        assert_eq!(shown(&mut ui).as_deref(), Some("10"));
        filter.set("[Price] > 20".to_owned());
        ui.update();
        assert_eq!(shown(&mut ui).as_deref(), Some("20"));
    }

    #[test]
    fn a_line_being_written_is_not_lost_while_it_says_nothing() {
        let filter = State::new("[Price] > 10".to_owned());
        let mut ui = ui(FilterBuilder::new(filter.binding(), fields()));
        press(&mut ui, "Condition");
        let lines = {
            let tree = ui.access_tree();
            tree.nodes.iter().filter(|n| n.role == Role::TextInput).count()
        };
        assert_eq!(lines, 2, "the new line stays although the expression did not change");
        assert_eq!(filter.get(), "[Price] > 10");
    }
}

mod hotkeys {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
        ui.handle_event(&Event::KeyDown { key, modifiers });
        ui.update();
    }

    #[test]
    fn a_toolbar_command_answers_to_its_keys_and_stops_when_disabled() {
        let (save, all) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let (s, a) = (save.clone(), all.clone());
        let off = Rc::new(Cell::new(0));
        let o = off.clone();
        let mut ui = ui(Toolbar::new()
            .action(
                ToolbarAction::new("Save").hotkey(Hotkey::command("s")).on_click(move || s.set(s.get() + 1)),
            )
            .action(
                ToolbarAction::new("Save all")
                    .hotkey(Hotkey::command("s").shift())
                    .on_click(move || a.set(a.get() + 1)),
            )
            .action(
                ToolbarAction::new("Off")
                    .hotkey(Hotkey::command("d"))
                    .disabled(true)
                    .on_click(move || o.set(1)),
            ));
        let command = Modifiers { meta: true, ..Modifiers::default() };
        press(&mut ui, Key::Character("s".to_owned()), command);
        assert_eq!((save.get(), all.get()), (1, 0));
        press(&mut ui, Key::Character("S".to_owned()), Modifiers { shift: true, ..command });
        assert_eq!((save.get(), all.get()), (1, 1), "with Shift it is the other command");
        press(&mut ui, Key::Character("d".to_owned()), command);
        assert_eq!(off.get(), 0);
    }

    #[test]
    fn a_hotkey_writes_itself() {
        let label = Hotkey::command("s").shift().label();
        assert!(label.contains('S'));
        assert!(label.contains('⇧') || label.contains("Shift"));
        assert!(Hotkey::key(Key::Escape).label().contains("Esc"));
    }

    #[test]
    fn labels_reach_the_words_that_were_fixed() {
        let code = State::new(String::new());
        let mut ui = ui(Localized::new(Labels::pt_br(), InputOtp::new(code.binding(), 4)));
        let tree = ui.access_tree();
        assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Código de verificação")));
    }
}

fn node_named(ui: &mut Ui, name: &str) -> Option<u64> {
    ui.update();
    ui.access_tree().nodes.iter().find(|node| node.name.as_deref() == Some(name)).map(|node| node.id)
}

fn press_named(ui: &mut Ui, name: &str) {
    let id = node_named(ui, name).unwrap_or_else(|| panic!("no {name}"));
    ui.perform_action(id, ravenrs_core::AccessAction::Click);
    ui.update();
}

fn says_in_tree(ui: &mut Ui, text: &str) -> bool {
    ui.update();
    ui.access_tree().nodes.iter().any(|node| {
        node.name.as_deref().is_some_and(|name| name.contains(text))
            || node.value.as_deref().is_some_and(|value| value.contains(text))
    })
}

#[test]
fn theme_picker_says_the_theme_and_changes_to_the_one_chosen() {
    let mut ui = ui(VStack::new((ThemePicker::new(),)));
    assert_eq!(ui.theme().name, Theme::raven_dark().name);
    press_named(&mut ui, "Theme");
    press_named(&mut ui, "Light");
    assert_eq!(ui.theme().name, Theme::raven_light().name);
}

#[test]
fn theme_editor_counts_warns_restores_and_takes_tokens_back() {
    let mut ui = ui(ScrollView::new((ThemeEditor::new(),)));
    ui.set_viewport(Size::new(1200.0, 3000.0), 1.0);
    assert!(says_in_tree(&mut ui, "No changes"));
    assert!(says_in_tree(&mut ui, "bg-000"));
    assert!(says_in_tree(&mut ui, "Contrast"), "text says how legible it is");

    // Text the same color as its card is not legible, and the editor says so.
    ui.theme_state().update(|theme| theme.palette.ink = theme.palette.bg_200);
    assert!(says_in_tree(&mut ui, "Contrast 1.0:1 on bg-200"));
    assert!(says_in_tree(&mut ui, "1 token changed"));

    press_named(&mut ui, "Copy tokens");
    press_named(&mut ui, "Restore");
    assert!(says_in_tree(&mut ui, "No changes"));
    assert_eq!(ui.theme().palette.ink, Theme::raven_dark().palette.ink);

    press_named(&mut ui, "Paste tokens");
    assert!(says_in_tree(&mut ui, "1 token changed"), "what was copied comes back");
    assert_eq!(ui.theme().palette.ink, Theme::raven_dark().palette.bg_200);
}
