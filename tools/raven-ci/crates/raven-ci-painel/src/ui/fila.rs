//! The queue and the executors (P-03, #887; page T3): where each build ran and runs now, on a
//! read-only `Scheduler` with a row per executor, the builds waiting with Subir na fila, and the
//! executors in a grid. The agents of Fase 3 come in as more rows.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, jobs, nada};
use crate::texto;

/// The stretch of time shown: the last two hours and a little after now.
const BEFORE_MIN: i64 = 120;
const AFTER_MIN: i64 = 15;

/// The page of the queue and the executors.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let occupancy = s.occupancy.get();
    let now_ms = occupancy["now_ms"].as_i64().unwrap_or(0);
    let now = now_ms.div_euclid(60_000);
    let executors: Vec<Json> = occupancy["executors"].as_array().cloned().unwrap_or_default();
    let builds: Vec<Json> = occupancy["builds"].as_array().cloned().unwrap_or_default();
    let index_of = |worker: &str| executors.iter().position(|e| e["name"] == worker);

    let resources: Vec<Resource> = executors
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let busy = e["busy"].as_u64().unwrap_or(0);
            Resource::new(i as u64 + 1, e["name"].as_str().unwrap_or_default()).subtitle(if busy > 0 {
                "rodando agora"
            } else {
                "local · livre"
            })
        })
        .collect();
    let mut opened = Vec::new();
    let events: Vec<SchedulerEvent> = builds
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let resource = index_of(b["worker"].as_str()?)? as u64 + 1;
            let start = b["start_ms"].as_i64()?.div_euclid(60_000);
            let end = b["end_ms"].as_i64().map_or(now, |e| e.div_euclid(60_000)).max(start + 1);
            let status = b["status"].as_str().unwrap_or_default();
            let title = format!(
                "{} #{}{}",
                b["job"].as_str().unwrap_or_default(),
                b["number"].as_u64().unwrap_or(0),
                if status == "running" { " · rodando" } else { "" }
            );
            opened.push((
                i as u64 + 1,
                b["job"].as_str().unwrap_or_default().to_owned(),
                b["number"].as_u64().unwrap_or(0),
            ));
            Some(
                SchedulerEvent::new(
                    i as u64 + 1,
                    resource,
                    TimePoint::from_minutes(start),
                    TimePoint::from_minutes(end),
                    title,
                )
                .tone(texto::status_tone(status))
                .done(status != "running")
                .locked(true),
            )
        })
        .collect();
    // A click on a build of the line of time opens it, as Enter does.
    if let Some(id) = s.selected_event.get() {
        s.selected_event.set(None);
        if let Some((_, job, number)) = opened.iter().find(|(e, _, _)| *e == id) {
            s.go(Pagina::Build(job.clone(), *number));
            return super::build::page(s, job, *number);
        }
    }
    let axis = State::new(TimeAxis {
        start: TimePoint::from_minutes(now - BEFORE_MIN),
        end: TimePoint::from_minutes(now + AFTER_MIN),
        scale: TimeScale::Hours,
        tick_width: 360.0,
    });
    let open = {
        let s = s.clone();
        move |id: u64| {
            if let Some((_, job, number)) = opened.iter().find(|(e, _, _)| *e == id) {
                s.go(Pagina::Build(job.clone(), *number));
            }
        }
    };
    let timeline = Scheduler::new(axis.binding())
        .resources(resources)
        .events(events)
        .now(TimePoint::from_minutes(now))
        .read_only(true)
        .resource_width(200.0)
        .selection(s.selected_event.binding())
        .on_open(open);

    let waiting: Vec<&Json> = builds.iter().filter(|b| b["status"] == "queued").collect();
    let queue: AnyView = if waiting.is_empty() {
        Box::new(Text::new("Ninguém na fila.").foreground(Semantic::InkMuted))
    } else {
        let rows: Vec<AnyView> = waiting.iter().map(|b| queued_row(s, b, now_ms)).collect();
        Box::new(VStack::new(rows).spacing(8.0).align(Align::Stretch))
    };

    let rows: Vec<Vec<ravenrs::prelude::Value>> = executors
        .iter()
        .map(|e| {
            let busy = e["busy"].as_u64().unwrap_or(0);
            vec![
                e["name"].as_str().unwrap_or_default().to_owned().into(),
                "Executor do servidor · mesma máquina".to_owned().into(),
                format!("{busy} de 1").into(),
                (if busy > 0 { "running" } else { "idle" }).to_owned().into(),
            ]
        })
        .collect();
    let grid = DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Executor").width(180.0),
            GridColumn::text("Como sobe").fill(),
            GridColumn::text("Ocupado").width(110.0),
            GridColumn::text("Estado").width(140.0).template(|value| -> AnyView {
                if value.key_text() == "running" {
                    jobs::status_badge("running")
                } else {
                    Box::new(Badge::new("Livre").tone(Tone::Success).icon(IconName::Check))
                }
            }),
        ],
    )
    .toolbar(false)
    .height(200.0);

    Box::new(
        VStack::new((
            PageHeader::new("Fila e agentes").description(format!(
                "Onde cada build rodou nas últimas 2 horas, por executor. Horário do servidor (UTC), agora {}.",
                texto::clock(now_ms)
            )),
            Card::new(timeline).padding(0.0),
            Text::new(format!("Na fila · {}", waiting.len())).style(TextStyle::Headline),
            queue,
            Text::new("Executores").style(TextStyle::Headline),
            grid,
            Alert::message("Os agentes (local como outro usuário, remoto, contêiner e pod) entram aqui como mais linhas na Fase 3.")
                .tone(Tone::Neutral),
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// A build waiting: what it is, since when, and Subir na fila.
fn queued_row(s: &Rc<S>, build: &Json, now_ms: i64) -> AnyView {
    let job = build["job"].as_str().unwrap_or_default().to_owned();
    let number = build["number"].as_u64().unwrap_or(0);
    let priority = build["priority"].as_i64().unwrap_or(0);
    let since = build["created_ms"].as_i64().map(|t| texto::ago(t, now_ms)).unwrap_or_default();
    let up: AnyView = if s.runs() {
        let (s, job) = (s.clone(), job.clone());
        Box::new(Button::new("Subir na fila").icon(IconName::ArrowUp).style(ButtonStyle::Secondary).on_click(
            move || {
                let body = json!({ "job": job, "number": number, "priority": priority + 1 });
                s.send("/api/ci/priority", &body, |s, status, body| {
                    if status == 200 {
                        s.message.set("Prioridade subiu.".to_owned());
                        s.go(Pagina::Fila);
                    } else {
                        s.message.set(body["message"].as_str().unwrap_or("não subiu").to_owned());
                    }
                });
            },
        ))
    } else {
        nada()
    };
    Box::new(
        Card::new(
            HStack::new((
                Icon::new(IconName::Clock).color(Semantic::Warning),
                Text::new(format!("{job} #{number}")).style(TextStyle::Headline),
                Text::new(format!("pedido {since} · prioridade {priority}")).foreground(Semantic::InkMuted),
                Spacer::new(),
                up,
            ))
            .spacing(12.0)
            .align(Align::Center),
        )
        .padding(12.0),
    )
}
