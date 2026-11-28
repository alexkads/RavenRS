//! Tooltip: a few words over a control, by the pointer or by the keyboard's focus, with the key
//! that does the same, a wait before it shows and a moment after it goes; Escape hides it.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "TooltipDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Dicas";

/// Every public feature of Tooltip on one page.
#[derive(Debug, Default)]
pub struct TooltipsPage;

impl View for TooltipsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let saved = cx.use_state(|| 0_u32);
        let save = saved.clone();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Passe o ponteiro ou chegue com Tab: a dica aparece. Esc esconde sem tirar o foco.",
                )
                .foreground(Semantic::InkMuted),
                HStack::new((
                    Tooltip::new(
                        Button::new("Salvar").on_click(move || save.update(|n| *n += 1)),
                        "Guarda o rascunho",
                    )
                    .shortcut("⌘S"),
                    Tooltip::new(Button::new("Devagar").style(ButtonStyle::Secondary), "Espera meio segundo")
                        .open_delay(0.5)
                        .close_delay(0.4),
                    Tooltip::new(Button::new("Enviar").disabled(true), "Preencha o endereço antes"),
                ))
                .spacing(12.0)
                .align(Align::Center),
                Text::new(format!("Salvo {} vezes", saved.get())),
            ))
            .spacing(16.0)
            .padding(48.0)
            .align(Align::Start),
        )
    }
}
