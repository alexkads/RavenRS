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
        ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, -120.0) })
    );
    assert!((first_y(&ui).expect("glyphs") - (before - 120.0)).abs() < 0.01);
    ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, -100_000.0) });
    ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, 100_000.0) });
    assert!((first_y(&ui).expect("glyphs") - before).abs() < 0.01, "clamped back to the top");
    assert!(
        !ui.handle_event(&Event::Scroll { position: Point::new(50.0, 100.0), delta: Point::new(0.0, 50.0) })
    );
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
    ui.update();
    assert!(ui.element_count() > 60);
    assert!(quads(&ui).len() > 100, "charts paint many primitives");
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
