//! Toggle: on and off, the compact size, a visible label or only a name for screen readers, and the
//! states that keep it from changing.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ToggleDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Chaves";

/// Every public feature of Toggle on one page.
#[derive(Debug, Default)]
pub struct TogglesPage;

impl View for TogglesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let alerts = cx.use_state(|| true);
        let digest = cx.use_state(|| false);
        let docker = cx.use_state(|| false);
        let locked = cx.use_state(|| true);
        let (alerts_now, digest_now, docker_now) = (alerts.get(), digest.get(), docker.get());
        let on_off = |on: bool| if on { "ligada" } else { "desligada" };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Para uma escolha que vale na hora, sem botão de salvar.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Com rótulo").style(TextStyle::Headline),
                    Toggle::new(alerts.binding()).label("Avisos no celular"),
                    Text::new(format!("Avisos no celular: {}", on_off(alerts_now))),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Compacta").style(TextStyle::Headline),
                    Toggle::new(digest.binding()).small().label("Resumo por e-mail"),
                    Text::new(format!("Resumo por e-mail: {}", on_off(digest_now))),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Só com nome acessível").style(TextStyle::Headline),
                    Text::new(
                        "Numa linha de grade, a linha já diz do que se trata: a chave não mostra texto, e o \
                         leitor de tela lê o nome dela.",
                    )
                    .foreground(Semantic::InkMuted),
                    HStack::new((
                        Text::new("docker"),
                        Toggle::new(docker.binding()).small().accessible_name("Ligar docker"),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                    Text::new(format!("Plugin docker: {}", on_off(docker_now))),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Estados").style(TextStyle::Headline),
                    Toggle::new(locked.binding()).label("Indisponível").disabled(true),
                    Toggle::new(locked.binding()).label("Só leitura").read_only(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
