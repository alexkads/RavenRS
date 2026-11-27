//! MaskedField: text that follows a pattern, with slots of the field's own, slots that may stay
//! empty, the value with or without the pattern, and whether every slot that must be is filled.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "MaskedFieldDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Máscaras";

/// Every public feature of MaskedField on one page.
#[derive(Debug, Default)]
pub struct MasksPage;

impl View for MasksPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let phone = cx.use_state(String::new);
        let phone_done = cx.use_state(|| false);
        let cpf = cx.use_state(String::new);
        let color = cx.use_state(String::new);
        let plate = cx.use_state(|| "ABC1D23".to_owned());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("O texto segue um padrão: 0 é dígito, a é letra, * é letra ou dígito, e cada campo pode dar sentido próprio a outros caracteres.")
                    .foreground(Semantic::InkMuted),
                MaskedField::new(phone.binding(), "(00) 90000-0000")
                    .label("Celular")
                    .optional_rule('9', |c| c.is_ascii_digit())
                    .complete(phone_done.binding())
                    .width(220.0),
                Text::new(format!(
                    "Celular: {} ({})",
                    if phone.get().is_empty() { "vazio".to_owned() } else { phone.get() },
                    if phone_done.get() { "completo" } else { "incompleto" }
                )),
                MaskedField::new(cpf.binding(), "000.000.000-00").label("CPF").keep_literals().width(220.0),
                Text::new(format!("CPF guardado com a pontuação: {}", cpf.get())),
                MaskedField::new(color.binding(), "\\#hhhhhh")
                    .label("Cor")
                    .rule('h', |c| c.is_ascii_hexdigit())
                    .placeholder("#RRGGBB")
                    .width(160.0),
                Text::new(format!("Cor: {}", color.get())),
                MaskedField::new(plate.binding(), "aaa-0*00").label("Placa travada").read_only(true),
                MaskedField::new(plate.binding(), "aaa-0*00").label("Placa desligada").disabled(true),
            ))
            .spacing(10.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
