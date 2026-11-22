//! RavenComponentBook: the stories of every RavenRS component, as Storybook keeps them, and
//! everything a design tool needs to design screens that RavenRS builds as they were drawn.
//!
//! Each component gives its stories through `ravenrs::widgets::book::ComponentBook`: its place
//! in the book, its settings (`argTypes`) and its named states. The book draws them live, lets
//! the settings be changed, and measures what RavenRS paints (the parts, sizes, corners,
//! fills, borders, fonts, icons and inner spaces of each piece, by token). From that it writes
//! the JSON of every component and the files of the design system the design tool reads: a
//! guide that says to compose only these components, a card with the settings and the anatomy
//! of each, and a vector preview drawn from the measurements. Components that give no stories
//! yet are shown by their example in the gallery and marked as pending.

pub mod export;
pub mod index;
pub mod measure;
pub mod patterns;
pub mod replay;
pub mod tokens;
pub mod versao;

/// The scenarios of `tests/e2e/`, which the book gives Raven Test Studio when it serves them.
pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs::widgets::book::{ArgValue, Args, Control};
use ravenrs_app_update::Updater;

pub use versao::VERSION;

/// The book's window: the components by family on the left; the story chosen, drawn live, with
/// its stories above it; its settings, its measurements and its code below.
pub struct Book {
    entries: Rc<Vec<index::Entry>>,
    component: State<usize>,
    story: State<usize>,
    args: State<Args>,
    /// The words typed in the sidebar's search, which keep only the components they name.
    filter: State<String>,
    updater: Updater,
    news: ravenrs_app_update::ui::NewsDialog,
    /// What the book's own test said when the window opened.
    health: Option<String>,
}

impl std::fmt::Debug for Book {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Book").field("entries", &self.entries.len()).finish_non_exhaustive()
    }
}

impl Book {
    fn choose(&self, component: usize, story: usize) {
        let entry = &self.entries[component];
        let args = entry.meta.stories.get(story).map(|s| entry.meta.args_of(s)).unwrap_or_default();
        self.component.set(component);
        self.story.set(story);
        self.args.set(args);
    }

    fn sidebar(&self) -> impl View {
        let chosen = self.component.get();
        let current = self.entries.get(chosen).map_or("", |e| e.meta.family());
        let query = self.filter.get().trim().to_lowercase();
        let shows = |e: &index::Entry| query.is_empty() || e.name().to_lowercase().contains(&query);
        let mut families: Vec<&str> = Vec::new();
        for e in self.entries.iter().filter(|e| shows(e)) {
            if !families.contains(&e.meta.family()) {
                families.push(e.meta.family());
            }
        }
        let groups: Vec<AnyView> = families
            .iter()
            .map(|family| {
                let items: Vec<AnyView> = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.meta.family() == *family && shows(e))
                    .map(|(i, e)| {
                        let book = self.handle();
                        let mut item = SidebarItem::text(e.name())
                            .compact()
                            .selected(i == chosen)
                            .on_click(move || book.choose(i, 0));
                        if e.pending {
                            item = item.badge(Badge::new("pendente").outline());
                        }
                        Box::new(item.test_id(format!("component:{}", e.name()))) as AnyView
                    })
                    .collect();
                // The family of the component on screen stays open; a search opens them all.
                let open = *family == current || !query.is_empty();
                Box::new(SidebarGroup::new(*family, items).collapsible().open(open)) as AnyView
            })
            .collect();
        let search = TextField::new(self.filter.binding())
            .placeholder("Buscar componente")
            .icon(IconName::Search)
            .clearable()
            .size(ButtonSize::Small)
            .test_id("search");
        Sidebar::new(groups).header(VStack::new((search,)).padding(8.0)).width(260.0)
    }

    /// A copy that shares the state, for the closures of the window.
    fn handle(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            component: self.component.clone(),
            story: self.story.clone(),
            args: self.args.clone(),
            filter: self.filter.clone(),
            updater: self.updater.clone(),
            news: self.news.clone(),
            health: self.health.clone(),
        }
    }

    fn controls(&self, entry: &index::Entry) -> AnyView {
        if entry.meta.arg_types.is_empty() {
            return Box::new(
                Text::new("Este componente ainda não tem configurações: ele mostra o exemplo da galeria até implementar ComponentBook.")
                    .foreground(Semantic::InkMuted),
            );
        }
        let rows: Vec<AnyView> = entry
            .meta
            .arg_types
            .iter()
            .map(|arg| {
                let name = arg.name;
                let shared = self.args.clone();
                let read = self.args.clone();
                let field: AnyView = match &arg.control {
                    Control::Bool => {
                        let on = Binding::new(
                            move || read.get().bool(name),
                            move |v| shared.update(|a| a.set(name, v)),
                        );
                        Box::new(Toggle::new(on).label(name))
                    }
                    Control::Choice(options) => {
                        let (names, again) = (options.clone(), options.clone());
                        let picked = Binding::new(
                            move || names.iter().position(|o| *o == read.get().choice(name)).unwrap_or(0),
                            move |i: usize| {
                                let chosen = again.get(i).copied().unwrap_or_default();
                                shared.update(|a| a.set(name, ArgValue::choice(chosen)));
                            },
                        );
                        Box::new(Select::new(picked, options.clone()).label(name).size(ButtonSize::Small))
                    }
                    Control::Number { min, max, step } => {
                        let value = Binding::new(
                            move || read.get().number(name) as f32,
                            move |v: f32| shared.update(|a| a.set(name, f64::from(v))),
                        );
                        Box::new(
                            Slider::new(value)
                                .label(name)
                                .range(*min as f32, *max as f32)
                                .step(*step as f32)
                                .show_value(),
                        )
                    }
                    _ => {
                        let text = Binding::new(
                            move || read.get().text(name),
                            move |v: String| shared.update(|a| a.set(name, v)),
                        );
                        Box::new(TextField::new(text).placeholder(name).size(ButtonSize::Small))
                    }
                };
                Box::new(
                    VStack::new((
                        field,
                        Text::new(arg.description).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                    ))
                    .spacing(4.0)
                    .test_id(format!("arg:{name}")),
                ) as AnyView
            })
            .collect();
        Box::new(Grid::new(2, rows).spacing(16.0))
    }

    fn measurements(&self, entry: &index::Entry) -> AnyView {
        let Some(story) = entry.meta.stories.get(self.story.get()) else { return Box::new(Text::new("")) };
        let theme = measure::themes()[0].1;
        let measured = measure::measure(&entry.meta, story, &self.args.get(), theme);
        let rows = export::anatomy(&measured);
        let text = |v: Option<&serde_json::Value>| -> String {
            match v {
                None | Some(serde_json::Value::Null) => "—".to_owned(),
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            }
        };
        let mut table = Table::new([
            Column::new("Parte").weight(2.0),
            Column::new("Tamanho"),
            Column::new("Raio"),
            Column::new("Fundo"),
            Column::new("Texto").weight(2.0),
            Column::new("Ícone"),
            Column::new("Recuo"),
        ]);
        for r in &rows {
            let part = match r.get("name").and_then(serde_json::Value::as_str) {
                Some(name) => format!("{} \"{name}\"", text(r.get("part"))),
                None => text(r.get("part")),
            };
            let font = r.get("text").map_or_else(
                || "—".to_owned(),
                |t| {
                    format!(
                        "{} {} {}px {}",
                        text(t.get("font")),
                        text(t.get("weight")),
                        text(t.get("size")),
                        text(t.get("color"))
                    )
                },
            );
            let icon = r.get("icon").map_or_else(
                || "—".to_owned(),
                |i| format!("{} {}px", text(i.get("name")), text(i.get("size"))),
            );
            table = table.row([
                part,
                format!("{} × {}", text(r.get("width")), text(r.get("height"))),
                text(r.get("radius")),
                text(r.get("fill")),
                font,
                icon,
                text(r.get("padding_left")),
            ]);
        }
        let said = format!(
            "{} × {} pt · {} partes · {} formas desenhadas, medidas no Raven Dark",
            measured.size.0,
            measured.size.1,
            measured.parts.len(),
            measured.paint.len()
        );
        Box::new(
            VStack::new((Text::new(said).foreground(Semantic::InkMuted), table))
                .spacing(8.0)
                .align(Align::Stretch),
        )
    }

    fn status_bar(&self) -> StatusBar {
        let version = ravenrs_app_update::ui::version_button(
            format!("RavenComponentBook · versão {VERSION}"),
            &self.updater,
            &self.news,
        );
        let pending = self.entries.iter().filter(|e| e.pending).count();
        let said = format!(
            "{} componentes · {} com histórias · {} pendentes",
            self.entries.len(),
            self.entries.len() - pending,
            pending
        );
        let bar = StatusBar::new().left((version, StatusBar::text(said).test_id("count")));
        let bar = match &self.health {
            Some(h) => bar.right((StatusBar::text(h.clone()).test_id("health"),)),
            None => bar,
        };
        ravenrs_app_update::ui::with_notice(bar, &self.updater, &self.news)
    }
}

impl View for Book {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let waker = cx.ui_waker();
        self.updater.connect(move || waker.wake());
        self.updater.poll();
        let entry = &self.entries[self.component.get().min(self.entries.len() - 1)];
        let story_names: Vec<&str> = entry.meta.stories.iter().map(|s| s.name).collect();
        let book = self.handle();
        let component = self.component.get();
        let current = self.story.clone();
        let picked = Binding::new(move || current.get(), move |i: usize| book.choose(component, i));
        let story = entry.meta.stories.get(self.story.get());
        let canvas: AnyView = match story {
            Some(story) => entry.meta.render(story, &self.args.get()),
            None => Box::new(Text::new("Sem histórias")),
        };
        let mut header: Vec<AnyView> = vec![Box::new(Text::new(entry.meta.title).style(TextStyle::Headline))];
        if entry.pending {
            header.push(Box::new(Badge::new("histórias pendentes").tone(Tone::Warning)));
        }
        let tab = State::new(0);
        let code = entry.code.clone().unwrap_or_else(|| "// Sem exemplo na galeria.".to_owned());
        let content = VStack::new((
            HStack::new(header).spacing(8.0).align(Align::Center),
            Text::new(entry.meta.summary).foreground(Semantic::InkMuted),
            SegmentedControl::new(picked, story_names).label("Histórias").test_id("stories"),
            Card::new(canvas).padding(24.0).test_id("canvas"),
            Tabs::new(tab.binding())
                .tab("Controles", self.controls(entry))
                .tab("Medidas", self.measurements(entry))
                .tab("Código", CodeBlock::new(code))
                .variant(TabsVariant::Underline),
        ))
        .spacing(16.0)
        .align(Align::Stretch)
        .padding(16.0);
        let page = VStack::new((
            HStack::new((self.sidebar(), ScrollView::new((content,)).grow(1.0)))
                .align(Align::Stretch)
                .grow(1.0),
            self.status_bar(),
            self.news.view(&self.updater),
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0);
        cx.child(&page)
    }
}

fn book(updater: Updater, health: Option<String>) -> Book {
    let entries = Rc::new(index::entries());
    let book = Book {
        entries,
        component: State::new(0),
        story: State::new(0),
        args: State::new(Args::new()),
        filter: State::new(String::new()),
        updater,
        news: ravenrs_app_update::ui::NewsDialog::new(),
        health,
    };
    book.choose(0, 0);
    book
}

/// The book with no window around it and no updates, for tests.
pub fn app() -> Localized<Book> {
    Localized::new(Labels::pt_br(), book(versao::idle(), None))
}

/// The book as its window opens: it tests itself, says so in the status bar, and looks for a
/// new version when it opens and every hour after.
pub fn installed_app() -> Localized<Book> {
    let updater = versao::idle();
    updater.watch();
    let health = match self_test() {
        Ok(said) => said,
        Err(why) => format!("Autoteste falhou: {why}"),
    };
    Localized::new(Labels::pt_br(), book(updater, Some(health)))
}

/// The book tests itself: every component is listed once, every story draws and measures in
/// the three themes with parts and paint, and in parents of every width of [`measure::WIDTHS`].
/// Returns what it checked.
pub fn self_test() -> Result<String, String> {
    let entries = index::entries();
    let mut names: Vec<&str> = entries.iter().map(index::Entry::name).collect();
    names.sort_unstable();
    if let Some(twice) = names.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("{} aparece duas vezes", twice[0]));
    }
    let mut stories = 0;
    let mut empty = Vec::new();
    for entry in entries.iter().filter(|e| !e.pending) {
        for story in &entry.meta.stories {
            let args = entry.meta.args_of(story);
            for (id, theme) in measure::themes() {
                let m = measure::measure(&entry.meta, story, &args, theme);
                if m.paint.is_empty() {
                    empty.push(format!("{} / {} ({id})", entry.name(), story.name));
                }
            }
            let dark = measure::themes()[0].1;
            for width in measure::WIDTHS {
                let m = measure::measure_at(&entry.meta, story, &args, dark, width);
                if m.paint.is_empty() {
                    empty.push(format!("{} / {} ({width} pt)", entry.name(), story.name));
                }
            }
            stories += 1;
        }
    }
    if !empty.is_empty() {
        return Err(format!("{} histórias não desenham nada: {}", empty.len(), empty.join("; ")));
    }
    let pending = entries.iter().filter(|e| e.pending).count();
    let (components, widths) = (entries.len(), measure::WIDTHS.len());
    Ok(format!(
        "Autoteste: {components} componentes, {stories} histórias medidas em {widths} larguras, {pending} pendentes"
    ))
}
