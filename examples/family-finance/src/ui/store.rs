//! What every screen shares: the database, a change counter, and small helpers.

use super::help::{Help, tip};
use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use ravenrs::layout::{Align, Axis, LayoutStyle};
use ravenrs::native::SaveDialog;
use ravenrs::prelude::*;
use ravenrs::widgets::Paint;
use ravenrs_report::ReportEngine;

use crate::db::{Database, DbResult};
use crate::model::{date_label, month_label, parse_date};
use crate::update::Updater;

/// The database and what the screens need around it. Every successful write
/// raises `revision`, and everything read from the database is keyed by it, so
/// each screen refreshes itself without being told to.
#[derive(Clone)]
pub(crate) struct Store {
    pub db: Rc<Database>,
    pub today: Date,
    pub toasts: Toasts,
    pub engine: Rc<RefCell<ReportEngine>>,
    pub path: Option<PathBuf>,
    pub updater: Updater,
    revision: State<u64>,
    /// The window's diagnostics panel, for the button in Ajustes.
    pub diagnostics: Diagnostics,
    /// The records of the screens generated from them, made once.
    pub records: Rc<OnceCell<Option<ravenrs_app_sqlite::SqliteDatabase>>>,
}

impl Store {
    pub fn new(db: Database, today: Date, path: Option<PathBuf>, updater: Updater) -> Self {
        Self {
            db: Rc::new(db),
            today,
            toasts: Toasts::new(),
            engine: Rc::new(RefCell::new(ReportEngine::new())),
            path,
            updater,
            revision: State::new(0),
            diagnostics: Diagnostics::default(),
            records: Rc::default(),
        }
    }

    /// Read while building, so the caller rebuilds after any change.
    pub fn version(&self) -> u64 {
        self.revision.get()
    }

    /// Runs a write. Success refreshes the screens and says so; failure says what to do.
    pub fn write<T>(&self, success: &str, work: impl FnOnce(&Database) -> DbResult<T>) -> Option<T> {
        match work(&self.db) {
            Ok(value) => {
                self.revision.update(|r| *r += 1);
                if !success.is_empty() {
                    self.toasts.show(success, "", Tone::Success);
                }
                Some(value)
            }
            Err(e) => {
                self.toasts.show_for("Não foi possível salvar", e.to_string(), Tone::Danger, 8.0);
                None
            }
        }
    }

    /// Rebuilds the screens after a change outside the database, such as a backup.
    pub fn refresh(&self) {
        self.revision.update(|r| *r += 1);
    }

    /// Asks where to save `name` and writes `bytes` there. Saying where is the
    /// user's choice, so cancelling is not a failure and says nothing.
    pub fn save_file(&self, name: &str, kind: (&str, &[&str]), bytes: &[u8]) -> Option<PathBuf> {
        let path = SaveDialog::new(name).filter(kind.0, kind.1).ask()?;
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.toasts.show_for("Arquivo salvo", path.display().to_string(), Tone::Success, 8.0);
                Some(path)
            }
            Err(e) => {
                self.toasts.show_for("Não foi possível salvar o arquivo", e.to_string(), Tone::Danger, 8.0);
                None
            }
        }
    }

    /// Runs a read. A failure is logged and shows as empty: this runs while
    /// building, where nothing can be shown yet.
    pub fn read<T: Default>(&self, work: impl FnOnce(&Database) -> DbResult<T>) -> T {
        work(&self.db).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "could not read the database");
            T::default()
        })
    }
}

/// Keeps the last computed value until its key changes. Views build on every
/// hover and focus change, and none of that should reach the database.
pub(crate) struct Memo<K, V>(Rc<RefCell<Option<(K, V)>>>);

impl<K, V> Clone for Memo<K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K: PartialEq, V: Clone> Memo<K, V> {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    pub fn get(&self, key: K, compute: impl FnOnce() -> V) -> V {
        if let Some((held, value)) = self.0.borrow().as_ref()
            && *held == key
        {
            return value.clone();
        }
        let value = compute();
        *self.0.borrow_mut() = Some((key, value.clone()));
        value
    }
}

/// A column that takes the space left and keeps it, so a grid or a viewer
/// inside fills the window instead of pushing past it.
pub(crate) struct FillColumn<C> {
    pub children: C,
    pub spacing: f32,
    pub padding: Insets,
}

impl<C: ViewSeq> View for FillColumn<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.children);
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                spacing: self.spacing,
                padding: self.padding,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// A screen's margins. The bottom one leaves the assistant's sphere clear of what is on
/// the screen, so that it never covers a button.
pub(crate) const PAGE_INSETS: Insets = Insets { top: 24.0, right: 24.0, bottom: 72.0, left: 24.0 };

/// The same, with a screen's usual space.
pub(crate) fn screen<C: ViewSeq>(children: C) -> FillColumn<C> {
    FillColumn { children, spacing: 16.0, padding: PAGE_INSETS }
}

/// The same, for a screen whose content goes on past the bottom of the window.
pub(crate) fn screen_to_the_edge<C: ViewSeq>(children: C) -> FillColumn<C> {
    FillColumn { children, spacing: 16.0, padding: Insets { bottom: 0.0, ..PAGE_INSETS } }
}

/// The scenarios of a grid, kept in a file beside the database.
pub(crate) fn remembered_scenarios(path: Option<&std::path::Path>, grid: &str) -> Binding<Vec<GridScenario>> {
    let file = path.map(|path| path.with_extension(format!("cenarios-{grid}.json")));
    let kept = file
        .as_ref()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| GridScenario::list_from_json(&text).ok())
        .unwrap_or_default();
    let list = State::new(kept);
    let (read, write) = (list.clone(), list);
    Binding::new(
        move || read.get(),
        move |value: Vec<GridScenario>| {
            if let Some(file) = &file
                && let Err(e) = std::fs::write(file, GridScenario::list_to_json(&value))
            {
                tracing::warn!(error = %e, "could not keep the scenarios");
            }
            write.set(value);
        },
    )
}

/// Below this window width the sidebar leaves the page and opens as a sheet.
pub(crate) const NARROW_WINDOW: f32 = 900.0;
const SIDEBAR_WIDTH: f32 = 248.0;

/// The width of a page's content in a window this big, without its margins.
pub(crate) fn content_width(window: Size) -> f32 {
    let sidebar = if window.width < NARROW_WINDOW { 0.0 } else { SIDEBAR_WIDTH };
    window.width - sidebar - PAGE_INSETS.left - PAGE_INSETS.right
}

/// A grid that keeps the room left, inside a key that gives it its columns again.
pub(crate) fn filling<G: View + 'static>(grid: G) -> FillColumn<(G,)> {
    FillColumn { children: (grid,), spacing: 0.0, padding: 0.0.into() }
}

/// A day as a record holds it.
pub(crate) fn field_of_date(date: Date) -> ravenrs_app::Value {
    ravenrs_app::Value::Date(ravenrs_criteria::DateTime { days: date.to_days(), seconds: 0 })
}

/// A day from what a record holds.
pub(crate) fn date_of_field(value: &ravenrs_app::Value) -> Option<Date> {
    match value {
        ravenrs_app::Value::Date(moment) => Some(Date::from_days(moment.days)),
        _ => None,
    }
}

/// A key from what a record holds.
pub(crate) fn id_of_field(value: &ravenrs_app::Value) -> Option<i64> {
    match value {
        ravenrs_app::Value::Number(number) => Some(*number as i64),
        _ => None,
    }
}

/// Reads again what the framework's lists show, when the database changed under them.
pub(crate) fn follow_store(store: &Store, session: &ravenrs_app_ui::Session, seen: &std::cell::Cell<u64>) {
    let now = store.version();
    if seen.replace(now) != now {
        session.records_changed();
    }
}

/// A screen: what it says about itself in the top bar, and what it shows.
pub(crate) struct Page {
    pub subtitle: String,
    pub help: Help,
    pub body: AnyView,
}

impl Page {
    pub fn new(help: Help, subtitle: impl Into<String>, body: impl View + 'static) -> Self {
        Self { subtitle: subtitle.into(), help, body: Box::new(body) }
    }
}

/// A title with its "?" beside it.
pub(crate) fn titled(title: &str, help: Help) -> impl View + use<> {
    HStack::new((Text::new(title.to_owned()).style(TextStyle::Headline), tip(help)))
        .spacing(4.0)
        .align(Align::Center)
}

/// A card with its title inside.
pub(crate) fn titled_card<V: View + 'static>(title: &str, help: Help, content: V) -> impl View + use<V> {
    Card::new(VStack::new((titled(title, help), content)).spacing(16.0).align(Align::Stretch))
}

/// Moves between months.
pub(crate) fn month_nav(month: &State<Date>, today: Date) -> impl View + use<> {
    let (back, ahead, now) = (month.clone(), month.clone(), month.clone());
    HStack::new((
        Tooltip::new(
            IconButton::new(IconName::ChevronLeft)
                .label("Mês anterior")
                .on_click(move || back.update(|m| *m = m.add_months(-1))),
            "Mês anterior",
        ),
        Text::new(month_label(month.get())).style(TextStyle::Headline).width(170.0).centered(),
        Tooltip::new(
            IconButton::new(IconName::ChevronRight)
                .label("Próximo mês")
                .on_click(move || ahead.update(|m| *m = m.add_months(1))),
            "Próximo mês",
        ),
        Button::new("Ir para este mês")
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || now.set(today.first_of_month())),
    ))
    .spacing(4.0)
    .align(Align::Center)
}

/// A column of amounts in reais.
pub(crate) fn money_column(title: &str) -> GridColumn {
    GridColumn::number(title)
        .number_format(NumberFormat {
            decimals: 2,
            prefix: "R$ ".to_owned(),
            suffix: String::new(),
            thousands: '.',
            decimal: ',',
        })
        .width(130.0)
}

/// A column of ISO dates shown the Brazilian way; the ISO text keeps them sorting by date.
pub(crate) fn date_column(title: &str) -> GridColumn {
    GridColumn::text(title)
        .format(|value| match value {
            Value::Text(text) => parse_date(text).map_or_else(String::new, date_label),
            _ => String::new(),
        })
        .width(104.0)
}
