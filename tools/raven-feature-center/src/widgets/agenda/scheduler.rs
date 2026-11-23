//! Scheduler: a timeline with a row per resource, every public feature on one page. The rows,
//! their groups, second lines and one that takes nothing now; events in tones, done, locked or
//! fixed in size; stretches marked behind them; now; the toolbar, the compact look with its legend
//! and the title of the column; the card, the avatars and the rows that fill the height; what an
//! event says under its title; the menus of an event and of an empty place; and the events of
//! moving, drawing, opening, deleting and the rule of where an event may go, each on a line.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "SchedulerDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Agenda por recurso";

/// The day the page shows.
fn day() -> Date {
    Date::new(2026, 11, 22).unwrap_or_else(|| unreachable!("a real date"))
}

/// A moment of that day.
fn at(hour: u32, minute: u32) -> TimePoint {
    TimePoint::at(day(), hour, minute)
}

/// Every public feature of Scheduler on one page.
#[derive(Debug, Default)]
pub struct SchedulerPage;

impl View for SchedulerPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let axis = cx.use_state(|| TimeAxis::quarter_hours(at(14, 0), 2));
        let selected: State<Option<u64>> = cx.use_state(|| None);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_owned());
        let compact = cx.use_state(|| true);
        let card = cx.use_state(|| false);
        let avatars = cx.use_state(|| false);
        let fill = cx.use_state(|| false);
        let locked = cx.use_state(|| false);
        let toolbar = cx.use_state(|| true);
        let narrow = cx.use_state(|| false);

        let resources = vec![
            Resource::new(1, "oficina-1").subtitle("local").group("Servidor"),
            Resource::new(2, "docker").subtitle("efêmero").group("Servidor"),
            Resource::new(3, "k8s").subtitle("pods, até 4").group("Nuvem"),
            Resource::new(4, "vps-backup")
                .subtitle("remoto")
                .group("Nuvem")
                .unavailable("Desconectado há 3 h · o build que estava nele foi retomado no oficina-1"),
        ];
        let events = vec![
            SchedulerEvent::new(1, 1, at(14, 5), at(14, 22), "site #56").tone(Tone::Success).done(true),
            SchedulerEvent::new(2, 1, at(14, 48), at(15, 43), "ravenrs PR-803 #42 · rodando")
                .tone(Tone::Accent),
            SchedulerEvent::new(3, 2, at(14, 27), at(14, 48), "loja-web #118")
                .tone(Tone::Success)
                .locked(true),
            SchedulerEvent::new(4, 3, at(14, 0), at(14, 36), "api-pedidos #31 · falhou").tone(Tone::Danger),
            SchedulerEvent::new(5, 3, at(14, 40), at(14, 46), "#12").tone(Tone::Warning).resizable(false),
        ];
        let window = SchedulerRange { start: at(15, 0), end: at(15, 30), tone: Tone::Warning };
        let tell = |said: &State<String>, text: String| said.set(text);
        let (moved, drawn, opened, deleted) = (said.clone(), said.clone(), said.clone(), said.clone());
        let detail_of = |id: u64| match id {
            2 => Some("estágio Testes · 812 de 1.360".to_owned()),
            4 => Some("falhou no estágio Testes".to_owned()),
            _ => None,
        };
        let menu_said = said.clone();
        let lane_said = said.clone();
        let scheduler = Scheduler::new(axis.binding())
            .resources(resources)
            .events(events)
            .ranges([window])
            .now(at(15, 43))
            .selection(selected.binding())
            .resource_width(if narrow.get() { 150.0 } else { 200.0 })
            .toolbar(toolbar.get())
            .compact(compact.get())
            .legend("A linha vertical é agora")
            .resources_title("AGENTES")
            .card(card.get())
            .avatars(avatars.get())
            .fill_rows(fill.get())
            .read_only(locked.get())
            .event_detail(detail_of)
            .event_menu(move |id| {
                let said = menu_said.clone();
                vec![
                    MenuItem::new("Abrir o build")
                        .on_select(move || said.set(format!("Menu: abrir o evento {id}"))),
                ]
            })
            .lane_menu(move |change| {
                let said = lane_said.clone();
                vec![MenuItem::new("Reservar aqui").on_select(move || {
                    said.set(format!("Menu: reservar no recurso {}", change.resource));
                })]
            })
            .on_change(move |change| {
                tell(&moved, format!("Movido: evento {:?} para o recurso {}", change.id, change.resource))
            })
            .on_create(move |change| {
                tell(&drawn, format!("Desenhado: um evento no recurso {}", change.resource))
            })
            .on_open(move |id| tell(&opened, format!("Aberto: evento {id}")))
            .on_delete(move |id| tell(&deleted, format!("Apagado: evento {id}")))
            // Nothing goes to the agent that is away.
            .can_drop(|change| change.resource != 4);
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Uma linha por recurso nas últimas duas horas, em quartos de hora. O compacto põe o subtítulo ao lado \
                     do nome e só uma linha de horas; a linha hachurada é de um recurso que não pega nada agora.",
                )
                .foreground(Semantic::InkMuted),
                HStack::new((
                    Toggle::new(compact.binding()).label("Compacto"),
                    Toggle::new(toolbar.binding()).label("Barra"),
                    Toggle::new(card.binding()).label("Cartão"),
                    Toggle::new(avatars.binding()).label("Avatares"),
                    Toggle::new(fill.binding()).label("Linhas que preenchem"),
                    Toggle::new(locked.binding()).label("Só leitura"),
                    Toggle::new(narrow.binding()).label("Nomes estreitos"),
                ))
                .spacing(16.0)
                .wrap(),
                VStack::new((scheduler,)).height(360.0).align(Align::Stretch),
                Text::new(format!(
                    "Escolhido: {}",
                    selected.get().map_or_else(|| "nenhum".to_owned(), |id| format!("evento {id}"))
                )),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .align(Align::Stretch),
        )
    }
}
