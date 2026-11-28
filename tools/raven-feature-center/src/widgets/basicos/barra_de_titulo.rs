//! TitleBar: the bar an app draws on top of its window, with what comes before the title, the
//! actions, the buttons of the window when they have a handler, the menu of a right click, the
//! maximized and inactive looks, and the bar on top of a phone's screen.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "TitleBarDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Barra de título";

/// Every public feature of TitleBar on one page.
#[derive(Debug, Default)]
pub struct TitleBarPage;

impl View for TitleBarPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let maximized = cx.use_state(|| false);
        let active = cx.use_state(|| true);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_owned());
        let tell = |said: &State<String>, text: &'static str| {
            let said = said.clone();
            move || said.set(text.to_owned())
        };
        let (grow, menu_said) = (maximized.clone(), said.clone());
        let window = TitleBar::new("Relatório.pdf")
            .leading(Icon::new(IconName::File).size(16.0))
            .action(IconButton::new(IconName::Search).label("Buscar").size(ButtonSize::Mini))
            .maximized(maximized.get())
            .active(active.get())
            .on_minimize(tell(&said, "Minimizar"))
            .on_maximize(move || grow.update(|on| *on = !*on))
            .on_close(tell(&said, "Fechar"))
            .on_context_menu(move |at| menu_said.set(format!("Menu da janela em {:.0}, {:.0}", at.x, at.y)));
        let (menu_said, phone_said) = (said.clone(), said.clone());
        let phone = TitleBar::new("Raven CI")
            .leading(IconButton::new(IconName::Menu).label("Abrir o menu").on_click(move || {
                menu_said.set("Menu aberto".to_owned());
            }))
            .action(IconButton::new(IconName::Search).label("Buscar comandos").on_click(move || {
                phone_said.set("Busca aberta".to_owned());
            }))
            .mobile();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "A barra que o app desenha no alto da janela: o que vem antes do título, as ações e os botões da \
                     janela quando têm quem os atenda. No celular, 56 de altura e o título maior.",
                )
                .foreground(Semantic::InkMuted),
                Text::new("Na janela").style(TextStyle::Headline),
                window,
                HStack::new((
                    Toggle::new(maximized.binding()).label("Maximizada"),
                    Toggle::new(active.binding()).label("Com o teclado"),
                ))
                .spacing(16.0)
                .wrap(),
                Text::new("No celular").style(TextStyle::Headline),
                VStack::new((phone,)).max_width(390.0).align(Align::Stretch),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(960.0)
            .align(Align::Stretch),
        )
    }
}
