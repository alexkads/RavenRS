//! One small example per component, for the component pages of the site. The
//! pages show these functions as they are, so the code a reader copies compiles.
//!
//! `scripts/gen-component-pages.py` reads the markers: `// sample: Name, height`
//! before a function and `// end` after it.

use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;

macro_rules! samples {
    ($($name:literal => $function:ident),* $(,)?) => {
        /// The name of every component with an example.
        pub const NAMES: &[&str] = &[$($name),*];

        /// The example of a component.
        pub fn sample(name: &str) -> Option<AnyView> {
            match name {
                $($name => Some(Box::new($function())),)*
                _ => None,
            }
        }
    };
}

samples! {
    "Button" => button, "IconButton" => icon_button, "ToggleButton" => toggle_button,
    "ToggleGroup" => toggle_group, "SegmentedControl" => segmented_control, "Toggle" => toggle,
    "Checkbox" => checkbox, "RadioGroup" => radio_group, "Slider" => slider, "Stepper" => stepper,
    "ColorSwatch" => color_swatch, "ThemeSwitcher" => theme_switcher, "ThemePicker" => theme_picker, "ThemeEditor" => theme_editor,
    "TextField" => text_field, "Select" => select, "Combobox" => combobox, "DatePicker" => date_picker,
    "Calendar" => calendar, "EventCalendar" => event_calendar, "Scheduler" => scheduler,
    "RecurrenceEditor" => recurrence_editor, "InputOtp" => input_otp, "FormField" => form_field, "Label" => label,
    "TextArea" => text_area, "NumberField" => number_field, "MaskedField" => masked_field,
    "TimePicker" => time_picker, "DateTimePicker" => date_time_picker, "LookupField" => lookup_field,
    "FormLayout" => form_layout, "FilterBuilder" => filter_builder, "Toolbar" => toolbar, "SplitButton" => split_button,
    "ConfirmDialog" => confirm_dialog,
    "TreeView" => tree_view, "TagBox" => tag_box, "ColorPicker" => color_picker,
    "DateRangePicker" => date_range_picker, "Wizard" => wizard, "StepIndicator" => step_indicator, "StepBadge" => step_badge, "FileUpload" => file_upload,
    "ImageField" => image_field, "TileLayout" => tile_layout, "CardView" => card_view,
    "AppShell" => app_shell,
    "Ribbon" => ribbon, "Menubar" => menubar, "Tabs" => tabs, "Sidebar" => sidebar,
    "Breadcrumb" => breadcrumb, "Pagination" => pagination, "Accordion" => accordion,
    "Disclosure" => disclosure,
    "Dialog" => dialog, "Sheet" => sheet, "Popover" => popover, "HoverCard" => hover_card,
    "Tooltip" => tooltip, "HelpTip" => help_tip, "SidebarBrand" => sidebar_brand, "DropdownMenu" => dropdown_menu, "ContextMenu" => context_menu,
    "SplashScreen" => splash_screen, "CommandPalette" => command_palette, "Toaster" => toaster,
    "DataGrid" => data_grid, "Table" => table, "List" => list, "Stat" => stat,
    "Chart" => chart, "ChartReference" => chart_reference, "SankeyChart" => sankey_chart, "ChartCard" => chart_card, "BarChart" => bar_chart, "AreaChart" => area_chart, "Barcode" => barcode,
    "VStack" => v_stack, "HStack" => h_stack, "Grid" => grid, "Spacer" => spacer,
    "ScrollView" => scroll_view, "Splitter" => splitter, "SplitPanes" => split_panes, "SplitHandle" => split_handle, "Card" => card, "GlassCard" => glass_card,
    "Section" => section, "Divider" => divider, "Footer" => footer, "StatusBar" => status_bar, "ToolRail" => tool_rail, "Timeline" => timeline, "AspectRatio" => aspect_ratio, "Collapse" => collapse,
    "MoviePlayer" => movie_player, "MovieAssistant" => movie_assistant, "Chat" => chat, "ModelPicker" => model_picker, "FloatingPanel" => floating_panel, "Text" => text, "CodeBlock" => code_block, "ReleaseNotes" => release_notes, "Alert" => alert, "Badge" => badge, "Avatar" => avatar, "Kbd" => kbd,
    "ProgressBar" => progress_bar, "Spinner" => spinner, "Skeleton" => skeleton,
    "Dot" => dot, "OverlayBadge" => overlay_badge, "Rating" => rating, "IconTile" => icon_tile, "EmptyState" => empty_state, "HeroBanner" => hero_banner, "MultiProgress" => multi_progress, "ShimmerText" => shimmer_text, "DaySeparator" => day_separator,
    "ShortcutSheet" => shortcut_sheet, "ShortcutHintBar" => shortcut_hint_bar,
    "FloatingToolbar" => floating_toolbar, "TitleBar" => title_bar, "DropZone" => drop_zone, "ScrollEdgeArrow" => scroll_edge_arrow, "ValueSlider" => value_slider, "InlineRename" => inline_rename,
    "ZStack" => z_stack, "PropertyRow" => property_row, "PageHeader" => page_header, "ButtonGroup" => button_group,
    "FavoriteToggle" => favorite_toggle, "SwatchGrid" => swatch_grid, "NinePointAnchor" => nine_point_anchor, "ColorPair" => color_pair,
    "Histogram" => histogram,
    "Ruler" => ruler,
    "ColorWheel" => color_wheel,
    "Icon" => icon, "Image" => image, "Lottie" => lottie, "Carousel" => carousel, "Canvas" => canvas,
}

/// A colored picture to show in the image examples.
fn picture(hue: f32) -> ImageData {
    ImageData::from_fn(160, 90, move |x, y| {
        let (u, v) = (x as f32 / 160.0, y as f32 / 90.0);
        Color::rgb(0.2 + 0.6 * u * hue, 0.3 + 0.4 * v, 0.9 - 0.5 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

// sample: Button, 200
fn button() -> impl View {
    let clicks = State::new(0);
    let add = clicks.clone();
    VStack::new((
        HStack::new((
            Button::new("Save").on_click(move || add.update(|n| *n += 1)),
            Button::new("Cancel").style(ButtonStyle::Secondary),
            Button::new("More").style(ButtonStyle::Ghost),
            Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2),
        ))
        .spacing(8.0)
        .wrap(),
        HStack::new((
            Button::new("Saving").loading(true),
            Button::new("Grid view").style(ButtonStyle::Secondary).selected(true),
            Button::new("Open").shortcut("F8"),
        ))
        .spacing(8.0)
        .wrap(),
        Text::bound(&clicks, |n| format!("Saved {n} times")).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
}
// end

// sample: IconButton, 90
fn icon_button() -> impl View {
    HStack::new((
        IconButton::new(IconName::Pencil).label("Edit"),
        IconButton::new(IconName::Copy).label("Duplicate"),
        IconButton::new(IconName::Settings).label("Settings").style(ButtonStyle::Secondary),
        IconButton::new(IconName::Plus).label("New").style(ButtonStyle::Primary),
        IconButton::new(IconName::Bold).label("Bold").selected(true),
    ))
    .spacing(8.0)
}
// end

// sample: ToggleButton, 90
fn toggle_button() -> impl View {
    let bold = State::new(true);
    let italic = State::new(false);
    let open = State::new(true);
    HStack::new((
        ToggleButton::new(bold.binding(), "Bold"),
        ToggleButton::new(italic.binding(), "Italic"),
        ToggleButton::new(open.binding(), "Open").count(12).pill(),
    ))
    .spacing(8.0)
}
// end

// sample: ToggleGroup, 90
fn toggle_group() -> impl View {
    let align = State::new(0);
    ToggleGroup::single(
        align.binding(),
        [
            (Some(IconName::AlignLeft), ""),
            (Some(IconName::AlignCenter), ""),
            (Some(IconName::AlignRight), ""),
        ],
    )
}
// end

// sample: SegmentedControl, 90
fn segmented_control() -> impl View {
    let period = State::new(1);
    SegmentedControl::new(period.binding(), ["Day", "Week", "Month"]).label("Period")
}
// end

// sample: Toggle, 110
fn toggle() -> impl View {
    let push = State::new(true);
    let email = State::new(false);
    VStack::new((
        Toggle::new(push.binding()).label("Push notifications"),
        Toggle::new(email.binding()).label("Email digest").small(),
    ))
    .spacing(12.0)
}
// end

// sample: Checkbox, 110
fn checkbox() -> impl View {
    let terms = State::new(true);
    let news = State::new(false);
    VStack::new((
        Checkbox::new("I accept the terms", terms.binding()),
        Checkbox::new("Send me product news", news.binding()),
    ))
    .spacing(10.0)
}
// end

// sample: RadioGroup, 150
fn radio_group() -> impl View {
    let plan = State::new(1);
    RadioGroup::new(plan.binding(), ["Starter", "Team", "Enterprise"]).label("Plan")
}
// end

// sample: Slider, 200
fn slider() -> impl View {
    let volume = State::new(0.6_f32);
    let (exposure, warmth) = (State::new(1.5_f32), State::new(0.4_f32));
    let shown = volume.clone();
    VStack::new((
        Slider::new(volume.binding()).label("Volume"),
        Text::dynamic(move || format!("{:.0}%", shown.get() * 100.0)).foreground(Semantic::InkMuted),
        // Fills from zero, a double click goes back to it, and the value sits at the right.
        Slider::new(exposure.binding())
            .range(-5.0, 5.0)
            .step(0.1)
            .neutral(0.0)
            .label("Exposure")
            .show_value(),
        // The colors on the track say what the value means.
        Slider::new(warmth.binding())
            .track_colors([Color::hex(0x3b82f6), Color::hex(0xfacc15)])
            .label("Temperature"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: TextArea, 170
fn text_area() -> impl View {
    let notes = State::new(String::from("Two lines\nto start with."));
    // Grows from 2 to 6 lines as you type; `.rows(4)` would fix the height.
    TextArea::new(notes.binding()).placeholder("Notes").grows(2, 6)
}
// end

// sample: NumberField, 110
fn number_field() -> impl View {
    let (seats, price) = (State::new(Some(12.0)), State::new(Some(129_990_i64)));
    VStack::new((
        NumberField::new(seats.binding()).range(0.0, 100.0).spin_buttons().label("Seats"),
        NumberField::money(price.binding(), "R$ ").label("Price"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: MaskedField, 90
fn masked_field() -> impl View {
    let phone = State::new(String::from("11987654321"));
    MaskedField::new(phone.binding(), "(00) 00000-0000").label("Phone")
}
// end

// sample: TimePicker, 90
fn time_picker() -> impl View {
    let at = State::new(Time::new(9, 30));
    TimePicker::new(at.binding()).step_minutes(15).label("Starts at")
}
// end

// sample: DateTimePicker, 90
fn date_time_picker() -> impl View {
    let month = Date::new(2026, 11, 1).unwrap_or_else(|| Date::from_days(0));
    let at = State::new(Time::new(14, 0).map(|time| DateTime { date: month, time }));
    DateTimePicker::new(at.binding(), month).label("Starts")
}
// end

// sample: LookupField, 90
fn lookup_field() -> impl View {
    let city = State::new(None);
    LookupField::remote(city.binding(), |request, responder| {
        // Ask a database or a server here; a clone of the responder can answer later.
        let cities = ["Belo Horizonte", "Recife", "Salvador", "São Paulo"];
        let needle = request.query.to_lowercase();
        let found = cities.iter().filter(|c| c.to_lowercase().contains(&needle));
        responder.respond(found.map(|c| LookupItem::new(*c, *c)).collect());
    })
    .placeholder("Choose a city")
    .width(220.0)
    .label("City")
}
// end

// sample: FormLayout, 260
fn form_layout() -> impl View {
    let (name, city, notes) =
        (State::new(String::new()), State::new(String::new()), State::new(String::new()));
    FormLayout::new().columns(2).group(
        FormGroup::titled("Contact")
            .field(FormField::new("Name", TextField::new(name.binding())).required())
            .field(FormField::new("City", TextField::new(city.binding())))
            .wide(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
    )
}
// end

// sample: Toolbar, 90
fn toolbar() -> impl View {
    Toolbar::new()
        .label("Records")
        .action(
            ToolbarAction::new("New")
                .icon(IconName::Plus)
                .style(ButtonStyle::Primary)
                .hotkey(Hotkey::command("n"))
                .on_click(|| {}),
        )
        .action(ToolbarAction::new("Refresh").icon(IconName::RefreshCw).on_click(|| {}))
        .separator()
        .action(ToolbarAction::new("Export").icon(IconName::Download).on_click(|| {}))
        .action(ToolbarAction::new("Delete").icon(IconName::Trash2).style(ButtonStyle::Danger).disabled(true))
}
// end

// sample: SplitButton, 90
fn split_button() -> impl View {
    SplitButton::new("Save")
        .icon(IconName::Check)
        .on_click(|| {})
        .item(MenuItem::new("Save and close").on_select(|| {}))
        .item(MenuItem::new("Save and new").on_select(|| {}))
}
// end

// sample: ConfirmDialog, 90
fn confirm_dialog() -> impl View {
    let confirm = ConfirmDialog::new();
    let asker = confirm.clone();
    VStack::new((
        Button::new("Delete").style(ButtonStyle::Danger).on_click(move || {
            asker.ask("Delete this record?", "This cannot be undone.", "Delete", || {});
        }),
        confirm,
    ))
}
// end

// sample: FilterBuilder, 240
fn filter_builder() -> impl View {
    let filter = State::new(String::from("[Price] > 10 And Contains([Name], 'pro')"));
    FilterBuilder::new(
        filter.binding(),
        [
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
            FilterField::new("Active", "Active", FilterFieldKind::Boolean),
        ],
    )
}
// end

// sample: AppShell, 620
fn app_shell() -> impl View {
    use ravenrs_app::{
        Application, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace,
    };
    use ravenrs_app_ui::{AppShell, EditorRegistry, SystemModule, Words};

    let bank = EntityInfo::new("Bank")
        .caption("Bancos")
        .navigation("Cadastros")
        .icon("landmark")
        .field(FieldInfo::new("code", FieldKind::Text).caption("Código").typed_key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property());
    let account = EntityInfo::new("Account")
        .caption("Contas")
        .navigation("Cadastros")
        .icon("wallet")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(
            FieldInfo::new(
                "kind",
                FieldKind::Enum(vec![
                    EnumOption::new("checking", "Corrente"),
                    EnumOption::new("savings", "Poupança"),
                ]),
            )
            .caption("Tipo")
            .default_value("checking"),
        )
        .field(FieldInfo::new("bank", FieldKind::Reference("Bank".into())).caption("Banco"))
        .field(FieldInfo::new("opening", FieldKind::Money).caption("Saldo inicial").default_value(0.0))
        .field(FieldInfo::new("opened", FieldKind::Date).caption("Aberta em"))
        .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativa").default_value(true))
        .field(FieldInfo::new("notes", FieldKind::LongText).caption("Observações"));
    let entities = vec![account, bank];
    let Ok(database) = MemoryDatabase::new(entities.clone()) else {
        unreachable!("the entities are declared right above");
    };
    let mut setup = database.space();
    for (code, name) in [("001", "Banco do Brasil"), ("341", "Itaú")] {
        if let Ok(mut object) = setup.new_object("Bank") {
            object.set("code", code);
            object.set("name", name);
            let _ = setup.save(object);
        }
    }
    for (name, cents) in [("Conta corrente", 125_050.0), ("Poupança", 980_000.0), ("Carteira", 12_000.0)] {
        if let Ok(mut object) = setup.new_object("Account") {
            object.set("name", name);
            object.set("opening", cents);
            object.set("bank", "341");
            let _ = setup.save(object);
        }
    }
    let _ = setup.commit();
    let builder = Application::builder("Raven Contas").module(SystemModule::with_words(Words::pt_br()));
    let application = entities.into_iter().fold(builder, |builder, entity| builder.entity(entity)).build();
    application.ok().map(|application| {
        Localized::new(
            Labels::pt_br(),
            AppShell::with_options(
                application,
                move || database.space(),
                Words::pt_br(),
                EditorRegistry::default(),
            ),
        )
    })
}
// end

// sample: TreeView, 260
fn tree_view() -> impl View {
    let selected = State::new(Some(String::from("invoices")));
    let opened = State::new(std::collections::BTreeSet::from([String::from("sales")]));
    TreeView::new(vec![
        TreeNode::new("sales", "Sales")
            .icon(IconName::Folder)
            .child(TreeNode::new("invoices", "Invoices").icon(IconName::FileText))
            .child(TreeNode::new("quotes", "Quotes").icon(IconName::FileText)),
        TreeNode::new("stock", "Stock")
            .icon(IconName::Folder)
            .child(TreeNode::new("items", "Items").icon(IconName::FileText)),
        TreeNode::new("reports", "Reports").icon(IconName::FileText),
    ])
    .selected(selected.binding())
    .expanded(opened.binding())
}
// end

// sample: TagBox, 300
fn tag_box() -> impl View {
    let tags = State::new(vec![String::from("urgent")]);
    TagBox::new(
        tags.binding(),
        vec![
            LookupItem::new("urgent", "Urgent"),
            LookupItem::new("review", "To review"),
            LookupItem::new("paid", "Paid"),
        ],
    )
    .allow_new(true)
    .label("Tags")
}
// end

// sample: ColorPicker, 420
fn color_picker() -> impl View {
    let color = State::new(Some(Color::hex(0x2563EB)));
    ColorPicker::new(color.binding())
        .alpha(true)
        .palette([
            (Color::hex(0x2563EB), "Blue"),
            (Color::hex(0x16A34A), "Green"),
            (Color::hex(0xDC2626), "Red"),
            (Color::hex(0xF59E0B), "Amber"),
        ])
        .label("Category color")
}
// end

// sample: DateRangePicker, 420
fn date_range_picker() -> impl View {
    let today = Date::new(2026, 11, 2).unwrap_or_else(|| unreachable!("a valid date"));
    let period = State::new(Some(DateRange::new(today, today.add_days(6))));
    DateRangePicker::new(period.binding(), today)
        .today(today)
        .preset("Last 7 days", DateRange::new(today.add_days(-6), today))
        .preset("Last 30 days", DateRange::new(today.add_days(-29), today))
        .label("Period")
}
// end

// sample: StepIndicator, 180
fn step_indicator() -> impl View {
    StepIndicator::new(vec![
        IndicatorStep::new("Client", StepState::Done),
        IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
        IndicatorStep::new("Review", StepState::Current),
        IndicatorStep::new("Send", StepState::Upcoming),
    ])
    .vertical()
    .on_select(|_step| {})
}
// end

// sample: StepBadge, 80
fn step_badge() -> impl View {
    HStack::new((
        StepBadge::new(1).state(StepState::Done),
        StepBadge::new(2).state(StepState::Error),
        StepBadge::new(3).state(StepState::Current),
        StepBadge::new(4),
    ))
    .spacing(8.0)
}
// end

// sample: Wizard, 280
fn wizard() -> impl View {
    let step = State::new(0);
    let name = State::new(String::new());
    let filled = name.clone();
    Wizard::new(
        step.binding(),
        vec![
            WizardStep::new("Account", FormField::new("Name", TextField::new(name.binding()))).check(
                move || {
                    if filled.get().trim().is_empty() { Err("Fill in the name".to_owned()) } else { Ok(()) }
                },
            ),
            WizardStep::new("Opening balance", Text::new("How much it holds today")),
            WizardStep::new("Review", Text::new("Everything is ready")),
        ],
    )
    .on_finish(|| {})
}
// end

// sample: FileUpload, 240
fn file_upload() -> impl View {
    let files = State::new(Vec::<PickedFile>::new());
    FileUpload::new(files.binding()).accept(["pdf", "png", "jpg"]).most_files(5).label("Receipts")
}
// end

// sample: ImageField, 220
fn image_field() -> impl View {
    let photo = State::new(None::<PickedFile>);
    ImageField::new(photo.binding()).size(180.0, 120.0).label("Photo")
}
// end

// sample: TileLayout, 400
fn tile_layout() -> impl View {
    let spots = State::new(vec![TileSpot::new("sales", 2, 1)]);
    TileLayout::new(spots.binding())
        .columns(3)
        .row_height(110.0)
        .tile(Tile::new("sales", "Sales", Stat::new("This month", "R$ 48,200")))
        .tile(Tile::new("bills", "Bills", Stat::new("Open", "12")))
        .tile(Tile::new("stock", "Stock", Stat::new("Items", "1,204")))
        .tile(Tile::new("people", "People", Stat::new("Active", "37")))
}
// end

// sample: CardView, 340
fn card_view() -> impl View {
    let selected = State::new(std::collections::BTreeSet::from([1]));
    CardView::new(10_000, |i| -> AnyView {
        Box::new(
            VStack::new((
                Text::new(format!("Customer {i}")).font_weight(Weight::SEMIBOLD),
                Text::new(format!("R$ {},00", 100 + i * 7)).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        )
    })
    .card_name(|i| format!("Customer {i}"))
    .card_width(180.0)
    .card_height(84.0)
    .selection(selected.binding())
    .height(300.0)
}
// end

// sample: Stepper, 90
fn stepper() -> impl View {
    let seats = State::new(3);
    Stepper::new(seats.binding()).range(1, 50).label("Seats")
}
// end

// sample: ColorSwatch, 90
fn color_swatch() -> impl View {
    let current = State::new(0);
    view_fn(move || {
        let theme = Theme::raven_light();
        let options =
            [(Semantic::Accent, "Accent"), (Semantic::Success, "Success"), (Semantic::Danger, "Danger")];
        let swatches: Vec<AnyView> = options
            .iter()
            .enumerate()
            .map(|(i, (role, name))| {
                let pick = current.clone();
                Box::new(
                    ColorSwatch::new(theme.color(*role), *name)
                        .selected(current.get() == i)
                        .on_click(move || pick.set(i)),
                ) as AnyView
            })
            .collect();
        HStack::new(swatches)
    })
}
// end

// sample: ThemeSwitcher, 90
fn theme_switcher() -> impl View {
    ThemeSwitcher
}
// end

// sample: ThemePicker, 90
fn theme_picker() -> impl View {
    ThemePicker::new()
}
// end

// sample: ThemeEditor, 900
fn theme_editor() -> impl View {
    ThemeEditor::new()
}
// end

// sample: TextField, 180
fn text_field() -> impl View {
    let name = State::new(String::new());
    let price = State::new(String::from("49,90"));
    let shown = name.clone();
    VStack::new((
        TextField::new(name.binding()).placeholder("Full name").icon(IconName::User).clearable(),
        TextField::new(price.binding())
            .placeholder("Price")
            .size(ButtonSize::Small)
            .prefix(Text::new("R$").foreground(Semantic::InkMuted))
            .suffix(Badge::new("per month")),
        Text::dynamic(move || format!("Hello, {}", shown.get())).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Select, 320
fn select() -> impl View {
    let range = State::new(0);
    // `Select::of` holds the value itself; `.searchable()` adds a search box.
    let currency = State::new("BRL");
    VStack::new((
        Select::new(range.binding(), ["Last 7 days", "Last 30 days", "This year"]).label("Date range"),
        Select::of(currency.binding(), [("BRL", "Real"), ("EUR", "Euro"), ("USD", "Dollar")])
            .label("Currency")
            .size(ButtonSize::Small)
            .searchable(),
    ))
    .spacing(8.0)
}
// end

// sample: Combobox, 320
fn combobox() -> impl View {
    let country = State::new(None);
    Combobox::new(country.binding(), ["Argentina", "Brazil", "Chile", "Portugal", "Spain"])
        .placeholder("Choose a country")
        .label("Country")
}
// end

// sample: DatePicker, 420
fn date_picker() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(None);
    DatePicker::new(date.binding(), today).today(today).label("Date")
}
// end

// sample: Calendar, 380
fn calendar() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(Some(today));
    Calendar::new(date.binding(), today).today(today)
}
// end

// sample: EventCalendar, 620
fn event_calendar() -> impl View {
    let today = Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"));
    let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
    EventCalendar::new(month.binding(), day.binding()).today(today).events([
        CalendarEvent::new(1, today.add_days(-4), "Rent").done(true),
        CalendarEvent::new(2, today, "Review").tone(Tone::Warning),
        CalendarEvent::new(3, today.add_days(3), "Salary").tone(Tone::Success),
    ])
}
// end

// sample: Scheduler, 400
fn scheduler() -> impl View {
    let first = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let at = |day: i64| TimePoint::from_date(first).plus_days(day);
    let axis = State::new(TimeAxis::days(first, 30));
    Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Carla")])
        .events([
            SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
            SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
            SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        ])
        .on_change(|change| println!("{change:?}"))
}
// end

// sample: RecurrenceEditor, 280
fn recurrence_editor() -> impl View {
    let start = Date::new(2026, 11, 5).unwrap_or_else(|| unreachable!("a real date"));
    let rule = State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4]));
    RecurrenceEditor::new(rule.binding(), start)
}
// end

// sample: InputOtp, 90
fn input_otp() -> impl View {
    let code = State::new(String::new());
    InputOtp::new(code.binding(), 6)
}
// end

// sample: FormField, 130
fn form_field() -> impl View {
    let email = State::new(String::new());
    FormField::new("Work email", TextField::new(email.binding()).placeholder("you@company.com"))
        .required()
        .hint("We send the invoice here.")
        .validate(email.binding(), validate::email)
}
// end

// sample: Label, 80
fn label() -> impl View {
    Label::new("Company name")
}
// end

// sample: Ribbon, 240
fn ribbon() -> impl View {
    Ribbon::new()
        .app_menu("File", vec![MenuItem::new("Save").icon(IconName::Save).shortcut("⌘S")])
        .quick_access(RibbonItem::button("Undo", IconName::Undo2))
        .page(
            RibbonPage::new("Home")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::button("Paste", IconName::ClipboardPaste).large())
                        .item(RibbonItem::button("Copy", IconName::Copy).small()),
                )
                .group(
                    RibbonGroup::new("Output").item(RibbonItem::button("Print", IconName::Printer).large()),
                ),
        )
        .page(
            RibbonPage::new("Insert")
                .group(RibbonGroup::new("Items").item(RibbonItem::button("Image", IconName::Image).large())),
        )
}
// end

// sample: Menubar, 240
fn menubar() -> impl View {
    Menubar::new()
        .menu("File")
        .item(MenuItem::new("New").icon(IconName::FilePlus).shortcut("⌘N"))
        .separator()
        .item(MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"))
        .menu("Edit")
        .item(MenuItem::new("Undo").icon(IconName::Undo2).shortcut("⌘Z"))
}
// end

// sample: Tabs, 170
fn tabs() -> impl View {
    let tab = State::new(0);
    Tabs::new(tab.binding())
        .tab("Activity", Text::new("Ana merged #482").foreground(Semantic::InkMuted))
        .tab("Settings", Text::new("Each tab builds when selected.").foreground(Semantic::InkMuted))
        .tab("Billing", Stat::new("Next invoice", "R$ 1.240"))
        // `TabsVariant::Segmented` makes it a track, `.item` a tab without content.
        .variant(TabsVariant::Underline)
}
// end

// sample: Sidebar, 440
fn sidebar() -> impl View {
    let state = SidebarState::new(true);
    HStack::new((
        Sidebar::new((
            SidebarGroup::new(
                "Workspace",
                (
                    SidebarItem::new(IconName::LayoutDashboard, "Overview").selected(true),
                    SidebarItem::new(IconName::Users, "Customers").badge(Badge::new("12")),
                    SidebarItem::new(IconName::Settings, "Settings")
                        .item(SidebarItem::text("General").compact())
                        .item(SidebarItem::text("Billing").compact()),
                ),
            ),
            SidebarGroup::new("Projects", (SidebarSkeleton::new(2),)).collapsible(),
        ))
        .collapsible(SidebarCollapse::Icons)
        .state(state.clone())
        .footer(
            HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                .spacing(10.0)
                .align(Align::Center),
        ),
        VStack::new((SidebarTrigger::new(state), Text::new("Or press Cmd/Ctrl+B.")))
            .spacing(12.0)
            .padding(16.0)
            .grow(1.0),
    ))
    .height(420.0)
    .align(Align::Stretch)
}
// end

// sample: Breadcrumb, 80
fn breadcrumb() -> impl View {
    // `on_navigate` makes every location but the last one a link.
    Breadcrumb::new(["Workspace", "Projects", "RavenRS", "Settings"]).on_navigate(|_level| {})
}
// end

// sample: Pagination, 90
fn pagination() -> impl View {
    let page = State::new(0);
    Pagination::new(page.binding(), 24)
}
// end

// sample: Accordion, 230
fn accordion() -> impl View {
    Accordion::new(vec![
        Disclosure::new("What platforms does it support?", Text::new("Desktop and browsers with WebGPU."))
            .open(true),
        Disclosure::new("Can I build my own components?", Text::new("Yes, with the same public API.")),
    ])
}
// end

// sample: Disclosure, 130
fn disclosure() -> impl View {
    Disclosure::new("Advanced options", Text::new("Shown when open.").foreground(Semantic::InkMuted))
        // Tells what is inside while it is closed. `.switch` and `.expanded` take bindings.
        .mark(Dot::new().tone(Tone::Warning).label("Changed"))
}
// end

// sample: Dialog, 340
fn dialog() -> impl View {
    let open = State::new(false);
    let (show, cancel, confirm) = (open.clone(), open.clone(), open.clone());
    VStack::new((
        Button::new("Delete project").style(ButtonStyle::Danger).on_click(move || show.set(true)),
        Dialog::new(open.binding(), "Delete the project?")
            .description("This cannot be undone.")
            .action(Button::new("Cancel").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)))
            .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || confirm.set(false)))
            // A stray click outside must not close this one.
            .close_on_backdrop(false),
    ))
}
// end

// sample: Sheet, 380
fn sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    let active = State::new(true);
    VStack::new((
        Button::new("Open filters").style(ButtonStyle::Secondary).on_click(move || show.set(true)),
        Sheet::new(open.binding(), "Filters")
            .description("Narrow down the accounts shown.")
            .content(Toggle::new(active.binding()).label("Only active accounts")),
    ))
}
// end

// sample: SplashScreen, 380
fn splash_screen() -> impl View {
    /// Pretends to load for three seconds. A real app sets the same values as its work advances.
    struct Loading;

    impl View for Loading {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            const SECONDS: f64 = 3.0;
            let visible = cx.use_state(|| false);
            let started = cx.use_state(|| None::<f64>);
            let now = cx.now();
            let elapsed = match (visible.get(), started.get()) {
                (true, Some(start)) => now - start,
                (true, None) => {
                    started.set(Some(now));
                    0.0
                }
                (false, _) => {
                    started.set_if_changed(None);
                    0.0
                }
            };
            if elapsed >= SECONDS {
                visible.set(false);
            } else if visible.get() {
                cx.wake_at(now + 0.25);
            }
            let status = match elapsed {
                e if e < 1.0 => "Opening the database",
                e if e < 2.0 => "Loading your accounts",
                _ => "Almost there",
            };
            let show = visible.clone();
            let splash =
                SplashScreen::new(visible.binding(), "Raven Finance").icon(IconName::Wallet).status(status);
            // A spinner until there is something to measure, then a bar.
            let splash = if elapsed < 1.0 { splash } else { splash.progress((elapsed / SECONDS) as f32) };
            let page = VStack::new((
                Button::new("Show the splash screen")
                    .style(ButtonStyle::Secondary)
                    .on_click(move || show.set(true)),
                splash,
            ));
            cx.child(&page)
        }
    }

    Loading
}
// end

// sample: Popover, 240
fn popover() -> impl View {
    let open = State::new(false);
    let flip = open.clone();
    Popover::new(
        open.binding(),
        Button::new("Show details").style(ButtonStyle::Secondary).on_click(move || flip.update(|o| *o = !*o)),
        VStack::new((Text::new("Storage").style(TextStyle::Headline), ProgressBar::new(0.62)))
            .spacing(8.0)
            .width(220.0),
    )
    // `Placement::Above` and the others work too; `.surface(false)` drops the panel.
    .placement(Placement::Below)
}
// end

// sample: HoverCard, 220
fn hover_card() -> impl View {
    HoverCard::new(
        Text::new("@ana.lima").foreground(Semantic::Accent),
        HStack::new((Avatar::new("Ana Lima"), Text::new("Design lead at Northwind"))).spacing(10.0),
    )
}
// end

// sample: Tooltip, 130
fn tooltip() -> impl View {
    VStack::new((
        Spacer::new().min(40.0),
        HStack::new((
            Tooltip::new(Button::new("Hover me").style(ButtonStyle::Secondary), "It stays out of the way"),
            Button::new("Save").style(ButtonStyle::Secondary).tooltip("Save the file"),
            Tooltip::new(Button::new("Open").style(ButtonStyle::Secondary), "Open a file").shortcut("⌘O"),
        ))
        .spacing(8.0),
    ))
}
// end

// sample: HelpTip, 130
fn help_tip() -> impl View {
    HStack::new((
        Text::new("Recurring"),
        HelpTip::new("Recurring", "It comes back on the same day of each month.")
            .how("Choose it when you save."),
    ))
    .spacing(6.0)
}
// end

// sample: SidebarBrand, 90
fn sidebar_brand() -> impl View {
    SidebarBrand::new(Icon::new(IconName::Wallet), "Raven").subtitle("Family finances")
}
// end

// sample: DropdownMenu, 360
fn dropdown_menu() -> impl View {
    DropdownMenu::new("Actions")
        .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E"))
        .item(MenuItem::new("Share").icon(IconName::Share2))
        .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")))
        .item(MenuItem::new("Show grid").checked(true))
        .item(MenuItem::new("Label").element(Badge::new("Family").tone(Tone::Accent)))
        .separator()
        .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
// end

// sample: ContextMenu, 240
fn context_menu() -> impl View {
    ContextMenu::new(
        Text::new("Right-click in this box")
            .foreground(Semantic::InkMuted)
            .centered()
            .height(90.0)
            .background(Semantic::Control)
            .corner_radius(12.0),
    )
    .item(MenuItem::new("Copy").icon(IconName::Copy).shortcut("⌘C"))
    .item(MenuItem::new("Sort by").item(MenuItem::new("Name")).item(MenuItem::new("Date")))
    .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
// end

// sample: CommandPalette, 400
fn command_palette() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Open palette").icon(IconName::Command).on_click(move || show.set(true)),
        CommandPalette::new(open.binding())
            .command(Command::new("Go to reports", || {}).icon(IconName::FileText).group("Pages"))
            .command(Command::new("Open settings", || {}).icon(IconName::Settings).group("Pages")),
    ))
}
// end

// sample: Toaster, 240
fn toaster() -> impl View {
    let toasts = Toasts::new();
    let show = toasts.clone();
    VStack::new((
        Button::new("Show toast")
            .on_click(move || show.show("Report exported", "sales-q3.pdf is ready.", Tone::Success)),
        // `toasts.show_until_closed` keeps one up until it is closed.
        Toaster::new(toasts).position(ToastPosition::BottomRight),
    ))
}
// end

// sample: DataGrid, 420
fn data_grid() -> impl View {
    let rows = (0..1000)
        .map(|i| {
            vec![
                Value::Number(f64::from(i)),
                Value::Text(format!("Customer {i}")),
                Value::Number(f64::from(i % 90) * 13.5),
            ]
        })
        .collect();
    DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::number("ID").width(90.0),
            GridColumn::text("Customer").width(220.0).editable(),
            GridColumn::currency("MRR", "R$ ", 2).width(150.0).summary(Summary::Sum),
        ],
    )
    .height(380.0)
}
// end

// sample: Table, 190
fn table() -> impl View {
    Table::new([Column::new("Invoice"), Column::new("Customer").weight(2.0), Column::new("Amount").numeric()])
        .row(["INV-1042", "Northwind Labs", "R$ 4.800"])
        .row(["INV-1043", "Aurora Studio", "R$ 1.720"])
}
// end

// sample: List, 210
fn list() -> impl View {
    List::new((
        ListItem::new("Ana Lima").subtitle("ana@northwind.dev").avatar("Ana Lima").trailing("Owner"),
        ListItem::new("Report.pdf").subtitle("2.4 MB").icon(IconName::FileText).on_click(|| {}),
    ))
}
// end

// sample: Stat, 120
fn stat() -> impl View {
    HStack::new((
        Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true),
        Stat::new("Churn", "1.8%").delta("-0.4%", false),
    ))
    .spacing(32.0)
}
// end

// sample: Chart, 280
fn chart() -> impl View {
    Chart::new(ChartKind::Line)
        .title("Sales")
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
        .series("2026", [14.0, 21.0, 26.0, 24.0, 31.0, 35.0])
        .height(260.0)
}
// end

// sample: ChartReference, 280
fn chart_reference() -> impl View {
    Chart::new(ChartKind::Bar)
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("Sales", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
        .show_value_axis()
        .grid_dashed()
        .value_domain(0.0, 25.0)
        .tooltip(ChartTooltip::new().value_format(|value| format!("{value:.0} units")))
        .reference(ChartReference::new(20.0).label("Target").color(Semantic::Warning))
        .height(260.0)
}
// end

// sample: SankeyChart, 340
fn sankey_chart() -> impl View {
    SankeyChart::new()
        .title("Where the money goes")
        .flow("Salary", "Taxes", 1200.0)
        .flow("Salary", "Home", 1500.0)
        .flow("Salary", "Savings", 800.0)
        .flow("Freelance", "Savings", 600.0)
        .flow("Home", "Rent", 1100.0)
        .flow("Home", "Power", 400.0)
        .flow("Savings", "Funds", 900.0)
        .flow("Savings", "Stocks", 500.0)
        .format(|value| format!("$ {value:.0}"))
        .height(300.0)
}
// end

// sample: ChartCard, 420
fn chart_card() -> impl View {
    ChartCard::new(
        Chart::new(ChartKind::Area)
            .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
            .series("Desktop", [186.0, 305.0, 237.0, 73.0, 209.0, 214.0])
            .series("Mobile", [80.0, 200.0, 120.0, 190.0, 130.0, 140.0])
            .stack(ChartStack::Normal)
            .gradient()
            .height(220.0),
    )
    .title("Visitors")
    .description("January to June")
    .trend("Up by 5.2% this month", IconName::TrendingUp)
    .note("All channels")
}
// end

// sample: BarChart, 220
fn bar_chart() -> impl View {
    BarChart::new([("Mon", 42.0), ("Tue", 58.0), ("Wed", 51.0), ("Thu", 73.0), ("Fri", 66.0)]).height(180.0)
}
// end

// sample: AreaChart, 220
fn area_chart() -> impl View {
    AreaChart::new([12.0, 14.0, 13.0, 17.0, 16.0, 21.0, 19.0, 24.0, 23.0, 27.0]).height(180.0)
}
// end

// sample: Barcode, 190
fn barcode() -> impl View {
    HStack::new((
        Barcode::qr("https://ravenrs.dev").alt("Link to the RavenRS site"),
        Barcode::new(Symbology::Ean13, "789123456789"),
    ))
    .spacing(24.0)
}
// end

// sample: VStack, 150
fn v_stack() -> impl View {
    VStack::new((Text::new("First"), Text::new("Second"), Button::new("Third"))).spacing(12.0)
}
// end

// sample: HStack, 90
fn h_stack() -> impl View {
    HStack::new((Text::new("First"), Text::new("Second"), Button::new("Third")))
        .spacing(12.0)
        .align(Align::Center)
}
// end

// sample: Grid, 170
fn grid() -> impl View {
    let cell = |n: u32| Card::new(Text::new(n.to_string())).padding(16.0);
    Grid::new(3, (cell(1), cell(2), cell(3), cell(4), cell(5), cell(6))).spacing(8.0)
}
// end

// sample: Spacer, 90
fn spacer() -> impl View {
    HStack::new((Text::new("Left"), Spacer::new(), Button::new("Right"))).align(Align::Center)
}
// end

// sample: ScrollView, 220
fn scroll_view() -> impl View {
    let rows: Vec<AnyView> = (1..=30).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new(rows).spacing(8.0).height(180.0)
}
// end

// sample: Splitter, 230
fn splitter() -> impl View {
    Splitter::horizontal(Text::new("Explorer").padding(16.0), Text::new("Editor").padding(16.0))
        .ratio(0.32)
        .height(190.0)
}
// end

// sample: SplitPanes, 230
fn split_panes() -> impl View {
    SplitPanes::horizontal()
        .pane(SplitPane::new(Text::new("Sessions").padding(16.0)).size(180.0).range(120.0, 260.0))
        .pane(SplitPane::new(Text::new("Receipt").padding(16.0)).range(120.0, f32::MAX))
        .pane(SplitPane::new(Text::new("Activity").padding(16.0)).size(180.0).range(120.0, 260.0))
        .height(190.0)
}
// end

// sample: SplitHandle, 120
fn split_handle() -> impl View {
    let height = State::new(120.0_f32);
    // The size is yours to use; the bar only changes it.
    VStack::new((
        Text::new("Drag the bar, or focus it and use the arrow keys."),
        SplitHandle::new(Axis::Vertical, height.binding()).range(80.0, 300.0).inverted(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
// end

// sample: Card, 150
fn card() -> impl View {
    Card::new(
        VStack::new((
            Text::new("Quarterly report").style(TextStyle::Headline),
            Text::new("A grouped surface, lifted off the page.").foreground(Semantic::InkMuted),
        ))
        .spacing(4.0),
    )
    // Set it on the chosen card of a group.
    .selected(false)
}
// end

// sample: GlassCard, 150
fn glass_card() -> impl View {
    GlassCard::new(Text::new("Translucent, for Raven Glass compositions."))
}
// end

// sample: Section, 130
fn section() -> impl View {
    Section::new(
        "Notifications",
        (Text::new("Grouped content under a title.").foreground(Semantic::InkMuted),),
    )
    .action(Button::new("Edit").style(ButtonStyle::Secondary).size(ButtonSize::Small))
}
// end

// sample: Divider, 110
fn divider() -> impl View {
    VStack::new((Text::new("Above"), Divider::horizontal(), Text::new("Below")))
        .spacing(12.0)
        .align(Align::Stretch)
}
// end

// sample: Footer, 170
fn footer() -> impl View {
    Footer::new()
        .left((Footer::text("© 2026 Raven Finance"),))
        .center((Footer::link("Privacy"), Footer::link("Terms"), Footer::link("Help")))
        .right((Footer::text("v27.1"), Badge::new("Online").tone(Tone::Success)))
}
// end

// sample: StatusBar, 60
fn status_bar() -> impl View {
    StatusBar::new()
        .left((StatusBar::action("Version 27.1"),))
        .center((
            StatusBar::text("Version 27.2 is available").truncate().grow(1.0),
            StatusBar::action("What's new"),
            StatusBar::action("Update").style(ButtonStyle::Primary),
        ))
        .center_tone(Tone::Warning)
        .right((StatusBar::text("12 items"),))
}
// end

// sample: MoviePlayer, 520
fn movie_player() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MoviePlayer};
    // Real films come from `Recorder`, which runs the app from a script.
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=6_u64 {
        let square =
            Quad::new(Rect::new(20.0 + second as f32 * 40.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("A square", "It goes across the screen.", 0);
    film.finish(6000).map(|film| MoviePlayer::new(film).width(640.0)).ok()
}
// end

// sample: MovieAssistant, 760
fn movie_assistant() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MovieAssistant, MovieLibrary};
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=9_u64 {
        let square =
            Quad::new(Rect::new(20.0 + second as f32 * 25.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("Move a square", "How it goes across the screen.", 0);
    film.chapter("Save your work", "Keep a copy of what you did.", 5000);
    // Accents and case do not matter, and near words count: try "saving".
    film.finish(9000)
        .map(|film| MovieAssistant::new(MovieLibrary::new().film("Manual", film)).width(640.0))
        .ok()
}
// end

// sample: Chat, 420
fn chat() -> impl View {
    /// Keeps the messages, here while the page rebuilds. A real app keeps them in its state.
    struct Conversation;

    impl View for Conversation {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let messages = cx.use_state(Vec::<ChatMessage>::new);
            let answers = messages.clone();
            let chat = Chat::new(messages.binding())
                .placeholder("Ask something")
                .intro("It does not know who answers: yours adds to the messages, now or later.")
                .examples(["How do I start?", "Where are my files?"])
                .on_send(move |text| {
                    let reply = ChatMessage::assistant(format!("You asked: {text}"))
                        .card(|| Box::new(Button::new("Open it").size(ButtonSize::Small)));
                    answers.update(|all| all.push(reply));
                });
            cx.child(&VStack::new((chat,)).height(380.0).width(420.0))
        }
    }
    Conversation
}
// end

// sample: FloatingPanel, 80
fn floating_panel() -> impl View {
    /// Keeps whether it is open, here while the page rebuilds.
    struct Corner;

    impl View for Corner {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let open = cx.use_state(|| false);
            // `.movable` and `.minimized` take bindings, for the app to remember them.
            cx.child(&FloatingPanel::new(open.binding(), "Assistant", || {
                Box::new(Text::new("The app stays usable behind this panel."))
            }))
        }
    }
    Corner
}
// end

// sample: ModelPicker, 360
fn model_picker() -> impl View {
    let choice = |name: &str, status| ModelChoice {
        name: name.into(),
        details: "1.1 GB · License: Apache 2.0".into(),
        status,
        in_use: false,
        result: None,
        error: None,
    };
    // It shows what it is given. `ravenrs_ai::ModelManager` does the work on the desktop.
    ModelPicker::new(vec![
        choice("To download", ModelStatus::Available),
        choice("Arriving", ModelStatus::Downloading(0.4)),
        ModelChoice {
            result: Some("About 20 words per second.".into()),
            ..choice("On this machine", ModelStatus::Installed)
        },
    ])
    .on_test(|_| {})
    .on_use(|_| {})
}
// end

// sample: ToolRail, 280
fn tool_rail() -> impl View {
    let open = State::new(0);
    ToolRail::new(
        open.binding(),
        [(IconName::Type, "Text"), (IconName::ZoomIn, "Zoom"), (IconName::Layers, "Fades")],
    )
    // Hold the button, right-click or press Right for the tools that share its place.
    .separator()
    .tool(ToolRailItem::new(IconName::Square, "Shape").variant(IconName::Circle, "Ellipse"))
}
// end

// sample: Timeline, 190
fn timeline() -> impl View {
    Timeline::new(vec![
        TimelineTrack::new(
            "Notes",
            34.0,
            vec![TimelineItem::new(1, 2_000, 3_000, "Callout").selected(true).movable()],
        ),
        TimelineTrack::new(
            "Screen",
            34.0,
            vec![TimelineItem::new(2, 6_000, 4_000, "Zoom").tone(Tone::Success)],
        ),
        TimelineTrack::new(
            "Film",
            44.0,
            vec![TimelineItem::new(3, 0, 14_000, "Take 1").tone(Tone::Accent).trimmable()],
        ),
    ])
    .markers(vec![TimelineMarker { at_ms: 6_000, title: "Chapter two".to_owned() }])
    .position_ms(4_000)
}
// end

// sample: CodeBlock, 70
fn code_block() -> impl View {
    CodeBlock::new("cargo add ravenrs")
}
// end

// sample: ReleaseNotes, 480
fn release_notes() -> impl View {
    let selected = State::new(0);
    ReleaseNotes::new(
        vec![
            ReleaseNote::new("Version 27.2", "Status bars and release notes")
                .date("Nov 1, 2026")
                .badge("New", Tone::Success)
                .changes(["A line at the foot of the window", "What each version brought"])
                .section("Why update", "Tell people about new versions without getting in their way."),
            ReleaseNote::new("Version 27.1", "The first of the year").badge("Installed", Tone::Neutral),
        ],
        selected.binding(),
    )
}
// end

// sample: AspectRatio, 190
fn aspect_ratio() -> impl View {
    VStack::new((AspectRatio::new(16.0 / 9.0, Image::new(picture(1.0)).stretch()),))
        .width(320.0)
        .align(Align::Stretch)
}
// end

// sample: Collapse, 170
fn collapse() -> impl View {
    let open = State::new(true);
    let flip = open.clone();
    view_fn(move || {
        let toggle = flip.clone();
        VStack::new((
            Button::new("Toggle").style(ButtonStyle::Secondary).on_click(move || toggle.update(|o| *o = !*o)),
            Collapse::new(open.get(), Text::new("The height animates, whatever the content.")),
        ))
        .spacing(12.0)
    })
}
// end

// sample: Text, 170
fn text() -> impl View {
    VStack::new((
        Text::new("Display").style(TextStyle::Display),
        Text::new("Headline").style(TextStyle::Headline),
        Text::new("Muted body text").foreground(Semantic::InkMuted),
    ))
    .spacing(6.0)
}
// end

// sample: Alert, 260
fn alert() -> impl View {
    VStack::new((
        Alert::new("Deployment finished", "The site is live.").tone(Tone::Success),
        Alert::new("Payment failed", "Update your card.").tone(Tone::Danger),
        Alert::message("Moving deletes the originals.").tone(Tone::Warning).compact(),
    ))
    .spacing(10.0)
    .align(Align::Stretch)
}
// end

// sample: Badge, 80
fn badge() -> impl View {
    HStack::new((
        Badge::new("Neutral"),
        Badge::new("New").tone(Tone::Accent),
        Badge::new("Paid").tone(Tone::Success).icon(IconName::Check),
        Badge::new("Overdue").tone(Tone::Danger),
        Badge::new("Draft").outline(),
        Badge::new("Family").color(Color::hex(0x8b5cf6)).height(26.0),
    ))
    .spacing(8.0)
    .wrap()
}
// end

// sample: Avatar, 90
fn avatar() -> impl View {
    HStack::new((
        Avatar::new("Ana Lima"),
        Avatar::new("Bruno Reis").border(2.0, Semantic::Accent),
        Avatar::new("Carla Dias").size(44.0).image(Image::new(picture(1.0))),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
// end

// sample: Kbd, 80
fn kbd() -> impl View {
    HStack::new((
        Kbd::new("⌘"),
        Kbd::new("Shift"),
        Kbd::new("K"),
        // The button gives the key its own color.
        Button::new("Open").size(ButtonSize::Small).shortcut("F8"),
    ))
    .spacing(6.0)
    .align(Align::Center)
}
// end

// sample: ProgressBar, 170
fn progress_bar() -> impl View {
    VStack::new((
        ProgressBar::new(0.25),
        ProgressBar::new(0.9).tone(Tone::Success),
        ProgressBar::new(0.4).label("Copying").percent().detail("12 of 30 photos").height(4.0),
    ))
    .spacing(14.0)
    .align(Align::Stretch)
}
// end

// sample: Spinner, 80
fn spinner() -> impl View {
    Spinner::new()
}
// end

// sample: Lottie, 140
/// A Lottie file: a ring that draws itself, on and on. Any file from LottieFiles goes here.
const LOADING_RING: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":0,"k":[70,70]}},
{"ty":"tm","s":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":60,"s":[100]}]},"e":{"a":1,"k":[{"t":0,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[100]}]},"o":{"a":0,"k":0},"m":1},
{"ty":"st","c":{"a":0,"k":[0.25,0.34,0.86,1]},"o":{"a":0,"k":100},"w":{"a":0,"k":9},"lc":2,"lj":2},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

fn lottie() -> impl View {
    match LottieAnimation::from_json(LOADING_RING.as_bytes()) {
        Ok(animation) => Some(Lottie::new(animation).size(96.0, 96.0).alt("A ring that draws itself")),
        Err(_) => None,
    }
}
// end

// sample: Skeleton, 120
fn skeleton() -> impl View {
    HStack::new((
        Skeleton::circle(44.0),
        VStack::new((Skeleton::line(14.0), Skeleton::block(120.0, 12.0)))
            .spacing(8.0)
            .align(Align::Stretch)
            .grow(1.0),
    ))
    .spacing(14.0)
}
// end

// sample: Dot, 60
fn dot() -> impl View {
    HStack::new((
        Dot::new().tone(Tone::Warning).label("Not saved"),
        Text::new("Not saved").style(TextStyle::Callout),
        Dot::new().tone(Tone::Success).size(10.0).label("Online"),
        Text::new("Online").style(TextStyle::Callout),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
// end

// sample: FloatingToolbar, 230
fn floating_toolbar() -> impl View {
    let (place, level) = (State::new(None::<Point>), State::new(100.0_f32));
    FloatingToolbar::new(
        Spacer::new().frame(None, Some(200.0)).background(Semantic::Control).corner_radius(8.0),
    )
    .label("Zoom")
    .movable(place.binding())
    .action(ToolbarAction::new("Fit"))
    .action(ToolbarAction::new("Fill"))
    .separator()
    .action(ToolbarAction::new("Zoom out").icon(IconName::Minus).icon_only())
    .view(ValueSlider::new(level.binding()).range(10.0, 400.0).step(10.0).unit("%").label("Zoom level"))
    .action(ToolbarAction::new("Zoom in").icon(IconName::Plus).icon_only())
}
// end

// sample: TitleBar, 90
fn title_bar() -> impl View {
    let maximized = State::new(false);
    view_fn(move || {
        let toggle = maximized.clone();
        TitleBar::new("Report.pdf")
            .leading(Icon::new(IconName::File).size(16.0))
            .action(IconButton::new(IconName::Search).label("Search").size(ButtonSize::Mini))
            .maximized(maximized.get())
            .on_minimize(|| {})
            .on_maximize(move || toggle.update(|on| *on = !*on))
            .on_close(|| {})
    })
}
// end

// sample: ScrollEdgeArrow, 110
fn scroll_edge_arrow() -> impl View {
    let photos: Vec<AnyView> = (1..=24)
        .map(|n| -> AnyView {
            Box::new(
                Text::new(format!("Photo {n}"))
                    .style(TextStyle::Callout)
                    .centered()
                    .frame(Some(120.0), Some(72.0))
                    .background(Semantic::Control)
                    .corner_radius(6.0),
            )
        })
        .collect();
    ScrollEdgeArrow::new(photos).fade(Semantic::SurfaceElevated)
}
// end

// sample: Histogram, 130
fn histogram() -> impl View {
    let hill = |center: f32| -> Vec<f32> {
        (0..256).map(|i| 400.0 * (-((i as f32 - center) / 40.0).powi(2)).exp() + 10.0).collect()
    };
    Histogram::new([
        HistogramChannel::new("Red", Color::rgb(0.9, 0.15, 0.15), hill(90.0)),
        HistogramChannel::new("Green", Color::rgb(0.15, 0.8, 0.2), hill(128.0)),
        HistogramChannel::new("Blue", Color::rgb(0.2, 0.3, 0.95), hill(170.0)),
    ])
    .zones(5)
    .height(110.0)
}
// end

// sample: Ruler, 70
fn ruler() -> impl View {
    VStack::new((
        Ruler::horizontal().scale(2.0).origin(-20.0).marker(60.0),
        Ruler::horizontal().scale(0.2).whole_units(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
// end

// sample: ColorWheel, 180
fn color_wheel() -> impl View {
    let tint = State::new(HueSaturation::new(30.0, 0.4));
    let shown = tint.clone();
    HStack::new((
        ColorWheel::new(tint.binding()).diameter(160.0).label("Shadows"),
        Text::bound(&shown, |v| format!("{:.0}°, {:.0}%", v.hue, v.saturation * 100.0))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(16.0)
}
// end

// sample: DropZone, 140
fn drop_zone() -> impl View {
    let count = State::new(0_usize);
    let (shown, add) = (count.clone(), count);
    DropZone::new(
        Text::bound(&shown, |n| format!("{n} files received. Drop more, or press Enter here."))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    )
    .label("Files of the session")
    .min_height(96.0)
    .choosable()
    .on_files(move |files| add.update(|n| *n += files.len()))
}
// end

// sample: ValueSlider, 70
fn value_slider() -> impl View {
    let (opacity, size) = (State::new(80.0_f32), State::new(24.0_f32));
    HStack::new((
        ValueSlider::new(opacity.binding())
            .range(0.0, 100.0)
            .step(1.0)
            .unit("%")
            .label("Opacity")
            .show_label(),
        ValueSlider::new(size.binding()).range(1.0, 200.0).step(1.0).unit(" px").label("Size").field(),
    ))
    .spacing(24.0)
    .align(Align::Center)
}
// end

// sample: InlineRename, 80
fn inline_rename() -> impl View {
    let name = State::new(String::from("Warm evening"));
    VStack::new((
        InlineRename::new(name.binding()).label("Preset name").width(220.0),
        Text::new("Double click it, or press F2 on it.")
            .style(TextStyle::Caption)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
}
// end

// sample: OverlayBadge, 90
fn overlay_badge() -> impl View {
    HStack::new((
        OverlayBadge::new(Avatar::new("Ana Lima"), Dot::new().tone(Tone::Success).size(10.0).label("Online"))
            .corner(FloatingCorner::BottomRight)
            .inset(5.0),
        OverlayBadge::new(
            Button::new("Inbox").style(ButtonStyle::Secondary),
            Badge::new("3").tone(Tone::Danger),
        ),
    ))
    .spacing(32.0)
    .align(Align::Center)
}
// end

// sample: Rating, 100
fn rating() -> impl View {
    let score = State::new(3u32);
    VStack::new((
        Rating::new(score.binding()).label("Photo rating").clearable(),
        Rating::read_only(4).size(14.0),
    ))
    .spacing(10.0)
}
// end

// sample: IconTile, 80
fn icon_tile() -> impl View {
    HStack::new((
        IconTile::new(IconName::CircleCheck).tone(Tone::Success),
        IconTile::new(IconName::TriangleAlert).tone(Tone::Warning),
        IconTile::new(IconName::Image).size(32.0),
    ))
    .spacing(12.0)
    .align(Align::Center)
}
// end

// sample: EmptyState, 260
fn empty_state() -> impl View {
    EmptyState::new("No photos yet")
        .icon(IconName::Image)
        .description("Import a folder to start. The originals stay where they are.")
        .action(Button::new("Import"))
        .action(Button::new("Learn more").style(ButtonStyle::Ghost))
}
// end

// sample: HeroBanner, 170
fn hero_banner() -> impl View {
    VStack::new((
        HeroBanner::new("You can remove the card").icon(IconName::CircleCheck).tone(Tone::Success),
        HeroBanner::new("Do not remove the card").icon(IconName::CircleAlert).tone(Tone::Danger),
    ))
    .spacing(10.0)
    .align(Align::Stretch)
}
// end

// sample: MultiProgress, 170
fn multi_progress() -> impl View {
    MultiProgress::new().label("Import").meter("Local copy", 84, 120).meter("Previews", 12, 84)
}
// end

// sample: ShimmerText, 60
fn shimmer_text() -> impl View {
    ShimmerText::new("Looking for updates")
}
// end

// sample: DaySeparator, 110
fn day_separator() -> impl View {
    let today = Date::new(2026, 11, 4).unwrap_or_else(|| Date::from_days(0));
    VStack::new((
        DaySeparator::of(today, today),
        DaySeparator::of(today.add_days(-1), today),
        DaySeparator::new("Last week"),
    ))
    .spacing(14.0)
    .align(Align::Stretch)
}
// end

// sample: ShortcutSheet, 110
fn shortcut_sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Keyboard shortcuts")
            .style(ButtonStyle::Secondary)
            .icon(IconName::Keyboard)
            .on_click(move || show.set(true)),
        ShortcutSheet::new(open.binding())
            .group("Navigate", [("←", "Previous photo"), ("→", "Next photo")])
            .group("Rate", [("1–5", "Give a score"), ("0", "Take it away")]),
    ))
}
// end

// sample: ShortcutHintBar, 60
fn shortcut_hint_bar() -> impl View {
    ShortcutHintBar::new()
        .leading("3 of 40")
        .hint(["←", "→"], "move")
        .hint(["1", "5"], "rate")
        .hint(["P"], "pick")
}
// end

// sample: ZStack, 110
fn z_stack() -> impl View {
    ZStack::new((
        Canvas::new(|frame, scene| {
            scene.push_quad(Quad::new(frame, Color::hex(0x4c66ee)).radius(8.0));
        })
        .size(220.0, 80.0),
        Badge::new("On top").tone(Tone::Success),
    ))
    .align(Align::Center)
}
// end

// sample: PropertyRow, 130
fn property_row() -> impl View {
    VStack::new((
        PropertyRow::new("Camera", "NIKON D3100"),
        PropertyRow::new("Lens", ""),
        PropertyRow::with("Status", Badge::new("Edited").tone(Tone::Success)),
    ))
    .spacing(6.0)
    .align(Align::Stretch)
}
// end

// sample: PageHeader, 130
fn page_header() -> impl View {
    PageHeader::new("Sessions")
        .description("Every shoot, newest first.")
        .badge(Badge::new("12"))
        .action(Button::new("New session").icon(IconName::Plus))
}
// end

// sample: ButtonGroup, 80
fn button_group() -> impl View {
    let period = State::new(0);
    view_fn(move || {
        let chosen = period.get();
        let item = |index, label: &str| {
            let period = period.clone();
            let style = if chosen == index { ButtonStyle::Primary } else { ButtonStyle::Secondary };
            Button::new(label).style(style).on_click(move || period.set(index))
        };
        ButtonGroup::new([item(0, "Day"), item(1, "Week"), item(2, "Month")]).label("Period")
    })
}
// end

// sample: FavoriteToggle, 70
fn favorite_toggle() -> impl View {
    let (heart, star) = (State::new(true), State::new(false));
    HStack::new((
        FavoriteToggle::new(heart.binding()).label("Warm portrait"),
        FavoriteToggle::new(star.binding()).label("Pinned").star(),
    ))
    .spacing(8.0)
}
// end

// sample: SwatchGrid, 160
fn swatch_grid() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::hex(0x2563eb)));
    let (for_front, for_back, shown) = (front.clone(), back.clone(), front.clone());
    VStack::new((
        view_fn(move || {
            let (for_front, for_back) = (for_front.clone(), for_back.clone());
            SwatchGrid::new([
                (Color::hex(0xe11d48), "Rose"),
                (Color::hex(0xf59e0b), "Amber"),
                (Color::hex(0x16a34a), "Green"),
                (Color::hex(0x2563eb), "Blue"),
                (Color::hex(0x7c3aed), "Violet"),
                (Color::hex(0x111827), "Ink"),
            ])
            .columns(3)
            .selected(Some(shown.get()))
            .on_pick(move |color, held| {
                // With Cmd or Ctrl, the color goes to the background.
                if held.meta || held.ctrl { for_back.set(color) } else { for_front.set(color) }
            })
        }),
        Text::new("Click: foreground. Cmd or Ctrl and click: background.").style(TextStyle::Caption),
    ))
    .spacing(6.0)
}
// end

// sample: NinePointAnchor, 70
fn nine_point_anchor() -> impl View {
    let point = State::new(AnchorPoint::Center);
    NinePointAnchor::new(point.binding()).cell_size(12.0)
}
// end

// sample: ColorPair, 90
fn color_pair() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::WHITE));
    ColorPair::new(front.binding(), back.binding()).defaults(Color::BLACK, Color::WHITE)
}
// end

// sample: Icon, 80
fn icon() -> impl View {
    HStack::new((
        Icon::new(IconName::Sparkles).size(24.0).color(Semantic::Accent),
        Icon::new(IconName::Bell).size(24.0),
        Icon::new(IconName::Check).size(24.0).color(Semantic::Success),
    ))
    .spacing(12.0)
}
// end

// sample: Image, 150
fn image() -> impl View {
    HStack::new((
        // `.crossfade` dissolves into the next image the app hands it.
        Image::new(picture(1.0)).size(160.0, 90.0).corner_radius(12.0).crossfade(0.2),
        Image::new(picture(0.3)).size(90.0, 90.0).corner_radius(45.0),
    ))
    .spacing(12.0)
}
// end

// sample: Carousel, 280
fn carousel() -> impl View {
    let slides: Vec<AnyView> = [1.0, 0.6, 0.2]
        .into_iter()
        .map(|hue| Box::new(Image::new(picture(hue)).stretch().height(220.0)) as AnyView)
        .collect();
    Carousel::new(slides).height(220.0)
}
// end

// sample: Canvas, 170
fn canvas() -> impl View {
    Canvas::new(|frame, scene| {
        for i in 0..8 {
            let x = frame.x() + i as f32 * 28.0;
            let height = 20.0 + i as f32 * 12.0;
            let bar = Rect::new(x, frame.max_y() - height, 22.0, height);
            scene.push_quad(Quad::new(bar, Color::rgb(0.3, 0.4, 0.93)).radius(4.0));
        }
    })
    .size(240.0, 130.0)
}
// end

/// Shows one example alone, for the frame on a component's page.
pub struct SampleApp {
    content: AnyView,
}

impl std::fmt::Debug for SampleApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleApp").finish_non_exhaustive()
    }
}

impl SampleApp {
    /// The example of a component, if it has one.
    pub fn new(name: &str) -> Option<Self> {
        sample(name).map(|content| Self { content })
    }
}

impl View for SampleApp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = cx.child(self.content.as_ref());
        cx.container(
            LayoutStyle {
                grow: 1.0,
                padding: Insets::uniform(24.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        )
    }
}

/// Components without an example would make the site's list a promise it does not keep.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_every_example_builds() {
        let unique: std::collections::BTreeSet<&&str> = NAMES.iter().collect();
        assert_eq!(unique.len(), NAMES.len());
        for name in NAMES {
            let app = SampleApp::new(name).unwrap_or_else(|| panic!("{name}"));
            let mut ui = Ui::new(app, Theme::raven_dark());
            ui.set_viewport(Size::new(800.0, 500.0), 1.0);
            ui.update();
        }
        assert!(SampleApp::new("Nope").is_none());
    }
}
