//! The app's window: a sidebar, a toolbar and the screens.

mod agenda;
mod bills;
mod budgets;
mod dashboard;
mod forms;
mod ledger;
mod records;
mod reports;
mod settings;
mod store;
mod training;
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
use ravenrs::movie::{BugReporter, FlightRecorder};
use records::RecordsState;
use reports::ReportsState;
use store::{Memo, Store};

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
/// How much of the last minutes a bug report can show.
const FLIGHT_SECONDS: f64 = 240.0;

/// The whole app.
pub struct App {
    store: Store,
    page: State<usize>,
    palette: State<bool>,
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
        let store = Store::new(db, today, path, updater);
        let recorder = FlightRecorder::new(FLIGHT_SECONDS);
        let reporter = training::reporter(&store, &recorder);
        Self {
            store,
            page: State::new(0),
            palette: State::new(false),
            news: updates::NewsDialog::new(),
            training: training::TrainingState::new(),
            recorder,
            reporter,
            forms: Forms::new(),
            dashboard: DashboardState::new(),
            ledger: LedgerState::new(today),
            bills: BillsState::new(),
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

    /// The app's icon, decoded once: views build often.
    fn mark(&self) -> AnyView {
        match self.icon.get(0, crate::icon) {
            // Next to the name it says nothing a screen reader has not just read.
            Some(icon) => Box::new(Image::new(icon).size(26.0, 26.0).corner_radius(6.0).alt("")),
            None => Box::new(Icon::new(IconName::Wallet).color(Semantic::Accent)),
        }
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
        let current = self.page.get();
        let mut items: Vec<AnyView> = vec![
            Box::new(
                HStack::new((self.mark(), Text::new("Raven Finanças").style(TextStyle::Headline)))
                    .spacing(10.0)
                    .align(Align::Center)
                    .padding((6.0, 10.0)),
            ),
            Box::new(SidebarSection::new("Dia a dia")),
        ];
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            if i == 5 {
                items.push(Box::new(SidebarSection::new("Consultas")));
            }
            if i == 6 {
                items.push(Box::new(SidebarSection::new("Configuração")));
            }
            if i == 8 {
                items.push(Box::new(SidebarSection::new("Ajuda")));
            }
            let page = self.page.clone();
            let mut item =
                SidebarItem::new(*icon, *label).selected(current == i).on_click(move || page.set(i));
            if i == BILLS_PAGE && overdue > 0 {
                item = item.count(overdue.to_string());
            }
            items.push(Box::new(item));
        }
        Sidebar::new(items)
    }

    fn toolbar(&self) -> impl View + use<> {
        let (_, title) = PAGES[self.page.get().min(PAGES.len() - 1)];
        let palette = self.palette.clone();
        let (expense_store, expense) = (self.store.clone(), self.forms.transaction.clone());
        let (income_store, income) = (self.store.clone(), self.forms.transaction.clone());
        let (bill_store, bill) = (self.store.clone(), self.forms.bill.clone());
        HStack::new((
            Breadcrumb::new(["Raven Finanças", title]),
            Spacer::new(),
            Tooltip::new(
                IconButton::new(IconName::Search)
                    .label("Procurar comando")
                    .on_click(move || palette.set(true)),
                "Procurar comando (⌘K)",
            ),
            DropdownMenu::new("Novo")
                .icon(IconName::Plus)
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
            ThemeSwitcher,
        ))
        .spacing(10.0)
        .align(Align::Center)
        .padding((10.0, 24.0))
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

    fn page_view(&self) -> AnyView {
        let opener = {
            let page = self.page.clone();
            Rc::new(move |i: usize| page.set(i)) as Rc<dyn Fn(usize)>
        };
        match self.page.get() {
            0 => Box::new(dashboard::dashboard(&self.store, &self.dashboard, &self.forms, opener)),
            1 => Box::new(ledger::ledger(&self.store, &self.ledger, &self.forms)),
            2 => Box::new(bills::bills(&self.store, &self.bills, &self.forms)),
            3 => Box::new(agenda::agenda(&self.store, &self.agenda, &self.forms)),
            4 => Box::new(budgets::budgets(&self.store, &self.budgets)),
            5 => Box::new(reports::reports(&self.store, &self.reports)),
            6 => Box::new(records::records(&self.store, &self.records, &self.forms)),
            7 => Box::new(settings::settings(&self.store, &self.forms, &self.news, &self.page)),
            _ => Box::new(training::training(&self.training)),
        }
    }
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
        let main = VStack::new((
            updates::UpdatePump(self.store.updater.clone()),
            self.recorder.clone(),
            self.toolbar(),
            Divider::horizontal(),
            // Keyed by screen, so each one opens with fresh local state and scrolled to the top.
            Keyed::new(
                self.page.get() as u64,
                store::FillColumn { children: (self.page_view(),), spacing: 0.0, padding: 0.0 },
            ),
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0);
        // Everything inside speaks Portuguese, the components' own words included.
        Localized::new(
            Labels::pt_br(),
            VStack::new((
                HStack::new((self.sidebar(), main)).spacing(0.0).align(Align::Stretch).grow(1.0),
                updates::status_bar(&self.store.updater, &self.news, &self.reporter),
                self.forms.transaction.view(&self.store),
                self.forms.bill.view(&self.store),
                self.forms.pay.view(&self.store),
                self.forms.schedule.view(&self.store),
                self.forms.occurrence.view(&self.store),
                self.forms.confirm.view(),
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
