//! `Wizard` and `ConfirmDialog`, every feature (#579): a report made in steps, one of which refuses
//! to be left without a name, that asks before it is published or thrown away, and an account
//! that is deleted only after its word is typed. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "StepsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Passo a passo e confirmação";

/// The titles of the steps.
pub const STEPS: [&str; 3] = ["Dados", "Título", "Revisão"];
/// The last button of the wizard.
pub const FINISH: &str = "Publicar";
/// The word that has to be typed to delete the account.
pub const WORD: &str = "APAGAR";

/// The page: the wizard, the account and a line with the last answer.
#[derive(Debug, Default)]
pub struct StepsPage;

impl View for StepsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Siga os passos do relatório.".to_string());
        let step = cx.use_state(|| 0_usize);
        let title = cx.use_state(String::new);
        let source = cx.use_state(|| 0_usize);
        // One dialog for the page, kept between builds; any button asks through it.
        let confirm = cx.use_state(ConfirmDialog::new).get();

        let named = title.clone();
        let steps = vec![
            WizardStep::new(
                STEPS[0],
                VStack::new((
                    Text::new("De onde vêm os dados do relatório?"),
                    Select::new(source.binding(), ["Vendas", "Estoque", "Clientes"]).label("Origem"),
                ))
                .spacing(8.0),
            ),
            WizardStep::new(
                STEPS[1],
                FormField::new(
                    "Título do relatório",
                    TextField::new(title.binding()).placeholder("Vendas do mês"),
                )
                .required(),
            )
            .check(move || {
                if named.with(|t| t.trim().is_empty()) {
                    Err("Dê um título ao relatório".to_string())
                } else {
                    Ok(())
                }
            })
            .failed(title.with(|t| t.trim().is_empty()) && step.get() > 1),
            WizardStep::new(
                STEPS[2],
                Text::new(format!(
                    "\"{}\" com dados de {}",
                    title.get(),
                    ["Vendas", "Estoque", "Clientes"][source.get().min(2)]
                )),
            ),
        ];
        let (publish, publishing, back) = (confirm.clone(), said.clone(), step.clone());
        let (discard, discarding, restart, blank) =
            (confirm.clone(), said.clone(), step.clone(), title.clone());
        let wizard = Wizard::new(step.binding(), steps)
            .finish_label(FINISH)
            .on_finish(move || {
                let (line, step) = (publishing.clone(), back.clone());
                publish.ask_plainly(
                    "Publicar o relatório?",
                    "Ele pode ser despublicado depois.",
                    "Publicar agora",
                    move || {
                        line.set("Relatório publicado".to_string());
                        step.set(0);
                    },
                );
            })
            .on_cancel(move || {
                let (line, step, title) = (discarding.clone(), restart.clone(), blank.clone());
                discard.ask(
                    "Descartar o rascunho?",
                    "O que foi preenchido se perde.",
                    "Descartar",
                    move || {
                        line.set("Rascunho descartado".to_string());
                        title.set(String::new());
                        step.set(0);
                    },
                );
            });

        let (delete, deleting) = (confirm.clone(), said.clone());
        let account = VStack::new((
            Text::new("Conta").style(TextStyle::Headline),
            Text::new(format!("Apagar a conta pede que se digite {WORD}.")).foreground(Semantic::InkMuted),
            Button::new("Apagar a conta").style(ButtonStyle::Danger).on_click(move || {
                let line = deleting.clone();
                delete.ask_typing(
                    "Apagar a conta?",
                    format!("Isto não se desfaz. Digite {WORD} para confirmar."),
                    "Apagar para sempre",
                    WORD,
                    move || line.set("Conta apagada".to_string()),
                );
            }),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((
                Text::new("Novo relatório").style(TextStyle::Headline),
                wizard,
                account,
                Text::new(said.get()),
                confirm,
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
