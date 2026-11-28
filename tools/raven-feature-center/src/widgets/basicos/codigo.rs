//! InputOtp: separate boxes for a one-time code, digits only, that tell when the code is
//! complete and lock while it is verified, keeping what was typed.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "InputOtpDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Código de verificação";

/// Every public feature of InputOtp on one page.
#[derive(Debug, Default)]
pub struct InputOtpPage;

impl View for InputOtpPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let code = cx.use_state(String::new);
        let verifying = cx.use_state(|| false);
        let finished = cx.use_state(|| 0_u32);
        let (lock, count) = (verifying.clone(), finished.clone());
        let unlock = verifying.clone();
        let status = if verifying.get() {
            format!("Verificando {}…", code.get())
        } else {
            let typed = code.get();
            format!("Digitado: {}", if typed.is_empty() { "nada" } else { typed.as_str() })
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma caixa por dígito, só números. Ao completar, o código vai para a verificação e o campo trava até a resposta, sem perder o que foi digitado.")
                    .foreground(Semantic::InkMuted),
                Text::new("Conectar o serviço").style(TextStyle::Headline),
                InputOtp::new(code.binding(), 6).disabled(verifying.get()).on_complete(move |_| {
                    count.update(|n| *n += 1);
                    lock.set(true);
                }),
                Text::new(status),
                Text::new(format!("Conclusões: {}", finished.get())).foreground(Semantic::InkMuted),
                Button::new("Resposta do servidor")
                    .style(ButtonStyle::Secondary)
                    .disabled(!verifying.get())
                    .on_click(move || unlock.set(false)),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(720.0)
            .align(Align::Start),
        )
    }
}
