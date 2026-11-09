//! The first screen: where the money stands and what needs attention.

use std::path::Path;
use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::help::{self, tip};
use super::store::{Memo, PAGE_INSETS, Page, Store, content_width};
use crate::model::{
    Account, Bill, BillStatus, BudgetLine, Flow, MonthTotals, date_label, month_label, month_short,
    parse_date, shift_months,
};
use crate::money::format_brl;

/// How many days ahead count as "coming up".
const SOON_DAYS: i32 = 30;
/// What the balance chart can cover, and how many months that is.
const PERIODS: [(&str, usize); 3] = [("12 meses", 12), ("6 meses", 6), ("3 meses", 3)];
/// How many months income and expenses are compared over.
const COMPARED_MONTHS: usize = 6;
/// Beside the database, this file keeps the account the dashboard shows.
const ACCOUNT_FILE: &str = "painel-conta";
/// The slices of the spending ring, in order.
const SERIES_COLORS: [Semantic; 5] =
    [Semantic::Chart1, Semantic::Chart2, Semantic::Chart3, Semantic::Chart4, Semantic::Chart5];
const RING_SIZE: f32 = 220.0;
/// Beside the database, this file keeps where each card stands and its size.
const LAYOUT_FILE: &str = "painel-cards.json";
/// The cards, by key, and the cells each takes until the person moves it: `(key, columns, rows)`.
const CARDS: [(&str, u16, u16); 8] = [
    ("saldo", 4, 3),
    ("proximas", 2, 3),
    ("receitas", 2, 1),
    ("despesas", 2, 1),
    ("a-pagar", 2, 1),
    ("fluxos", 6, 2),
    ("categorias", 3, 2),
    ("por-conta", 3, 2),
];
const COLUMNS: usize = 6;
/// Below this width the cards go one under the other, in two columns.
const WIDE_FROM: f32 = 900.0;
const ROW_HEIGHT: f32 = 140.0;
/// What `TileLayout` takes from a tile's height: the gap to the next row, its header and its
/// bottom margin.
const TILE_FRAME: f32 = 12.0 + 48.0 + 16.0;

#[derive(Default)]
struct Numbers {
    /// The account the numbers are of, or `None` for all of them.
    account: Option<i64>,
    accounts: Vec<Account>,
    months: Vec<MonthTotals>,
    /// Last month up to this same day: a month that just began compares with this, not with a whole month.
    last_month_so_far: MonthTotals,
    balances: Vec<i64>,
    budget: Vec<BudgetLine>,
    open_bills: Vec<Bill>,
    /// Payments planned for the coming days, still to confirm.
    planned_out_cents: i64,
}

/// The screen's data, kept between rebuilds.
#[derive(Clone)]
pub(crate) struct DashboardState {
    numbers: Memo<(u64, Option<i64>), Rc<Numbers>>,
    /// Which of the [`PERIODS`] the balance chart covers.
    period: State<usize>,
    /// The account chosen at the top, or `None` for all of them, kept between openings.
    account: Binding<Option<i64>>,
    /// Where each card stands and how big, kept between openings.
    layout: Binding<Vec<TileSpot>>,
}

impl DashboardState {
    pub fn new(path: Option<&Path>) -> Self {
        Self {
            numbers: Memo::new(),
            period: State::new(0),
            account: remembered_account(path),
            layout: remembered_layout(path),
        }
    }
}

fn default_layout() -> Vec<TileSpot> {
    CARDS.iter().map(|&(key, columns, rows)| TileSpot::new(key, columns, rows)).collect()
}

/// Reads where the cards were left, and keeps each change beside the database. A file the
/// app cannot read gives the cards their places again.
fn remembered_layout(path: Option<&Path>) -> Binding<Vec<TileSpot>> {
    let file = path.map(|path| path.with_extension(LAYOUT_FILE));
    let kept = file
        .as_ref()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(default_layout);
    let layout = State::new(kept);
    let (read, write) = (layout.clone(), layout);
    Binding::new(
        move || read.get(),
        move |value: Vec<TileSpot>| {
            // A resize reports every move of the pointer, most of them changing nothing.
            if *write.peek() == value {
                return;
            }
            if let Some(file) = &file
                && let Err(e) = serde_json::to_string(&value)
                    .map_err(std::io::Error::other)
                    .and_then(|text| std::fs::write(file, text))
            {
                tracing::warn!(error = %e, "could not remember the dashboard's layout");
            }
            write.set(value);
        },
    )
}

/// The height a card's content has, for the cells it takes.
fn room(spots: &[TileSpot], key: &str) -> f32 {
    let rows = spots.iter().find(|s| s.key == key).map_or(1, |s| s.rows);
    f32::from(rows) * (ROW_HEIGHT + 12.0) - TILE_FRAME
}

/// The columns a card takes.
fn width_of(spots: &[TileSpot], key: &str) -> u16 {
    spots.iter().find(|s| s.key == key).map_or(1, |s| s.columns)
}

/// Reads the account the dashboard was left on, and keeps it beside the database.
fn remembered_account(path: Option<&Path>) -> Binding<Option<i64>> {
    let file = path.map(|path| path.with_extension(ACCOUNT_FILE));
    let kept = file.as_ref().and_then(|file| std::fs::read_to_string(file).ok()?.trim().parse().ok());
    let account = State::new(kept);
    let (read, write) = (account.clone(), account);
    Binding::new(
        move || read.get(),
        move |value: Option<i64>| {
            write.set(value);
            if let Some(file) = &file {
                let saved = match value {
                    Some(id) => std::fs::write(file, id.to_string()),
                    None => std::fs::remove_file(file).or_else(|e| match e.kind() {
                        std::io::ErrorKind::NotFound => Ok(()),
                        _ => Err(e),
                    }),
                };
                if let Err(e) = saved {
                    tracing::warn!(error = %e, "could not remember the dashboard's account");
                }
            }
        },
    )
}

/// A sum of money in big figures that, when it changes, as with another account, counts
/// from what it showed to what it is.
fn money(cents: i64) -> CountUp {
    CountUp::new(cents as f64)
        .format(|cents| format_brl(cents.round() as i64))
        .style(TextStyle::Metric)
        .start_at_value()
        .seconds(0.75)
}

/// A big number with how it compares with last month; its title is the card's.
fn figure(cents: i64, delta: Option<(String, bool, bool)>) -> impl View {
    let delta = delta.map(|(text, up, good)| {
        Badge::new(text).tone(if good { Tone::Success } else { Tone::Danger }).icon(if up {
            IconName::TrendingUp
        } else {
            IconName::TrendingDown
        })
    });
    VStack::new((money(cents), delta)).spacing(8.0).align(Align::Start)
}

fn tone_of(status: BillStatus) -> Tone {
    match status {
        BillStatus::Overdue => Tone::Danger,
        BillStatus::DueToday => Tone::Warning,
        BillStatus::Paid => Tone::Success,
        BillStatus::Upcoming => Tone::Neutral,
    }
}

/// A chart's number, which comes in reais, the way the app writes money.
fn reais(value: f64) -> String {
    format_brl((value * 100.0).round() as i64)
}

/// A part of a total, where a tiny part is not nothing.
fn share_of(part: i64, total: i64) -> String {
    let percent = part as f64 / total.max(1) as f64 * 100.0;
    if percent > 0.0 && percent < 1.0 { "<1%".to_owned() } else { format!("{percent:.0}%") }
}

fn percent_change(now: i64, before: i64) -> Option<String> {
    let change = (now - before) as f64 / before.max(1) as f64 * 100.0;
    // Nothing changed is nothing to point out.
    (before > 0 && change.abs() >= 0.5).then(|| format!("{change:+.0}% vs. mês passado"))
}

pub(crate) fn dashboard(
    store: &Store,
    state: &DashboardState,
    forms: &Forms,
    open_page: Rc<dyn Fn(usize)>,
) -> Page {
    let numbers = state.numbers.get((store.version(), state.account.get()), || {
        let today = store.today;
        let accounts = store.read(|db| db.accounts());
        // An account deleted since it was chosen leaves every account shown.
        let account = state.account.get().filter(|id| accounts.iter().any(|a| a.id == *id));
        Rc::new(Numbers {
            account,
            accounts,
            months: store.read(|db| db.monthly_totals_of(today, 12, account)),
            last_month_so_far: store.read(|db| {
                let start = today.first_of_month().add_months(-1);
                db.totals_between_of(start, shift_months(start, 0, today.day()).add_days(1), account)
            }),
            balances: store.read(|db| db.balance_history_of(today, 12, account)),
            budget: store.read(|db| db.budget_lines_of(today, account)),
            // Bills have no account until they are paid, so these stay the family's, and so
            // do the payments planned with them, for the two cards to agree.
            open_bills: store.read(|db| db.open_bills()),
            planned_out_cents: store
                .read(|db| db.occurrences(today, today.add_days(SOON_DAYS)))
                .iter()
                .filter(|o| o.transaction_id.is_none() && o.schedule.input.flow == Flow::Expense)
                .map(|o| o.schedule.input.amount_cents)
                .sum(),
        })
    });
    let today = store.today;
    let (store, forms, state) = (store.clone(), forms.clone(), state.clone());
    let body = responsive(move |size| {
        let numbers = numbers.clone();
        let spots = state.layout.get();
        let chosen = numbers.accounts.iter().find(|a| Some(a.id) == numbers.account);
        let total: i64 = match chosen {
            Some(account) => account.balance_cents,
            None => numbers.accounts.iter().map(|a| a.balance_cents).sum(),
        };
        let balance_label =
            chosen.map_or_else(|| "Saldo das contas".to_owned(), |a| format!("Saldo de {}", a.name));
        let empty = MonthTotals::default();
        let now = numbers.months.last().unwrap_or(&empty);
        let before = &numbers.last_month_so_far;
        let overdue: Vec<&Bill> =
            numbers.open_bills.iter().filter(|b| b.status(today) == BillStatus::Overdue).collect();
        let overdue_total: i64 = overdue.iter().map(|b| b.amount_cents).sum();
        let horizon = today.add_days(SOON_DAYS);
        let soon: Vec<&Bill> = numbers.open_bills.iter().filter(|b| b.due <= horizon).collect();
        let soon_total: i64 = soon.iter().map(|b| b.amount_cents).sum::<i64>() + numbers.planned_out_cents;

        let income = figure(
            now.income_cents,
            percent_change(now.income_cents, before.income_cents).map(|change| {
                (change, now.income_cents >= before.income_cents, now.income_cents >= before.income_cents)
            }),
        );
        let expense = figure(
            now.expense_cents,
            percent_change(now.expense_cents, before.expense_cents).map(|change| {
                (change, now.expense_cents >= before.expense_cents, now.expense_cents <= before.expense_cents)
            }),
        );

        let months: Vec<String> = numbers
            .months
            .iter()
            .filter_map(|m| parse_date(&format!("{}-01", m.month)))
            .map(month_short)
            .collect();
        let period = state.period.get().min(PERIODS.len() - 1);
        let shown = PERIODS[period].1.min(numbers.balances.len());
        let skip = numbers.balances.len() - shown;
        let balance_chart = Chart::new(ChartKind::Area)
            .categories(months.iter().skip(months.len().saturating_sub(shown)).cloned())
            .series("Saldo", numbers.balances.iter().skip(skip).map(|c| *c as f64 / 100.0))
            .curve(ChartCurve::Monotone)
            .gradient()
            .tight_scale()
            .animate_changes()
            .format(reais)
            .tooltip(ChartTooltip::new().indicator(ChartIndicator::Line))
            .alt(format!("{balance_label} por mês"))
            // Another period is another picture: it draws in again.
            .appear_key(period)
            // Below the big number, the chart takes what the card has left.
            .height((room(&spots, "saldo") - 64.0).max(60.0));
        let balance = Tile::new(
            "saldo",
            balance_label.clone(),
            VStack::new((money(total), balance_chart)).spacing(8.0).align(Align::Stretch),
        )
        .accessory(
            HStack::new((
                tip(help::SALDO),
                Select::new(state.period.binding(), PERIODS.map(|p| p.0))
                    .label("Período")
                    .size(ButtonSize::Small)
                    .width(120.0),
            ))
            .spacing(8.0)
            .align(Align::Center),
        );

        let recent = numbers.months.len().saturating_sub(COMPARED_MONTHS);
        let flows = Tile::new(
            "fluxos",
            "Receitas e despesas",
            Chart::new(ChartKind::Bar)
                .categories(months.iter().skip(recent).cloned())
                .series("Receitas", numbers.months.iter().skip(recent).map(|m| m.income_cents as f64 / 100.0))
                .series(
                    "Despesas",
                    numbers.months.iter().skip(recent).map(|m| m.expense_cents as f64 / 100.0),
                )
                .format(reais)
                .animate_changes()
                .tooltip(ChartTooltip::new().indicator(ChartIndicator::Dashed))
                .alt("Receitas e despesas por mês")
                .height(room(&spots, "fluxos").max(60.0)),
        )
        .accessory(tip(help::ENTRADAS_E_SAIDAS));

        let mut spent: Vec<(String, i64)> = numbers
            .budget
            .iter()
            .filter(|l| l.spent_cents > 0)
            .map(|l| (l.category.clone(), l.spent_cents))
            .collect();
        spent.sort_by_key(|(_, cents)| std::cmp::Reverse(*cents));
        let spent_total: i64 = spent.iter().map(|(_, cents)| cents).sum();
        if spent.len() > SERIES_COLORS.len() {
            let others: i64 = spent[SERIES_COLORS.len() - 1..].iter().map(|(_, cents)| cents).sum();
            spent.truncate(SERIES_COLORS.len() - 1);
            spent.push(("Outras".to_owned(), others));
        }
        let spending: Vec<AnyView> = spent
            .iter()
            .zip(SERIES_COLORS)
            .map(|((name, cents), color)| {
                Box::new(
                    HStack::new((
                        Icon::new(IconName::Circle).size(10.0).color(color),
                        Text::new(name.clone()).style(TextStyle::Callout).truncate().grow(1.0),
                        Text::new(format_brl(*cents)).style(TextStyle::Callout),
                        Text::new(share_of(*cents, spent_total))
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                ) as AnyView
            })
            .collect();
        let spending_title = format!("Despesas de {}", month_label(today));
        let spending_view: AnyView = if spending.is_empty() {
            Box::new(Text::new("Nenhuma despesa neste mês.").foreground(Semantic::InkMuted))
        } else {
            let whole = format_brl(spent_total);
            let whole = whole.rsplit_once(',').map_or(whole.as_str(), |(reais, _)| reais);
            let size = room(&spots, "categorias").clamp(80.0, RING_SIZE);
            // As many slices for every account, the missing ones empty, so that another
            // account turns and grows them instead of drawing the ring again.
            let slices = (0..SERIES_COLORS.len()).map(|i| spent.get(i));
            let ring = Chart::new(ChartKind::Doughnut)
                .categories(slices.clone().map(|s| s.map_or("", |(name, _)| name.as_str())))
                .series("Despesas", slices.map(|s| s.map_or(0.0, |(_, cents)| *cents as f64 / 100.0)))
                .item_colors(SERIES_COLORS)
                .animate_changes()
                .hole(0.72)
                .center_text(whole, "no mês")
                .format(reais)
                .hide_legend()
                .alt(spending_title.as_str())
                .size(size, size);
            let list = VStack::new(spending).spacing(12.0).align(Align::Stretch).grow(1.0);
            // A narrow card has room for the list only under the ring.
            if width_of(&spots, "categorias") >= 3 {
                Box::new(HStack::new((ring, list)).spacing(24.0).align(Align::Center))
            } else {
                Box::new(VStack::new((ring, list)).spacing(16.0).align(Align::Center))
            }
        };
        let categories = Tile::new("categorias", spending_title.clone(), spending_view)
            .accessory(tip(help::POR_CATEGORIA));

        // As many bills as the card has room for, under the button to see them all.
        let warning = if overdue.is_empty() { 0.0 } else { 36.0 };
        let fit = ((room(&spots, "proximas") - 44.0 - warning) / 58.0).floor().max(1.0) as usize;
        let mut rows: Vec<AnyView> = soon
            .iter()
            .take(fit)
            .map(|bill| {
                let status = bill.status(today);
                let (id, pay_store, pay) = (bill.id, store.clone(), forms.pay.clone());
                Box::new(
                    HStack::new((
                        VStack::new((
                            Text::new(bill.title()).style(TextStyle::Callout).truncate(),
                            HStack::new((
                                Badge::new(status.label()).tone(tone_of(status)),
                                Text::new(date_label(bill.due))
                                    .style(TextStyle::Callout)
                                    .foreground(Semantic::InkMuted),
                            ))
                            .spacing(8.0)
                            .align(Align::Center),
                        ))
                        .spacing(4.0)
                        .grow(1.0),
                        Text::new(format_brl(bill.amount_cents)).style(TextStyle::Callout),
                        Button::new("Pagar")
                            .accessible_name(format!("Pagar {}", bill.title()))
                            .size(ButtonSize::Small)
                            .style(ButtonStyle::Secondary)
                            .on_click(move || pay.start(&pay_store, id)),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                ) as AnyView
            })
            .collect();
        if rows.is_empty() {
            rows.push(Box::new(
                Text::new("Nada para pagar nos próximos 30 dias.").foreground(Semantic::InkMuted),
            ));
        }
        let go_bills = open_page.clone();
        let late = (!overdue.is_empty()).then(|| {
            let word = if overdue.len() == 1 { "vencida" } else { "vencidas" };
            HStack::new((
                Badge::new(format!("{} {word} · {}", overdue.len(), format_brl(overdue_total)))
                    .tone(Tone::Danger),
                Spacer::new(),
            ))
        });
        let bills = Tile::new(
            "proximas",
            "Próximas contas",
            VStack::new((
                late,
                VStack::new(rows).spacing(12.0).align(Align::Stretch),
                Button::new("Ver todas as contas")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .trailing_icon(IconName::ArrowRight)
                    .on_click(move || go_bills(2)),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
        .accessory(tip(help::PROXIMAS));

        let accounts: Vec<AnyView> = numbers
            .accounts
            .iter()
            .map(|a| {
                Box::new(
                    HStack::new((
                        Icon::new(IconName::Wallet).color(Semantic::InkMuted),
                        Text::new(a.name.clone()).style(TextStyle::Body).grow(1.0),
                        Text::new(format_brl(a.balance_cents)).style(TextStyle::Body),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                ) as AnyView
            })
            .collect();
        let per_account = Tile::new(
            "por-conta",
            "Saldo por conta",
            VStack::new(accounts).spacing(16.0).align(Align::Stretch),
        )
        .accessory(tip(help::POR_CONTA));

        let picked = numbers.account;
        let choose = state.account.clone();
        let choices = std::iter::once((None, "Todas as contas".to_owned()))
            .chain(numbers.accounts.iter().map(|a| (Some(a.id), a.name.clone())));
        // Bills are only tied to an account when paid, so they cannot follow the choice.
        let family_bills = picked.is_some().then(|| {
            Text::new("Contas a pagar e pagamentos programados continuam os da família toda.")
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted)
        });
        let layout = state.layout.clone();
        let account_bar = HStack::new((
            Select::of(Binding::new(move || picked, move |value| choose.set(value)), choices)
                .label("Conta")
                .width(240.0),
            tip(help::CONTA),
            family_bills,
            Spacer::new(),
            Button::new("Restaurar layout")
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .icon(IconName::RotateCcw)
                .disabled(spots == default_layout())
                .on_click(move || layout.set(default_layout())),
            tip(help::LAYOUT),
        ))
        .spacing(8.0)
        .align(Align::Center);

        let tiles = TileLayout::new(state.layout.clone())
            .columns(if content_width(size) >= WIDE_FROM { COLUMNS } else { 2 })
            .min_column_width(140.0)
            .row_height(ROW_HEIGHT)
            .tile(balance)
            .tile(bills)
            .tile(Tile::new("receitas", "Receitas do mês", income).accessory(tip(help::RECEITAS)))
            .tile(Tile::new("despesas", "Despesas do mês", expense).accessory(tip(help::DESPESAS)))
            .tile(
                Tile::new("a-pagar", "A pagar em 30 dias", figure(soon_total, None))
                    .accessory(tip(help::A_PAGAR)),
            )
            .tile(flows)
            .tile(categories)
            .tile(per_account);

        ScrollView::new((VStack::new((account_bar, tiles)).spacing(16.0).align(Align::Stretch),))
            .padding(PAGE_INSETS)
    });
    Page::new(help::PAINEL, format!("Hoje é {}", date_label(today)), body)
}
