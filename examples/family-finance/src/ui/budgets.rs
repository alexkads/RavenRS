//! Each category's spending against its monthly limit.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::store::{Memo, Store, heading, month_nav};
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

fn accepts_zero(text: &str) -> Result<(), String> {
    if text.trim().is_empty() { Ok(()) } else { parse_brl(text).map(|_| ()) }
}

pub(crate) fn budgets(store: &Store, state: &BudgetsState) -> impl View + use<> {
    let month = state.month.get();
    let lines =
        state.lines.get((store.version(), month), || Rc::new(store.read(|db| db.budget_lines(month))));
    let (limit_total, spent_total) =
        lines.iter().fold((0, 0), |(l, s), line| (l + line.budget_cents, s + line.spent_cents));
    let over: Vec<&BudgetLine> =
        lines.iter().filter(|l| l.budget_cents > 0 && l.spent_cents > l.budget_cents).collect();
    let warning = (!over.is_empty()).then(|| {
        let names: Vec<&str> = over.iter().map(|l| l.category.as_str()).collect();
        Alert::new("Limite estourado", format!("Passaram do limite: {}.", names.join(", ")))
            .tone(Tone::Warning)
    });

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
                        Text::new(line.category.clone()).style(TextStyle::Headline),
                        Badge::new(status).tone(tone),
                        Spacer::new(),
                        Text::new(if line.budget_cents > 0 {
                            format!("{} de {}", format_brl(line.spent_cents), format_brl(line.budget_cents))
                        } else {
                            format_brl(line.spent_cents)
                        })
                        .style(TextStyle::Callout),
                        Button::new("Definir limite")
                            .accessible_name(format!("Definir limite de {name}"))
                            .size(ButtonSize::Small)
                            .style(ButtonStyle::Ghost)
                            .on_click(move || {
                                limit.set(if cents > 0 { format_input(cents) } else { String::new() });
                                editing.set(Some((id, open_name.clone())));
                            }),
                    ))
                    .spacing(10.0)
                    .align(Align::Center),
                    ProgressBar::new(ratio.min(1.0)).tone(tone),
                ))
                .spacing(10.0)
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

    ScrollView::new((VStack::new((
        heading(
            "Orçamentos",
            format!("Gasto {} de um limite de {}", format_brl(spent_total), format_brl(limit_total)),
        ),
        month_nav(&state.month, store.today),
        warning,
        VStack::new(rows).spacing(12.0).align(Align::Stretch),
        dialog,
    ))
    .spacing(16.0)
    .align(Align::Stretch),))
    .padding(24.0)
}
