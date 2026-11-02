//! The first screen: where the money stands and what needs attention.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
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
/// The height of that row, which fits the chart and five bills.
const TOP_HEIGHT: f32 = 400.0;

#[derive(Default)]
struct Numbers {
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
pub(crate) struct DashboardState(Memo<u64, Rc<Numbers>>);

impl DashboardState {
    pub fn new() -> Self {
        Self(Memo::new())
    }
}

fn tone_of(status: BillStatus) -> Tone {
    match status {
        BillStatus::Overdue => Tone::Danger,
        BillStatus::DueToday => Tone::Warning,
        BillStatus::Paid => Tone::Success,
        BillStatus::Upcoming => Tone::Neutral,
    }
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
    let numbers = state.0.get(store.version(), || {
        let today = store.today;
        Rc::new(Numbers {
            accounts: store.read(|db| db.accounts()),
            months: store.read(|db| db.monthly_totals(today, 12)),
            last_month_so_far: store.read(|db| {
                let start = today.first_of_month().add_months(-1);
                db.totals_between(start, shift_months(start, 0, today.day()).add_days(1))
            }),
            balances: store.read(|db| db.balance_history(today, 12)),
            budget: store.read(|db| db.budget_lines(today)),
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
    let (store, forms) = (store.clone(), forms.clone());
    let body = responsive(move |size| {
        let numbers = numbers.clone();
        let content = content_width(size);
        let wide = content >= WIDE_FROM;
        let total: i64 = numbers.accounts.iter().map(|a| a.balance_cents).sum();
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
                Card::new(income),
                Card::new(expense),
                Card::new(Stat::new("A pagar em 30 dias", format_brl(soon_total))),
            ),
        )
        .spacing(16.0);

        let months: Vec<String> = numbers
            .months
            .iter()
            .filter_map(|m| parse_date(&format!("{}-01", m.month)))
            .map(month_short)
            .collect();
        let balance = Card::new(
            VStack::new((
                Stat::new("Saldo das contas", format_brl(total)).large(),
                AreaChart::new(numbers.balances.iter().map(|c| *c as f32 / 100.0)).height(if wide {
                    250.0
                } else {
                    170.0
                }),
                HStack::new((
                    Text::new(months.first().cloned().unwrap_or_default())
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                    Spacer::new(),
                    Text::new(months.last().cloned().unwrap_or_default())
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                )),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
        .grow(1.0);

        let largest = numbers.budget.iter().map(|l| l.spent_cents).max().unwrap_or(0).max(1);
        let spending: Vec<AnyView> = numbers
            .budget
            .iter()
            .filter(|l| l.spent_cents > 0)
            .take(6)
            .map(|l| {
                Box::new(
                    VStack::new((
                        HStack::new((
                            Text::new(l.category.clone()).style(TextStyle::Callout),
                            Spacer::new(),
                            Text::new(format_brl(l.spent_cents)).style(TextStyle::Callout),
                        )),
                        ProgressBar::new(l.spent_cents as f32 / largest as f32),
                    ))
                    .spacing(4.0)
                    .align(Align::Stretch),
                ) as AnyView
            })
            .collect();
        let spending_title = format!("Despesas de {}", month_label(today));
        let category_card: AnyView = if spending.is_empty() {
            Box::new(titled_card(
                &spending_title,
                Text::new("Nenhuma despesa neste mês.").foreground(Semantic::InkMuted),
            ))
        } else {
            Box::new(titled_card(&spending_title, VStack::new(spending).spacing(12.0).align(Align::Stretch)))
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
                HStack::new((Text::new("Próximas contas").style(TextStyle::Headline).grow(1.0), late))
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
        let accounts_card =
            titled_card("Saldo por conta", VStack::new(accounts).spacing(14.0).align(Align::Stretch));

        let top: AnyView = if wide {
            Box::new(
                HStack::new((balance, bills_card.width(BILLS_WIDTH)))
                    .spacing(16.0)
                    .align(Align::Stretch)
                    .height(TOP_HEIGHT),
            )
        } else {
            // With little room, what needs doing comes first.
            Box::new(VStack::new((bills_card, balance)).spacing(16.0).align(Align::Stretch))
        };
        let bottom = Grid::new(if wide { 2 } else { 1 }, (category_card, accounts_card)).spacing(16.0);

        ScrollView::new((VStack::new((top, stats, bottom)).spacing(16.0).align(Align::Stretch),))
            .padding(PAGE_INSETS)
    });
    Page::new(format!("Hoje é {}", date_label(today)), body)
}
