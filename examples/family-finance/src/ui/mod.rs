//! The app's window: a sidebar, a toolbar and the screens.

mod agenda;
mod assistant;
mod bills;
mod budgets;
mod dashboard;
mod entities;
mod forms;
mod help;
mod ledger;
mod opening;
mod records;
mod reports;
mod request;
mod settings;
mod spoken;
mod store;
mod training;

pub use assistant::{ASSISTANT_INTRO, ASSISTANT_TOOLS};
pub use opening::Opening;
pub use spoken::spoken;
mod updates;

use std::path::PathBuf;
use std::rc::Rc;

use ravenrs::core::Key;
use ravenrs::prelude::*;

use agenda::AgendaState;
use bills::BillsState;
use budgets::BudgetsState;
use dashboard::DashboardState;
use forms::Forms;
use ledger::LedgerState;
use ravenrs::movie::{BugReporter, FlightRecorder, RecentErrors};
use records::RecordsState;
use reports::ReportsState;
use store::{Memo, NARROW_WINDOW, Page, Store};

use crate::db::Database;
use crate::model::{BillStatus, Flow};
use crate::update::Updater;

/// The screens, in the sidebar's order.
const PAGES: [(IconName, &str); 9] = [
    (IconName::LayoutDashboard, "Painel"),
    (IconName::ArrowUpDown, "Lançamentos"),
    (IconName::CreditCard, "Contas a pagar"),
    (IconName::CalendarDays, "Agenda"),
    (IconName::Target, "Orçamentos"),
    (IconName::FileText, "Relatórios"),
    (IconName::Users, "Cadastros"),
    (IconName::Settings, "Ajustes"),
    (IconName::Video, "Treinamentos"),
];

const BILLS_PAGE: usize = 2;
/// Beside the database, this file says the menu was left reduced to icons.
const MENU_FILE: &str = "menu-recolhido";
/// How much of the last minutes a bug report can show.
const FLIGHT_SECONDS: f64 = 240.0;

/// The whole app.
pub struct App {
    store: Store,
    page: State<usize>,
    /// The screen whose parts were folded in the menu.
    menu_folded: State<Option<usize>>,
    palette: State<bool>,
    sidebar: SidebarState,
    news: updates::NewsDialog,
    forms: Forms,
    dashboard: DashboardState,
    ledger: LedgerState,
    bills: BillsState,
    agenda: AgendaState,
    budgets: BudgetsState,
    reports: ReportsState,
    records: RecordsState,
    training: training::TrainingState,
    recorder: FlightRecorder,
    reporter: BugReporter,
    overdue: Memo<u64, usize>,
    icon: Memo<u8, Option<ImageData>>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}

impl App {
    /// The panel to give the window, so that Ajustes can open it.
    pub fn diagnostics(&self) -> Diagnostics {
        self.store.diagnostics.clone()
    }

    /// An app over this database. `path` is only shown in the settings.
    pub fn new(db: Database, today: Date, path: Option<PathBuf>, updater: Updater) -> Self {
        Self::in_window(db, today, path, updater, Diagnostics::default(), RecentErrors::default())
    }

    /// Like [`Self::new`], for a window that already has its diagnostics panel, and
    /// with what keeps the errors logged, for bug reports.
    pub fn in_window(
        db: Database,
        today: Date,
        path: Option<PathBuf>,
        updater: Updater,
        diagnostics: Diagnostics,
        errors: RecentErrors,
    ) -> Self {
        let training = training::TrainingState::new(path.as_deref());
        let sidebar = Self::remembered_menu(path.as_deref());
        let (ledger, bills) = (LedgerState::new(today, path.as_deref()), BillsState::new(path.as_deref()));
        let mut store = Store::new(db, today, path, updater);
        store.diagnostics = diagnostics;
        let recorder = FlightRecorder::new(FLIGHT_SECONDS);
        let reporter = training::reporter(&store, &recorder, errors);
        Self {
            store,
            page: State::new(0),
            menu_folded: State::new(None),
            palette: State::new(false),
            sidebar,
            news: updates::NewsDialog::new(),
            training,
            recorder,
            reporter,
            forms: Forms::new(),
            dashboard: DashboardState::new(),
            ledger,
            bills,
            agenda: AgendaState::new(today),
            budgets: BudgetsState::new(today),
            reports: ReportsState::new(today),
            records: RecordsState::new(),
            overdue: Memo::new(),
            icon: Memo::new(),
        }
    }

    /// Opens a screen, by its place in the sidebar. Screenshots and tests start here.
    pub fn open_page(&self, page: usize) {
        self.page.set(page);
    }

    /// The screen that is open, by its place in the sidebar. The performance scenes
    /// move through the screens with it after the app went on screen.
    pub fn page_state(&self) -> State<usize> {
        self.page.clone()
    }

    /// The app's icon, decoded once: views build often.
    fn mark(&self) -> AnyView {
        match self.icon.get(0, crate::icon) {
            // Next to the name it says nothing a screen reader has not just read.
            Some(icon) => Box::new(Image::new(icon).size(26.0, 26.0).corner_radius(6.0).alt("")),
            None => Box::new(Icon::new(IconName::Wallet).color(Semantic::Accent)),
        }
    }

    /// Reads whether the menu was left open, and keeps it beside the database.
    fn remembered_menu(path: Option<&std::path::Path>) -> SidebarState {
        let file = path.map(|path| path.with_extension(MENU_FILE));
        let closed = file.as_ref().is_some_and(|file| file.exists());
        let open = State::new(!closed);
        let (read, write) = (open.clone(), open);
        SidebarState::controlled(Binding::new(
            move || read.get(),
            move |value: bool| {
                write.set(value);
                if let Some(file) = &file {
                    let saved = if value { std::fs::remove_file(file) } else { std::fs::write(file, b"") };
                    if let Err(e) = saved {
                        tracing::warn!(error = %e, "could not remember the menu");
                    }
                }
            },
        ))
    }

    /// A row of the menu, by its place in the list of screens.
    fn menu_row(&self, i: usize) -> SidebarItem {
        let (icon, label) = PAGES[i];
        let page = self.page.clone();
        SidebarItem::new(icon, label).compact().selected(self.page.get() == i).on_click(move || page.set(i))
    }

    /// A row with the parts of a screen, unfolded while that screen is open, unless
    /// the person folded it.
    fn menu_parts<'a>(
        &self,
        i: usize,
        selected: Option<usize>,
        parts: impl IntoIterator<Item = (&'a str, Rc<dyn Fn()>)>,
    ) -> SidebarItem {
        let current = self.page.get() == i;
        let (read, read_folded, folded) =
            (self.page.clone(), self.menu_folded.clone(), self.menu_folded.clone());
        let page = self.page.clone();
        let mut row = self.menu_row(i).expanded_binding(Binding::new(
            move || read.get() == i && read_folded.get() != Some(i),
            // Clicking the row of the screen already open folds it; any other opens that screen.
            move |open: bool| folded.set((!open && page.get() == i).then_some(i)),
        ));
        for (place, (label, open)) in parts.into_iter().enumerate() {
            let page = self.page.clone();
            row = row.item(
                SidebarItem::text(label).compact().selected(current && selected == Some(place)).on_click(
                    move || {
                        page.set(i);
                        open();
                    },
                ),
            );
        }
        row
    }

    fn sidebar(&self) -> impl View + use<> {
        let today = self.store.today;
        let overdue = self.overdue.get(self.store.version(), || {
            self.store
                .read(|db| db.open_bills())
                .iter()
                .filter(|b| b.status(today) == BillStatus::Overdue)
                .count()
        });
        let palette = self.palette.clone();
        let (collapse, open) = (self.sidebar.clone(), self.sidebar.is_open());
        let setter = |state: &State<usize>, value: usize| -> Rc<dyn Fn()> {
            let state = state.clone();
            Rc::new(move || state.set(value))
        };
        let view = |value| setter(&self.bills.view, value);
        let tab = |value| setter(&self.records.tab, value);
        let report = |value: usize| -> Rc<dyn Fn()> {
            let reports = self.reports.clone();
            Rc::new(move || reports.open(value))
        };
        let mut bills = self.menu_parts(
            BILLS_PAGE,
            Some(self.bills.view.get()),
            [("Em aberto", view(0)), ("Vencidas", view(1)), ("Pagas", view(2)), ("Todas", view(3))],
        );
        if overdue > 0 {
            bills = bills.badge(Badge::new(overdue.to_string()).tone(Tone::Danger));
        }
        let content: Vec<AnyView> = vec![
            Box::new(
                SidebarItem::new(IconName::Search, "Procurar comando")
                    .compact()
                    .shortcut("⌘K")
                    .on_click(move || palette.set(true)),
            ),
            Box::new(SidebarSection::new("Dia a dia")),
            Box::new(self.menu_row(0)),
            Box::new(self.menu_row(1)),
            Box::new(bills),
            Box::new(self.menu_row(3)),
            Box::new(self.menu_row(4)),
            Box::new(SidebarSection::new("Consultas")),
            Box::new(self.menu_parts(
                5,
                Some(self.reports.current()),
                [
                    ("Extrato do mês", report(0)),
                    ("Contas a pagar", report(1)),
                    ("Orçamento", report(2)),
                    ("Resumo de 12 meses", report(3)),
                ],
            )),
            Box::new(SidebarSection::new("Configuração")),
            Box::new(self.menu_parts(
                6,
                Some(self.records.tab.get()),
                [("Contas", tab(0)), ("Categorias", tab(1)), ("Família", tab(2))],
            )),
        ];
        Sidebar::new(content)
            .collapsible(SidebarCollapse::Icons)
            .rail(true)
            .narrow_below(NARROW_WINDOW)
            .state(self.sidebar.clone())
            .header(SidebarBrand::new(self.mark(), "Raven Finanças").subtitle("Finanças da família"))
            .footer(
                VStack::new((
                    self.menu_row(8),
                    self.menu_row(7),
                    SidebarItem::new(IconName::PanelLeft, if open { "Recolher menu" } else { "Abrir menu" })
                        .compact()
                        .shortcut("⌘B")
                        .on_click(move || collapse.toggle()),
                ))
                .spacing(2.0)
                .align(Align::Stretch),
            )
    }

    fn toolbar(&self, subtitle: String, page_help: help::Help, window: Size) -> impl View + use<> {
        let (_, title) = PAGES[self.page.get().min(PAGES.len() - 1)];
        let (expense_store, expense) = (self.store.clone(), self.forms.transaction.clone());
        let (income_store, income) = (self.store.clone(), self.forms.transaction.clone());
        let (bill_store, bill) = (self.store.clone(), self.forms.bill.clone());
        HStack::new((
            // In a small window the menu is a sheet, and this is the way to it.
            (window.width < NARROW_WINDOW).then(|| SidebarTrigger::new(self.sidebar.clone())),
            VStack::new((
                HStack::new((Text::new(title).style(TextStyle::Title), help::tip(page_help)))
                    .spacing(4.0)
                    .align(Align::Center),
                Text::new(subtitle).style(TextStyle::Callout).foreground(Semantic::InkMuted).truncate(),
            ))
            .spacing(2.0)
            .grow(1.0),
            DropdownMenu::new("Novo")
                .icon(IconName::Plus)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .item(
                    MenuItem::new("Despesa")
                        .icon(IconName::TrendingDown)
                        .shortcut("⌘N")
                        .on_select(move || expense.start_new(&expense_store, Flow::Expense)),
                )
                .item(
                    MenuItem::new("Receita")
                        .icon(IconName::TrendingUp)
                        .on_select(move || income.start_new(&income_store, Flow::Income)),
                )
                .item(
                    MenuItem::new("Conta a pagar")
                        .icon(IconName::CreditCard)
                        .on_select(move || bill.start_new(&bill_store)),
                ),
        ))
        .spacing(10.0)
        .align(Align::Center)
        .padding((12.0, 24.0))
        .background(Semantic::Surface)
    }

    fn palette_view(&self) -> impl View + use<> {
        let mut palette = CommandPalette::new(self.palette.binding())
            .placeholder("Digite um comando ou o nome de uma tela");
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            let page = self.page.clone();
            palette = palette.command(
                Command::new(format!("Ir para {label}"), move || page.set(i)).icon(*icon).group("Telas"),
            );
        }
        let (expense_store, expense) = (self.store.clone(), self.forms.transaction.clone());
        let (income_store, income) = (self.store.clone(), self.forms.transaction.clone());
        let (bill_store, bill) = (self.store.clone(), self.forms.bill.clone());
        palette
            .command(
                Command::new("Nova despesa", move || expense.start_new(&expense_store, Flow::Expense))
                    .icon(IconName::TrendingDown)
                    .shortcut("⌘N")
                    .group("Lançar"),
            )
            .command(
                Command::new("Nova receita", move || income.start_new(&income_store, Flow::Income))
                    .icon(IconName::TrendingUp)
                    .group("Lançar"),
            )
            .command(
                Command::new("Nova conta a pagar", move || bill.start_new(&bill_store))
                    .icon(IconName::CreditCard)
                    .group("Lançar"),
            )
    }

    fn page_view(&self, window: Size) -> Page {
        let opener = {
            let page = self.page.clone();
            Rc::new(move |i: usize| page.set(i)) as Rc<dyn Fn(usize)>
        };
        match self.page.get() {
            0 => dashboard::dashboard(&self.store, &self.dashboard, &self.forms, opener),
            1 => ledger::ledger(&self.store, &self.ledger, &self.forms, window),
            2 => bills::bills(&self.store, &self.bills, &self.forms, window),
            3 => agenda::agenda(&self.store, &self.agenda, &self.forms, window),
            4 => budgets::budgets(&self.store, &self.budgets, window),
            5 => reports::reports(&self.store, &self.reports),
            6 => records::records(&self.store, &self.records),
            7 => settings::settings(
                &self.store,
                &self.forms,
                &self.news,
                &self.page,
                self.training.models.clone(),
                self.training.voices.clone(),
                window,
            ),
            _ => training::training(&self.training),
        }
    }
}

/// The components' own words in Portuguese. On the agenda, the rows are the people.
fn labels() -> Labels {
    let mut labels = Labels::pt_br();
    labels.scheduler_resources = "Quem".into();
    labels
}

/// Keyboard shortcuts that work on every screen.
struct Shortcuts {
    store: Store,
    forms: Forms,
}

impl View for Shortcuts {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (store, form) = (self.store.clone(), self.forms.transaction.clone());
        cx.shortcut(Key::Character("n".into()), true, Rc::new(move || form.start_new(&store, Flow::Expense)));
        cx.empty()
    }
}

impl Component for App {
    fn body(&self) -> impl View {
        responsive(|window| self.window_view(window))
    }
}

impl App {
    fn window_view(&self, window: Size) -> impl View + use<> {
        let page = self.page_view(window);
        let main = VStack::new((
            updates::UpdatePump(self.store.updater.clone()),
            self.recorder.clone(),
            self.toolbar(page.subtitle, page.help, window),
            Divider::horizontal(),
            // Keyed by screen, so each one opens with fresh local state and scrolled to the top.
            Keyed::new(
                self.page.get() as u64,
                store::FillColumn { children: (page.body,), spacing: 0.0, padding: 0.0.into() },
            ),
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0);
        // Everything inside speaks Portuguese, the components' own words included.
        Localized::new(
            labels(),
            VStack::new((
                HStack::new((
                    self.sidebar(),
                    main,
                    assistant::chat(&self.training, &self.store, &self.forms, &self.page),
                ))
                .spacing(0.0)
                .align(Align::Stretch)
                .grow(1.0),
                updates::status_bar(&self.store.updater, &self.news, &self.reporter),
                self.forms.transaction.view(&self.store),
                self.forms.bill.view(&self.store),
                self.forms.pay.view(&self.store),
                self.forms.schedule.view(&self.store),
                self.forms.occurrence.view(&self.store),
                self.forms.confirm.clone(),
                VStack::new((self.news.view(&self.store.updater), self.reporter.clone())).spacing(0.0),
                self.palette_view(),
                Shortcuts { store: self.store.clone(), forms: self.forms.clone() },
                Toaster::new(self.store.toasts.clone()).bottom_inset(StatusBar::HEIGHT),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}
