//! `SplitPanes`, `Splitter`, `SplitHandle` and `Collapse`, every feature: three panes with limits
//! and one that hides, two panes side by side or stacked, a lone bar that sizes a panel, and
//! content that opens and closes. A line says what was moved or saved.

use ravenrs::layout::Axis;
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "SplitsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Divisões";

/// The screen-reader name of the lone bar.
pub const HANDLE: &str = "Altura do console";

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn pane(title: &str, text: &str) -> VStack<(Text, Text)> {
    VStack::new((
        Text::new(title.to_owned()).style(TextStyle::Headline),
        Text::new(text.to_owned()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
    .padding(12.0)
    .align(Align::Start)
}

fn size_name(size: Option<f32>) -> String {
    size.map_or_else(|| String::from("o resto"), |size| format!("{size:.0}"))
}

/// The page: panes, the splitter, the bar and what collapses.
#[derive(Debug, Default)]
pub struct SplitsPage;

impl View for SplitsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let sizes = cx.use_state(|| vec![Some(160.0_f32), None, Some(160.0)]);
        let activity = cx.use_state(|| true);
        let console = cx.use_state(|| 96.0_f32);
        let details = cx.use_state(|| false);
        let notes = cx.use_state(|| false);
        let event = cx.use_state(|| String::from("Nada movido ainda"));

        // Three panes kept in a state, the middle one taking what is left; the last one can hide.
        let said_end = event.clone();
        let panes = SplitPanes::horizontal()
            .pane(SplitPane::new(pane("Sessões", "De 120 a 240")).size(160.0).range(120.0, 240.0))
            .pane(SplitPane::new(pane("Recibo", "O que sobra")).range(160.0, f32::MAX))
            .pane(
                SplitPane::new(pane("Atividade", "Pode sumir"))
                    .size(160.0)
                    .range(120.0, 240.0)
                    .visible(activity.get() && !narrow),
            )
            .sizes(sizes.binding())
            .on_end(move || said_end.set(String::from("Larguras salvas")))
            .height(if narrow { 140.0 } else { 180.0 });
        let widths = sizes.get().into_iter().map(size_name).collect::<Vec<_>>().join(" · ");
        let (flip, said_flip) = (activity.clone(), event.clone());
        let shown = activity.get();
        let panes_bar = HStack::new((
            Text::new(format!("Larguras: {widths}")),
            Spacer::new(),
            Button::new(if shown { "Esconder a atividade" } else { "Mostrar a atividade" })
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    flip.update(|on| *on = !*on);
                    said_flip.set(String::from(if shown {
                        "Atividade escondida"
                    } else {
                        "Atividade de volta"
                    }));
                }),
        ))
        .align(Align::Center);
        let stacked = SplitPanes::vertical()
            .pane(SplitPane::new(pane("Consulta", "Em cima")).size(70.0).range(40.0, 120.0))
            .pane(SplitPane::new(pane("Resultado", "Embaixo")))
            .height(180.0);

        // Two panes: a share for the first, kept between limits.
        let splitters = Grid::new(
            if narrow { 1 } else { 2 },
            (
                Splitter::horizontal(pane("Explorador", "32% no início"), pane("Editor", "O resto"))
                    .ratio(0.32)
                    .limits(0.2, 0.6)
                    .height(150.0),
                Splitter::vertical(pane("Código", "Em cima"), pane("Terminal", "Embaixo"))
                    .ratio(0.6)
                    .height(150.0),
            ),
        )
        .spacing(12.0);

        // A lone bar: the panel below is yours to size, the bar only changes the number.
        let said_handle = event.clone();
        let height = console.get();
        let handle = VStack::new((
            Card::new(pane("Editor", "Arraste a barra ou use as setas")).no_shadow(),
            SplitHandle::new(Axis::Vertical, console.binding())
                .range(48.0, 200.0)
                .inverted()
                .label(HANDLE)
                .on_end(move || said_handle.set(String::from("Altura do console salva"))),
            Card::new(Text::new(format!("Console com {height:.0} de altura")))
                .no_shadow()
                .fill(Semantic::Control)
                .padding(12.0)
                .frame(None, Some(height)),
        ))
        .spacing(0.0)
        .align(Align::Stretch);

        // What opens and closes, from a view and from one already boxed.
        let (open_details, said_details) = (details.clone(), event.clone());
        let (open_notes, said_notes) = (notes.clone(), event.clone());
        let opened = details.get();
        let collapse = VStack::new((
            Button::new(if opened { "Esconder os detalhes" } else { "Mostrar os detalhes" })
                .style(ButtonStyle::Secondary)
                .on_click(move || {
                    open_details.update(|open| *open = !*open);
                    said_details.set(String::from(if opened {
                        "Detalhes fechados"
                    } else {
                        "Detalhes abertos"
                    }));
                }),
            Collapse::new(
                opened,
                Card::new(Text::new("Entrega em 3 dias úteis, frete grátis acima de R$ 200.")).no_shadow(),
            ),
            Button::new("Notas da versão").style(ButtonStyle::Ghost).on_click(move || {
                open_notes.update(|open| *open = !*open);
                said_notes.set(String::from("Notas alternadas"));
            }),
            Collapse::boxed(
                notes.get(),
                Box::new(
                    Text::new("Versão 0.1: primeira página das divisões.").foreground(Semantic::InkMuted),
                ),
            ),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((
                VStack::new((heading("Painéis lado a lado"), panes, panes_bar)).spacing(8.0),
                VStack::new((heading("Painéis empilhados"), stacked)).spacing(8.0),
                VStack::new((heading("Duas partes"), splitters)).spacing(8.0),
                VStack::new((heading("Uma barra só"), handle)).spacing(8.0),
                VStack::new((heading("Abrir e fechar"), collapse)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
