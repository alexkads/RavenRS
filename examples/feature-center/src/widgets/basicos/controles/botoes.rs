//! `Button`, `ButtonGroup`, `IconButton`, `ToggleButton`, `ToggleGroup` and `Kbd`, every feature:
//! styles and sizes side by side, the states a button goes through, a toolbar of icons, filter
//! chips and the groups a text editor uses. A line says what the last click did.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::core::Key;
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ButtonsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Botões";

/// The periods of the button group, in order.
pub const PERIODS: [&str; 3] = ["Dia", "Semana", "Mês"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The page: styles, sizes, states, icons, groups and key caps, and a line with the last event.
#[derive(Debug, Default)]
pub struct ButtonsPage;

impl View for ButtonsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Clique num botão ou pressione F8.".to_string());
        let clicks = cx.use_state(|| 0_u32);
        let sending = cx.use_state(|| false);
        let bold = cx.use_state(|| false);
        let period = cx.use_state(|| 1_usize);
        let open_only = cx.use_state(|| true);
        let pinned = cx.use_state(|| false);
        let align = cx.use_state(|| 0_usize);
        let marks = cx.use_state(BTreeSet::<usize>::new);

        // F8 saves from anywhere on the page: `shortcut` only shows the key, this makes it work.
        let saved = said.clone();
        cx.shortcut(Key::Function(8), false, Rc::new(move || saved.set("Salvo pelo atalho F8".to_string())));

        let tell = |line: &State<String>, text: &'static str| {
            let line = line.clone();
            move || line.set(text.to_string())
        };

        let styles = HStack::new((
            Button::new("Principal").on_click(tell(&said, "Principal clicado")),
            Button::new("Secundário")
                .style(ButtonStyle::Secondary)
                .on_click(tell(&said, "Secundário clicado")),
            Button::new("Discreto").style(ButtonStyle::Ghost).on_click(tell(&said, "Discreto clicado")),
            Button::new("Excluir tudo")
                .style(ButtonStyle::Danger)
                .on_click(tell(&said, "Perigo: nada foi excluído")),
        ))
        .spacing(8.0)
        .wrap();

        let sizes = HStack::new((
            Button::new("Mínimo").size(ButtonSize::Mini).style(ButtonStyle::Secondary),
            Button::new("Pequeno").size(ButtonSize::Small).style(ButtonStyle::Secondary),
            Button::new("Médio").size(ButtonSize::Medium).style(ButtonStyle::Secondary),
            Button::new("Grande").size(ButtonSize::Large).style(ButtonStyle::Secondary),
        ))
        .spacing(8.0)
        .wrap()
        .align(Align::Center);

        let count = clicks.clone();
        let counted = said.clone();
        let (start, started) = (sending.clone(), said.clone());
        let (stop, stopped) = (sending.clone(), said.clone());
        let (flip, flipped) = (bold.clone(), said.clone());
        let states = HStack::new((
            Button::new(format!("Contar ({})", clicks.get())).icon(IconName::Plus).on_click(move || {
                count.update(|n| *n += 1);
                counted.set(format!("Cliques: {}", count.get()));
            }),
            Button::new("Enviar").icon(IconName::Send).loading(sending.get()).on_click(move || {
                start.set(true);
                started.set("Enviando…".to_string());
            }),
            Button::new("Concluir o envio").style(ButtonStyle::Secondary).disabled(!sending.get()).on_click(
                move || {
                    stop.set(false);
                    stopped.set("Envio concluído".to_string());
                },
            ),
            Button::new("Negrito").style(ButtonStyle::Secondary).selected(bold.get()).on_click(move || {
                flip.update(|b| *b = !*b);
                flipped.set(format!("Negrito {}", if flip.get() { "ligado" } else { "desligado" }));
            }),
            Button::new("Indisponível").disabled(true),
        ))
        .spacing(8.0)
        .wrap();

        let icons = HStack::new((
            Button::new("Salvar")
                .icon(IconName::Save)
                .shortcut("F8")
                .accessible_description("Guarda o documento")
                .on_click(tell(&said, "Salvo pelo botão")),
            Button::new("Mais opções")
                .style(ButtonStyle::Secondary)
                .trailing_icon(IconName::ChevronDown)
                .on_click(tell(&said, "Menu de opções aberto")),
            Button::new("")
                .icon(IconName::Trash2)
                .style(ButtonStyle::Ghost)
                .accessible_name("Apagar o rascunho")
                .on_click(tell(&said, "Rascunho apagado")),
        ))
        .spacing(8.0)
        .wrap();

        let (search, deleted, pin_line) = (said.clone(), said.clone(), said.clone());
        let pin_state = pinned.clone();
        let toolbar = HStack::new((
            IconButton::new(IconName::Search)
                .label("Procurar")
                .description("Procura no documento")
                .on_click(move || search.set("Procurar aberto".to_string())),
            IconButton::new(IconName::Pin).label("Fixar o painel").selected(pinned.get()).on_click(
                move || {
                    pin_state.update(|p| *p = !*p);
                    pin_line.set(format!("Painel {}", if pin_state.get() { "fixado" } else { "solto" }));
                },
            ),
            IconButton::new(IconName::Trash2)
                .label("Excluir a linha")
                .style(ButtonStyle::Danger)
                .on_click(move || deleted.set("Linha excluída".to_string())),
            IconButton::new(IconName::ZoomIn).label("Aproximar pouco").size(ButtonSize::Small),
            IconButton::new(IconName::Settings).label("Ajustes grandes").size(ButtonSize::Large),
            IconButton::new(IconName::Lock).label("Trancado").disabled(true),
        ))
        .spacing(4.0)
        .wrap()
        .align(Align::Center);

        let pick = |index: usize| {
            let (chosen, line) = (period.clone(), said.clone());
            let style = if period.get() == index { ButtonStyle::Primary } else { ButtonStyle::Secondary };
            Button::new(PERIODS[index]).style(style).on_click(move || {
                chosen.set(index);
                line.set(format!("Período: {}", PERIODS[index]));
            })
        };
        let group = ButtonGroup::new([pick(0), pick(1), pick(2)]).label("Período");

        let chips = HStack::new((
            ToggleButton::new(open_only.binding(), "Abertas").count(12).pill(),
            ToggleButton::new(pinned.binding(), "Fixado").icon(IconName::Pin),
        ))
        .spacing(8.0);

        let alignment = ToggleGroup::single(
            align.binding(),
            [
                (Some(IconName::AlignLeft), "À esquerda"),
                (Some(IconName::AlignCenter), "Ao centro"),
                (Some(IconName::AlignRight), "À direita"),
            ],
        );
        let format_group = ToggleGroup::multiple(
            marks.binding(),
            [(Some(IconName::Bold), "Negrito"), (Some(IconName::Italic), "Itálico"), (None, "Sublinhado")],
        );
        let alignments = ["à esquerda", "ao centro", "à direita"];
        let state = format!(
            "Filtro abertas: {} · alinhado {} · {} marcas",
            if open_only.get() { "sim" } else { "não" },
            alignments[align.get().min(2)],
            marks.get().len()
        );

        let keys = HStack::new((
            Text::new("Abra a paleta com"),
            Kbd::new("⌘K"),
            Text::new("ou pressione"),
            Kbd::new("Enter").inherit_color(),
            Text::new("para enviar"),
        ))
        .spacing(6.0)
        .wrap()
        .align(Align::Center);

        let wide = Button::new("Continuar").full_width().on_click(tell(&said, "Continuar clicado"));

        cx.child(
            &VStack::new((
                VStack::new((heading("Estilos"), styles, heading("Tamanhos"), sizes)).spacing(8.0),
                VStack::new((heading("Estados"), states, heading("Ícones e atalho"), icons)).spacing(8.0),
                VStack::new((heading("Barra de ícones"), toolbar)).spacing(8.0),
                VStack::new((heading("Grupos"), group, chips, alignment, format_group, Text::new(state)))
                    .spacing(8.0)
                    .align(Align::Start),
                VStack::new((heading("Teclas"), keys)).spacing(8.0),
                VStack::new((heading("Largura toda"), wide)).spacing(8.0).align(Align::Stretch),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
