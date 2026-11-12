//! Moving around an application.

use ravenrs::core::Key;
use ravenrs::prelude::*;

samples! {
    "AppShell" => app_shell, "Ribbon" => ribbon, "Menubar" => menubar, "Tabs" => tabs, "Sidebar" => sidebar,
    "Breadcrumb" => breadcrumb, "Pagination" => pagination, "Accordion" => accordion,
    "Disclosure" => disclosure, "SidebarBrand" => sidebar_brand, "Link" => link, "Footer" => footer,
    "StatusBar" => status_bar, "ToolRail" => tool_rail, "ShortcutSheet" => shortcut_sheet,
    "ShortcutHintBar" => shortcut_hint_bar, "TitleBar" => title_bar, "PageHeader" => page_header,
    "DocumentTabs" => document_tabs, "KeyBar" => key_bar, "ScanField" => scan_field,
}

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
                .spacing(12.0)
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

// sample: SidebarBrand, 90
fn sidebar_brand() -> impl View {
    SidebarBrand::new(Icon::new(IconName::Wallet), "Raven").subtitle("Family finances")
}
// end

// sample: Link, 120
fn link() -> impl View {
    VStack::new((
        Link::new(
            "Read the getting started guide",
            "https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md",
        )
        .icon(IconName::ArrowRight),
        Link::new("Components", "https://alexkads.github.io/RavenRS/components/").muted(),
    ))
    .spacing(4.0)
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

// sample: DocumentTabs, 70
fn document_tabs() -> impl View {
    let tabs = State::new(vec![
        DocumentTab::new(1, "Wedding").icon(IconName::Camera),
        // ravenrs-lint: allow(raw_color) the tab keeps the color its owner picked for that document
        DocumentTab::new(2, "Portraits").unsaved(true).color(Color::rgb(0.25, 0.55, 0.95)),
        DocumentTab::new(3, "Studio with a long name that does not fit"),
        DocumentTab::new(4, "Home").closable(false),
    ]);
    let active = State::new(1_u64);
    view_fn(move || {
        let (closed, moved, renamed) = (tabs.clone(), tabs.clone(), tabs.clone());
        DocumentTabs::new(tabs.get(), active.binding())
            .on_close(move |id| closed.update(|list| list.retain(|tab| tab.id != id)))
            .on_reorder(move |from, to| {
                moved.update(|list| {
                    let tab = list.remove(from);
                    list.insert(to, tab);
                });
            })
            .on_rename(move |id, name| {
                renamed.update(|list| {
                    if let Some(tab) = list.iter_mut().find(|tab| tab.id == id) {
                        tab.title = name;
                    }
                });
            })
            .on_new(|| {})
            .fade(Semantic::SurfaceElevated)
    })
}
// end

// sample: ShortcutSheet, 110
fn shortcut_sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Show shortcuts")
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

// sample: PageHeader, 130
fn page_header() -> impl View {
    PageHeader::new("Sessions")
        .description("Every shoot, newest first.")
        .badge(Badge::new("12"))
        .action(Button::new("New session").icon(IconName::Plus))
}
// end

// sample: KeyBar, 80
fn key_bar() -> impl View {
    let paid = State::new(0);
    let on_key = paid.clone();
    VStack::new((
        KeyBar::new()
            .item(KeyBarItem::new(Hotkey::key(Key::Function(1)), "Help", || {}))
            .item(KeyBarItem::new(Hotkey::key(Key::Function(6)), "Remove item", || {}).disabled(true))
            .item(
                KeyBarItem::new(Hotkey::key(Key::Function(9)), "Pay", move || on_key.update(|n| *n += 1))
                    .primary(),
            ),
        Text::bound(&paid, |n| format!("Paid {n} times")),
    ))
    .spacing(8.0)
}
// end

// sample: ScanField, 100
fn scan_field() -> impl View {
    let last = State::new(String::new());
    let on_scan = last.clone();
    VStack::new((
        ScanField::new(move |scan: Scan| {
            let source = if scan.from_reader { "a scanner" } else { "the keyboard" };
            on_scan.set(format!("{} from {source}", scan.text));
        })
        .placeholder("Scan or type a code and press Enter"),
        Text::bound(&last, Clone::clone).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
}
// end
