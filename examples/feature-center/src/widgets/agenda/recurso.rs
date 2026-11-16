//! `Scheduler`, every feature: the plan of a team and its rooms as a timeline by resource, with
//! groups that collapse, bars that move along the time and between rows, stretch by their ends
//! and are drawn on an empty stretch; a weekly meeting that repeats; holidays shaded, the line of
//! now, a second line on the bars, menus on bars and on empty stretches, places refused, and the
//! look of a card or a plain one, read-only or not. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "SchedulerDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Agenda por recurso";

/// The scales of the axis, in the order of the picker.
pub const SCALES: [&str; 4] = ["Horas", "Dias", "Semanas", "Meses"];

/// The room under maintenance, where nothing can be put.
pub const CLOSED_ROOM: u64 = 7;

/// Today, fixed so the page reads the same every time.
pub fn today() -> Date {
    Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"))
}

fn first() -> Date {
    today().first_of_month()
}

fn day(n: i32) -> Date {
    first().add_days(n - 1)
}

fn resources() -> Vec<Resource> {
    vec![
        Resource::new(1, "Ana Souza").subtitle("Técnica").group("Equipe de campo"),
        Resource::new(2, "Bruno Lima").subtitle("Técnico").group("Equipe de campo"),
        Resource::new(3, "Caio Reis").subtitle("Suporte").group("Equipe de campo"),
        Resource::new(4, "Davi Melo").subtitle("Vendas").group("Escritório"),
        Resource::new(5, "Eva Rocha").subtitle("Financeiro").group("Escritório"),
        Resource::new(6, "Sala 1").subtitle("8 lugares").group("Salas"),
        Resource::new(CLOSED_ROOM, "Sala 2").subtitle("Em manutenção").group("Salas"),
    ]
}

/// The plan the page starts with, the weekly meeting expanded from its rule.
pub fn plan() -> Vec<SchedulerEvent> {
    let at = |d: i32, h: u32| TimePoint::at(day(d), h, 0);
    let mut events = vec![
        SchedulerEvent::new(1, 1, at(3, 0), at(6, 0), "Instalação").tone(Tone::Accent),
        SchedulerEvent::new(2, 2, at(9, 0), at(13, 0), "Treinamento").tone(Tone::Success),
        SchedulerEvent::new(3, 3, at(4, 0), at(5, 0), "Visita técnica").tone(Tone::Warning).done(true),
        SchedulerEvent::new(4, 4, at(17, 0), at(20, 0), "Feira de negócios").tone(Tone::Accent),
        SchedulerEvent::new(5, 5, at(23, 0), at(26, 0), "Fechamento").tone(Tone::Danger).locked(true),
        SchedulerEvent::new(6, 1, at(18, 0), at(18, 0), "Entrega do projeto").tone(Tone::Success),
        SchedulerEvent::new(7, 6, at(10, 0), at(12, 0), "Workshop").resizable(false),
        SchedulerEvent::new(8, 2, at(10, 0), at(12, 0), "Suporte remoto").tone(Tone::Warning),
    ];
    // Every Monday, five times: the app expands the rule into the days it falls on.
    let rule = RecurrenceRule::new(Frequency::Weekly).on_weekdays([1]).end(RecurrenceEnd::After(5));
    for (n, monday) in rule.occurrences(day(2), day(1), day(30)).into_iter().enumerate() {
        let start = TimePoint::at(monday, 9, 0);
        events.push(
            SchedulerEvent::new(100 + n as u64, 6, start, start.plus_minutes(60 * 6), "Reunião semanal")
                .tone(Tone::Neutral),
        );
    }
    events
}

/// The holidays of the month, shaded across all rows.
fn holidays() -> Vec<SchedulerRange> {
    [2, 15, 20]
        .into_iter()
        .map(|d| SchedulerRange {
            start: TimePoint::from_date(day(d)),
            end: TimePoint::from_date(day(d + 1)),
            tone: Tone::Danger,
        })
        .collect()
}

fn axis_for(scale: usize) -> TimeAxis {
    match scale {
        0 => TimeAxis::hours(today()),
        2 => TimeAxis::weeks(first(), 9),
        3 => TimeAxis::months(first(), 6),
        _ => TimeAxis::days(first(), 30),
    }
}

fn when(time: TimePoint) -> String {
    let (h, m) = time.hour_minute();
    let date = time.date();
    format!("{:02}/{:02} {h:02}:{m:02}", date.day(), date.month())
}

/// The page.
#[derive(Debug, Default)]
pub struct SchedulerPage;

impl View for SchedulerPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let scale = cx.use_state(|| 1_usize);
        let axis = cx.use_state(|| axis_for(1));
        let events = cx.use_state(plan);
        let selected = cx.use_state(|| None::<u64>);
        let next = cx.use_state(|| 200_u64);
        let said =
            cx.use_state(|| "Arraste uma barra, puxe uma ponta ou desenhe num trecho vazio.".to_string());
        let card = cx.use_state(|| true);
        let avatars = cx.use_state(|| true);
        let read_only = cx.use_state(|| false);
        let toolbar = cx.use_state(|| true);
        let fill = cx.use_state(|| false);

        // The picker of the scale puts the axis back on the month.
        let (read, write, to_axis) = (scale.clone(), scale.clone(), axis.clone());
        let scale_binding = Binding::new(
            move || read.get(),
            move |i: usize| {
                write.set(i);
                to_axis.set(axis_for(i));
            },
        );
        let title_of = {
            let all = events.clone();
            move |id: u64| all.peek().iter().find(|e| e.id == id).map(|e| e.title.clone()).unwrap_or_default()
        };
        let (moved, change_line, change_title) = (events.clone(), said.clone(), title_of.clone());
        let (drawn, create_line, counter) = (events.clone(), said.clone(), next.clone());
        let (open_line, open_title) = (said.clone(), title_of.clone());
        let (deleted, delete_line, delete_title) = (events.clone(), said.clone(), title_of.clone());
        let refused = said.clone();
        let (menu_events, menu_line, menu_title) = (events.clone(), said.clone(), title_of.clone());
        let (lane_events, lane_line, lane_counter) = (events.clone(), said.clone(), next.clone());
        let amounts = events.get();
        let scheduler = Scheduler::new(axis.binding())
            .resources(resources())
            .events(events.get())
            .ranges(holidays())
            .now(TimePoint::at(today(), 10, 30))
            .selection(selected.binding())
            .resource_width(if narrow { 120.0 } else { 200.0 })
            .card(card.get())
            .avatars(avatars.get())
            .read_only(read_only.get())
            .toolbar(toolbar.get())
            .fill_rows(fill.get())
            .event_detail(move |id| {
                amounts.iter().find(|e| e.id == id && e.resource <= 2).map(|e| {
                    let days = (e.end.date().to_days() - e.start.date().to_days()).max(1);
                    format!("R$ {},00", days * 450)
                })
            })
            .on_change(move |change| {
                moved.update(|all| {
                    if let Some(event) = all.iter_mut().find(|e| Some(e.id) == change.id) {
                        event.resource = change.resource;
                        event.start = change.start;
                        event.end = change.end;
                    }
                });
                let title = change.id.map(&change_title).unwrap_or_default();
                change_line.set(format!("{title} foi para {} até {}", when(change.start), when(change.end)));
            })
            .on_create(move |change| {
                let id = counter.get();
                counter.set(id + 1);
                drawn.update(|all| {
                    all.push(SchedulerEvent::new(
                        id,
                        change.resource,
                        change.start,
                        change.end,
                        "Novo evento",
                    ))
                });
                create_line.set(format!("Criou um evento em {}", when(change.start)));
            })
            .on_open(move |id| open_line.set(format!("Abriu {}", open_title(id))))
            .on_delete(move |id| {
                let title = delete_title(id);
                deleted.update(|all| all.retain(|e| e.id != id));
                delete_line.set(format!("Excluiu {title}"));
            })
            .can_drop(move |change| {
                let allowed = change.resource != CLOSED_ROOM;
                if !allowed {
                    refused.set_if_changed("A Sala 2 está em manutenção: nada pode ir para lá".into());
                }
                allowed
            })
            .event_menu(move |id| {
                let (keep, line, title) = (menu_events.clone(), menu_line.clone(), menu_title(id));
                let (done_line, done_title, done_keep) =
                    (menu_line.clone(), title.clone(), menu_events.clone());
                vec![
                    MenuItem::new("Marcar como feito").icon(IconName::Check).on_select(move || {
                        done_keep.update(|all| {
                            if let Some(event) = all.iter_mut().find(|e| e.id == id) {
                                event.done = true;
                            }
                        });
                        done_line.set(format!("{done_title} feito"));
                    }),
                    MenuItem::new("Excluir").icon(IconName::Trash2).destructive().on_select(move || {
                        keep.update(|all| all.retain(|e| e.id != id));
                        line.set(format!("Excluiu {title}"));
                    }),
                ]
            })
            .lane_menu(move |place| {
                let (keep, line, counter) = (lane_events.clone(), lane_line.clone(), lane_counter.clone());
                vec![MenuItem::new("Novo evento aqui").icon(IconName::Plus).on_select(move || {
                    let id = counter.get();
                    counter.set(id + 1);
                    keep.update(|all| {
                        all.push(SchedulerEvent::new(
                            id,
                            place.resource,
                            place.start,
                            place.start.plus_days(1),
                            "Novo evento",
                        ))
                    });
                    line.set(format!("Criou pelo menu um evento em {}", when(place.start)));
                })]
            });
        let picker: AnyView = if narrow {
            Box::new(Select::new(scale_binding, SCALES).label("Escala"))
        } else {
            Box::new(SegmentedControl::new(scale_binding, SCALES))
        };
        let chosen = selected.get().map(&title_of);
        let restore = events.clone();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                HStack::new((
                    Toggle::new(card.binding()).label("Visual de cartão"),
                    Toggle::new(avatars.binding()).label("Iniciais das pessoas"),
                    Toggle::new(fill.binding()).label("Linhas ocupam a altura"),
                    Toggle::new(toolbar.binding()).label("Barra de navegação"),
                    Toggle::new(read_only.binding()).label("Somente leitura"),
                ))
                .spacing(16.0)
                .wrap(),
                HStack::new((
                    picker,
                    Button::new("Desfazer tudo")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || restore.set(plan())),
                ))
                .spacing(12.0)
                .wrap(),
                VStack::new((scheduler,)).height(if narrow { 420.0 } else { 520.0 }).align(Align::Stretch),
                Text::new(format!(
                    "{} eventos · selecionado: {}",
                    events.get().len(),
                    chosen.unwrap_or_else(|| "nenhum".to_string())
                )),
                Text::new("Feriados em vermelho; a Sala 2 recusa eventos; o Fechamento está travado.")
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
