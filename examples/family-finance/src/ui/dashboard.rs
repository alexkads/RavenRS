//! The first screen: where the money stands and what needs attention.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Memo, Store, heading};
use crate::model::{
    Account, Bill, BillStatus, BudgetLine, MonthTotals, date_label, month_label, month_short, parse_date,
    shift_months,
};
use crate::money::format_brl;

/// How many days ahead count as "coming up".
const SOON_DAYS: i32 = 30;

#[derive(Default)]
struct Numbers {
    accounts: Vec<Account>,
    months: Vec<MonthTotals>,
    /// Last month up to this same day: a month that just began compares with this, not with a whole month.
    last_month_so_far: MonthTotals,
    balances: Vec<i64>,
    budget: Vec<BudgetLine>,
    open_bills: Vec<Bill>,
}

/// The screen's data, kept between rebuilds.
#[derive(Clone)]
pub(crate) struct DashboardState(Memo<u64, Rc<Numbers>>);

impl DashboardState {
    pub fn new() -> Self {
        Self(Memo::new())
    }
}

fn card<V: View + 'static>(title: &str, content: V) -> impl View + use<V> {
    let title = title.to_owned();
    Card::new(
        VStack::new((Text::new(title).style(TextStyle::Headline), content))
            .spacing(14.0)
            .align(Align::Stretch),
    )
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
    (before > 0)
        .then(|| format!("{:+.0}% sobre o mesmo período", (now - before) as f64 / before as f64 * 100.0))
}

pub(crate) fn dashboard(
    store: &Store,
    state: &DashboardState,
    forms: &Forms,
    open_page: Rc<dyn Fn(usize)>,
) -> impl View + use<> {
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
        })
    });
    let today = store.today;
    let (store, forms) = (store.clone(), forms.clone());
    responsive(move |size| {
        let numbers = numbers.clone();
        let wide = size.width >= 1180.0;
        let total: i64 = numbers.accounts.iter().map(|a| a.balance_cents).sum();
        let empty = MonthTotals::default();
        let now = numbers.months.last().unwrap_or(&empty);
        let before = &numbers.last_month_so_far;
        let overdue: Vec<&Bill> =
            numbers.open_bills.iter().filter(|b| b.status(today) == BillStatus::Overdue).collect();
        let overdue_total: i64 = overdue.iter().map(|b| b.amount_cents).sum();
        let horizon = today.add_days(SOON_DAYS);
        let soon: Vec<&Bill> = numbers.open_bills.iter().filter(|b| b.due <= horizon).collect();
        let soon_total: i64 = soon.iter().map(|b| b.amount_cents).sum();

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
            if wide { 4 } else { 2 },
            (
                Card::new(Stat::new("Saldo das contas", format_brl(total))),
                Card::new(income),
                Card::new(expense),
                Card::new(Stat::new("A pagar em 30 dias", format_brl(soon_total))),
            ),
        )
        .spacing(16.0);

        let warning = (!overdue.is_empty()).then(|| {
            Alert::new(
                format!(
                    "{} {} vencida{}",
                    overdue.len(),
                    if overdue.len() == 1 { "conta" } else { "contas" },
                    if overdue.len() == 1 { "" } else { "s" }
                ),
                format!("{} em atraso. Abra Contas a pagar para resolver.", format_brl(overdue_total)),
            )
            .tone(Tone::Danger)
        });

        let months: Vec<String> = numbers
            .months
            .iter()
            .filter_map(|m| parse_date(&format!("{}-01", m.month)))
            .map(month_short)
            .collect();
        let balance_chart = card(
            "Saldo nos últimos 12 meses",
            VStack::new((
                AreaChart::new(numbers.balances.iter().map(|c| *c as f32 / 100.0)).height(190.0),
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
            .spacing(6.0)
            .align(Align::Stretch),
        );
        let spending: Vec<(String, f32)> = numbers
            .budget
            .iter()
            .filter(|l| l.spent_cents > 0)
            .take(8)
            .map(|l| (l.category.clone(), l.spent_cents as f32 / 100.0))
            .collect();
        let category_chart: AnyView = if spending.is_empty() {
            Box::new(card(
                &format!("Despesas de {}", month_label(today)),
                Text::new("Nenhuma despesa neste mês.").foreground(Semantic::InkMuted),
            ))
        } else {
            Box::new(card(
                &format!("Despesas de {}", month_label(today)),
                BarChart::new(spending).height(214.0),
            ))
        };
        let charts = Grid::new(if wide { 2 } else { 1 }, (balance_chart, category_chart)).spacing(16.0);

        let mut rows: Vec<AnyView> = soon
            .iter()
            .take(7)
            .map(|bill| {
                let status = bill.status(today);
                let (id, pay_store, pay) = (bill.id, store.clone(), forms.pay.clone());
                Box::new(
                    HStack::new((
                        VStack::new((
                            Text::new(bill.title()).style(TextStyle::Body).truncate(),
                            Text::new(format!("{} · {}", date_label(bill.due), bill.category))
                                .style(TextStyle::Caption)
                                .foreground(Semantic::InkMuted),
                        ))
                        .spacing(2.0)
                        .grow(1.0),
                        Badge::new(status.label()).tone(tone_of(status)),
                        Text::new(format_brl(bill.amount_cents)).style(TextStyle::Body),
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
        let bills_card = card(
            "Próximas contas",
            VStack::new((
                VStack::new(rows).spacing(12.0).align(Align::Stretch),
                Button::new("Ver todas as contas")
                    .style(ButtonStyle::Ghost)
                    .trailing_icon(IconName::ArrowRight)
                    .on_click(move || go_bills(2)),
            ))
            .spacing(10.0)
            .align(Align::Stretch),
        );

        let accounts = numbers.accounts.iter().map(|a| {
            ListItem::new(a.name.clone())
                .subtitle(a.kind.label())
                .icon(IconName::Wallet)
                .trailing(format_brl(a.balance_cents))
        });
        let accounts_card = card("Contas", List::new(accounts.collect::<Vec<_>>()));
        let bottom = Grid::new(if wide { 2 } else { 1 }, (bills_card, accounts_card)).spacing(16.0);

        ScrollView::new((VStack::new((
            heading("Painel", format!("Hoje é {}", date_label(today))),
            warning,
            stats,
            charts,
            bottom,
        ))
        .spacing(16.0)
        .align(Align::Stretch),))
        .padding(24.0)
    })
}
