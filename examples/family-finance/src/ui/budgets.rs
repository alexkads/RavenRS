//! Each category's spending against its monthly limit.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::help;
use super::store::{Memo, PAGE_INSETS, Page, Store, content_width, month_nav};
use crate::model::BudgetLine;
use crate::money::{format_brl, format_input, parse_brl};

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct BudgetsState {
    month: State<Date>,
    lines: Memo<(u64, Date), Rc<Vec<BudgetLine>>>,
    editing: State<Option<(i64, String)>>,
    limit: State<String>,
}

impl BudgetsState {
    pub fn new(today: Date) -> Self {
        Self {
            month: State::new(today.first_of_month()),
            lines: Memo::new(),
            editing: State::new(None),
            limit: State::new(String::new()),
        }
    }
}

/// Below this width a category's name and how it stands no longer fit in half of it.
const TWO_COLUMNS_FROM: f32 = 700.0;
const RING_SIZE: f32 = 160.0;

fn accepts_zero(text: &str) -> Result<(), String> {
    if text.trim().is_empty() { Ok(()) } else { parse_brl(text).map(|_| ()) }
}

pub(crate) fn budgets(store: &Store, state: &BudgetsState, window: Size) -> Page {
    let content = content_width(window);
    let month = state.month.get();
    let lines =
        state.lines.get((store.version(), month), || Rc::new(store.read(|db| db.budget_lines(month))));
    let (limit_total, spent_total) =
        lines.iter().fold((0, 0), |(l, s), line| (l + line.budget_cents, s + line.spent_cents));
    let over: Vec<&BudgetLine> =
        lines.iter().filter(|l| l.budget_cents > 0 && l.spent_cents > l.budget_cents).collect();
    let share = if limit_total > 0 { spent_total as f32 / limit_total as f32 } else { 0.0 };
    let spent = Stat::new("Gasto no mês", format_brl(spent_total)).large();
    let ring = Chart::new(ChartKind::RadialBar)
        .categories(["Gasto no mês"])
        .series("Gasto no mês", [if share > 0.0 { 1.0 } else { 0.0 }])
        .item_colors([if share > 1.0 { Semantic::Danger } else { Semantic::Chart1 }])
        .angles(0.0, share.min(1.0) * 360.0)
        .tracks()
        .hole(0.74)
        .center_text(format!("{:.0}%", share * 100.0), "do limite")
        .hide_legend()
        .hide_tooltip()
        .alt(format!("{:.0}% do limite gasto no mês", share * 100.0))
        .size(RING_SIZE, RING_SIZE);
    let progress = VStack::new((
        spent,
        HStack::new((
            Text::new(format!("{:.0}% do limite de {}", share * 100.0, format_brl(limit_total)))
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            Spacer::new(),
            (!over.is_empty()).then(|| {
                let names: Vec<&str> = over.iter().map(|l| l.category.as_str()).collect();
                let verb = if names.len() == 1 { "passou" } else { "passaram" };
                Badge::new(format!("{} {verb} do limite", names.join(", "))).tone(Tone::Danger)
            }),
        ))
        .spacing(8.0)
        .align(Align::Center),
    ))
    .spacing(10.0)
    .align(Align::Stretch)
    .grow(1.0);
    let summary: AnyView = if content < TWO_COLUMNS_FROM {
        Box::new(Card::new(VStack::new((ring, progress)).spacing(16.0).align(Align::Center)))
    } else {
        Box::new(Card::new(HStack::new((ring, progress)).spacing(32.0).align(Align::Center)))
    };

    let mut rows: Vec<AnyView> = lines
        .iter()
        .map(|line| {
            let ratio =
                if line.budget_cents > 0 { line.spent_cents as f32 / line.budget_cents as f32 } else { 0.0 };
            let (tone, status) = match line.budget_cents {
                0 => (Tone::Neutral, "Sem limite".to_owned()),
                _ if ratio > 1.0 => (
                    Tone::Danger,
                    format!("{} acima do limite", format_brl(line.spent_cents - line.budget_cents)),
                ),
                _ if ratio >= 0.8 => {
                    (Tone::Warning, format!("Restam {}", format_brl(line.budget_cents - line.spent_cents)))
                }
                _ if line.spent_cents == 0 => {
                    (Tone::Neutral, format!("Restam {}", format_brl(line.budget_cents)))
                }
                _ => (Tone::Success, format!("Restam {}", format_brl(line.budget_cents - line.spent_cents))),
            };
            let (editing, limit, id, name, cents) = (
                state.editing.clone(),
                state.limit.clone(),
                line.category_id,
                line.category.clone(),
                line.budget_cents,
            );
            let open_name = name.clone();
            Box::new(Card::new(
                VStack::new((
                    HStack::new((
                        Text::new(line.category.clone()).style(TextStyle::Headline).truncate().grow(1.0),
                        Badge::new(status).tone(tone),
                        IconButton::new(IconName::Pencil)
                            .label(format!("Definir limite de {name}"))
                            .size(ButtonSize::Small)
                            .on_click(move || {
                                limit.set(if cents > 0 { format_input(cents) } else { String::new() });
                                editing.set(Some((id, open_name.clone())));
                            }),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                    ProgressBar::new(ratio.min(1.0)).tone(tone),
                    Text::new(if line.budget_cents > 0 {
                        format!("{} de {}", format_brl(line.spent_cents), format_brl(line.budget_cents))
                    } else {
                        format_brl(line.spent_cents)
                    })
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            )) as AnyView
        })
        .collect();
    if rows.is_empty() {
        rows.push(Box::new(
            Text::new("Cadastre categorias de despesa para acompanhar o orçamento.")
                .foreground(Semantic::InkMuted),
        ));
    }

    let dialog = state.editing.get().map(|(id, name)| {
        let (read, write) = (state.editing.clone(), state.editing.clone());
        let open = Binding::new(
            move || read.get().is_some(),
            move |open: bool| {
                if !open {
                    write.set(None);
                }
            },
        );
        let (cancel, done, text, save_store) =
            (state.editing.clone(), state.editing.clone(), state.limit.clone(), store.clone());
        Dialog::new(open, format!("Limite de {name}"))
            .description(
                "Quanto a família quer gastar nesta categoria por mês. Deixe vazio para não ter limite.",
            )
            .content(
                FormField::new(
                    "Limite mensal",
                    TextField::new(state.limit.binding()).placeholder("0,00").autofocus(),
                )
                .validate(state.limit.binding(), accepts_zero),
            )
            .action(Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(None)))
            .action(Button::new("Salvar limite").icon(IconName::Check).on_click(move || {
                let typed = text.get();
                let Ok(cents) = (if typed.trim().is_empty() { Ok(0) } else { parse_brl(&typed) }) else {
                    return;
                };
                if save_store.write("Limite salvo", |db| db.set_budget(id, cents)).is_some() {
                    done.set(None);
                }
            }))
    });

    Page::new(
        help::ORCAMENTOS,
        "Quanto a família gastou de cada limite no mês.",
        ScrollView::new((VStack::new((
            month_nav(&state.month, store.today),
            summary,
            Grid::new(if content < TWO_COLUMNS_FROM { 1 } else { 2 }, rows).spacing(12.0),
            dialog,
        ))
        .spacing(16.0)
        .align(Align::Stretch),))
        .padding(PAGE_INSETS),
    )
}
