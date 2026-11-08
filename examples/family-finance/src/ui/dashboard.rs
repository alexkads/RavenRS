//! The first screen: where the money stands and what needs attention.

use std::path::Path;
use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::help::{self, Help, tip};
use super::store::{Memo, PAGE_INSETS, Page, Store, content_width, titled_card};
use crate::model::{
    Account, Bill, BillStatus, BudgetLine, Flow, MonthTotals, date_label, month_label, month_short,
    parse_date, shift_months,
};
use crate::money::format_brl;

/// How many days ahead count as "coming up".
const SOON_DAYS: i32 = 30;
/// From this width the balance and the next bills fit side by side, and so do three numbers.
const WIDE_FROM: f32 = 900.0;
/// A big number in reais needs half of this.
const TWO_STATS_FROM: f32 = 600.0;
/// The next bills, beside the balance on a wide window.
const BILLS_WIDTH: f32 = 400.0;
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
/// From this width the spending ring has its list beside it.
const RING_BESIDE_FROM: f32 = 620.0;

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
}

impl DashboardState {
    pub fn new(path: Option<&Path>) -> Self {
        Self { numbers: Memo::new(), period: State::new(0), account: remembered_account(path) }
    }
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

/// A big number in a card, with its "?" in the corner.
fn stat_card(stat: Stat, help: Help) -> impl View {
    Card::new(HStack::new((stat.grow(1.0), tip(help))).align(Align::Start))
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
        let content = content_width(size);
        let wide = content >= WIDE_FROM;
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

        let mut income = Stat::new("Receitas do mês", format_brl(now.income_cents));
        if let Some(change) = percent_change(now.income_cents, before.income_cents) {
            income = income.delta(change, now.income_cents >= before.income_cents);
        }
        let mut expense = Stat::new("Despesas do mês", format_brl(now.expense_cents));
        if let Some(change) = percent_change(now.expense_cents, before.expense_cents) {
            // Spending less is the good direction here.
            expense = expense.delta_toned(
                change,
                now.expense_cents >= before.expense_cents,
                now.expense_cents <= before.expense_cents,
            );
        }
        let stats = Grid::new(
            if wide {
                3
            } else if content >= TWO_STATS_FROM {
                2
            } else {
                1
            },
            (
                stat_card(income, help::RECEITAS),
                stat_card(expense, help::DESPESAS),
                stat_card(Stat::new("A pagar em 30 dias", format_brl(soon_total)), help::A_PAGAR),
            ),
        )
        .spacing(16.0);

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
            .format(reais)
            .tooltip(ChartTooltip::new().indicator(ChartIndicator::Line))
            .alt(format!("{balance_label} por mês"))
            .height(if wide { 278.0 } else { 170.0 });
        let balance = Card::new(
            VStack::new((
                HStack::new((
                    Stat::new(balance_label.clone(), format_brl(total)).large().grow(1.0),
                    tip(help::SALDO),
                    Select::new(state.period.binding(), PERIODS.map(|p| p.0)).label("Período").width(120.0),
                ))
                .spacing(8.0)
                .align(Align::Start),
                balance_chart,
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
        .grow(1.0);

        let recent = numbers.months.len().saturating_sub(COMPARED_MONTHS);
        let flows = titled_card(
            "Receitas e despesas",
            help::ENTRADAS_E_SAIDAS,
            Chart::new(ChartKind::Bar)
                .categories(months.iter().skip(recent).cloned())
                .series("Receitas", numbers.months.iter().skip(recent).map(|m| m.income_cents as f64 / 100.0))
                .series(
                    "Despesas",
                    numbers.months.iter().skip(recent).map(|m| m.expense_cents as f64 / 100.0),
                )
                .format(reais)
                .tooltip(ChartTooltip::new().indicator(ChartIndicator::Dashed))
                .alt("Receitas e despesas por mês")
                .height(260.0),
        );

        let mut spent: Vec<(String, i64)> = numbers
            .budget
            .iter()
            .filter(|l| l.spent_cents > 0)
            .map(|l| (l.category.clone(), l.spent_cents))
            .collect();
        spent.sort_by_key(|(_, cents)| std::cmp::Reverse(*cents));
        let spent_total: i64 = spent.iter().map(|(_, cents)| cents).sum();
        // The theme has as many colors as a chart should have slices.
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
        let category_card: AnyView = if spending.is_empty() {
            Box::new(titled_card(
                &spending_title,
                help::POR_CATEGORIA,
                Text::new("Nenhuma despesa neste mês.").foreground(Semantic::InkMuted),
            ))
        } else {
            let whole = format_brl(spent_total);
            let whole = whole.rsplit_once(',').map_or(whole.as_str(), |(reais, _)| reais);
            let ring = Chart::new(ChartKind::Doughnut)
                .categories(spent.iter().map(|(name, _)| name.as_str()))
                .series("Despesas", spent.iter().map(|(_, cents)| *cents as f64 / 100.0))
                .item_colors(SERIES_COLORS)
                .hole(0.72)
                .center_text(whole, "no mês")
                .format(reais)
                .hide_legend()
                .alt(spending_title.as_str())
                .size(RING_SIZE, RING_SIZE);
            let list = VStack::new(spending).spacing(12.0).align(Align::Stretch).grow(1.0);
            let together: AnyView = if content >= RING_BESIDE_FROM {
                Box::new(HStack::new((ring, list)).spacing(24.0).align(Align::Center))
            } else {
                Box::new(VStack::new((ring, list)).spacing(16.0).align(Align::Center))
            };
            Box::new(titled_card(&spending_title, help::POR_CATEGORIA, together))
        };

        let mut rows: Vec<AnyView> = soon
            .iter()
            .take(5)
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
                            .spacing(6.0)
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
                    .spacing(10.0)
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
            Badge::new(format!("{} {word} · {}", overdue.len(), format_brl(overdue_total))).tone(Tone::Danger)
        });
        let bills_card = Card::new(
            VStack::new((
                HStack::new((
                    Text::new("Próximas contas").style(TextStyle::Headline),
                    tip(help::PROXIMAS),
                    Spacer::new(),
                    late,
                ))
                .spacing(8.0)
                .align(Align::Center),
                VStack::new(rows).spacing(12.0).align(Align::Stretch),
                Button::new("Ver todas as contas")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .trailing_icon(IconName::ArrowRight)
                    .on_click(move || go_bills(2)),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        );

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
                    .spacing(10.0)
                    .align(Align::Center),
                ) as AnyView
            })
            .collect();
        let accounts_card = titled_card(
            "Saldo por conta",
            help::POR_CONTA,
            VStack::new(accounts).spacing(14.0).align(Align::Stretch),
        );

        let picked = numbers.account;
        let choose = state.account.clone();
        let choices = std::iter::once((None, "Todas as contas".to_owned()))
            .chain(numbers.accounts.iter().map(|a| (Some(a.id), a.name.clone())));
        // Bills are only tied to an account when paid, so they cannot follow the choice.
        let family_bills = picked.is_some().then(|| {
            Text::new("Contas a pagar e pagamentos programados continuam os da família toda.")
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted)
                .grow(1.0)
        });
        let account_bar = HStack::new((
            Select::of(Binding::new(move || picked, move |value| choose.set(value)), choices)
                .label("Conta")
                .width(240.0),
            tip(help::CONTA),
            family_bills,
        ))
        .spacing(8.0)
        .align(Align::Center);

        let top: AnyView = if wide {
            Box::new(
                HStack::new((balance, bills_card.width(BILLS_WIDTH))).spacing(16.0).align(Align::Stretch),
            )
        } else {
            // With little room, what needs doing comes first.
            Box::new(VStack::new((bills_card, balance)).spacing(16.0).align(Align::Stretch))
        };
        let bottom = Grid::new(if wide { 2 } else { 1 }, (category_card, accounts_card)).spacing(16.0);

        ScrollView::new((VStack::new((account_bar, top, stats, flows, bottom))
            .spacing(16.0)
            .align(Align::Stretch),))
        .padding(PAGE_INSETS)
    });
    Page::new(help::PAINEL, format!("Hoje é {}", date_label(today)), body)
}
