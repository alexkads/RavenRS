//! Button: emphasis, sizes, content, states, accessibility and activation.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ButtonDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Botões";

/// Every public feature of Button on one page.
#[derive(Debug, Default)]
pub struct ButtonsPage;

impl View for ButtonsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let said = cx.use_state(|| "Nenhum botão acionado".to_owned());
        let count = cx.use_state(|| 0_u32);
        let selected = cx.use_state(|| false);
        let action = |label: &'static str| {
            let said = said.clone();
            Button::new(label).on_click(move || said.set(format!("Acionado: {label}")))
        };
        let counter = count.clone();
        let toggle = selected.clone();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Ênfase").style(TextStyle::Headline),
                HStack::new((
                    action("Salvar").style(ButtonStyle::Primary),
                    action("Revisar").style(ButtonStyle::Secondary),
                    action("Ver detalhes").style(ButtonStyle::Ghost),
                    action("Excluir").style(ButtonStyle::Danger),
                ))
                .spacing(8.0),
                Text::new("Tamanhos").style(TextStyle::Headline),
                HStack::new((
                    action("Pequeno").size(ButtonSize::Small),
                    action("Médio").size(ButtonSize::Medium),
                    action("Grande").size(ButtonSize::Large),
                    action("Mini").size(ButtonSize::Mini),
                ))
                .spacing(8.0),
                Text::new("Ícones e nome acessível").style(TextStyle::Headline),
                HStack::new((
                    action("Enviar").icon(IconName::Check),
                    action("Continuar").trailing_icon(IconName::ChevronRight),
                    Button::new("")
                        .icon(IconName::Search)
                        .accessible_name("Pesquisar documentos")
                        .accessible_description("Abre a pesquisa de documentos")
                        .on_click({
                            let said = said.clone();
                            move || said.set("Pesquisa aberta".to_owned())
                        }),
                ))
                .spacing(8.0),
                Text::new("Estados").style(TextStyle::Headline),
                HStack::new((
                    action("Indisponível").disabled(true),
                    action("Processando").loading(true),
                    Button::new("Fixar")
                        .selected(selected.get())
                        .on_click(move || toggle.update(|on| *on = !*on)),
                ))
                .spacing(8.0),
                Text::bound(&selected, |on| if *on { "Fixado".to_owned() } else { "Não fixado".to_owned() }),
                VStack::new((
                    Text::new("Largura e teclado").style(TextStyle::Headline),
                    action("Ocupar a largura").full_width(),
                    Button::new("Contar ativações")
                        .shortcut("Enter / Espaço")
                        .on_click(move || counter.update(|n| *n += 1)),
                    Text::bound(&count, |n| format!("Ativações: {n}")),
                    Text::new(said.get()),
                ))
                .spacing(12.0),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
