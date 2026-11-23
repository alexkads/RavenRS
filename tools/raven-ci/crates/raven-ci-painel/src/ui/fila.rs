//! The queue and the agents (P-03, #887; page T3, as #919 draws it): where each build ran in the
//! last two hours, on a read-only `Scheduler` in quarters of an hour with a row per executor of the
//! server, per agent connected and per template (an agent that went away hatched, saying when), the
//! builds waiting with why and what lets them go (Ligar o plugin, Subir na fila), and the agents in
//! a grid. "Adicionar agente" makes the token an agent comes in with.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, jobs, nada};
use crate::texto;

/// The stretch of time shown: the last two hours, ending a quarter after now.
const HOURS: u32 = 2;

/// A row of the line of time: an executor of the server, an agent or a template.
struct Lane {
    name: String,
    worker: String,
    subtitle: String,
    away: Option<String>,
}

/// The rows of the line of time, in the order of the grid.
fn lanes(occupancy: &Json, now_ms: i64) -> Vec<Lane> {
    let mut lanes: Vec<Lane> = occupancy["executors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["name"].as_str())
        .map(|name| Lane {
            name: name.to_owned(),
            worker: name.to_owned(),
            subtitle: "servidor".to_owned(),
            away: None,
        })
        .collect();
    for agent in occupancy["agents"].as_array().into_iter().flatten() {
        let name = agent["name"].as_str().unwrap_or_default().to_owned();
        let how = if agent["launched_by"].is_string() { "efêmero" } else { "agente" };
        let away = (agent["connected"] != true).then(|| match agent["seen_ms_ago"].as_i64() {
            Some(ago) => format!("Desconectado há {}", texto::duration(ago)),
            None => "Desconectado".to_owned(),
        });
        lanes.push(Lane { worker: format!("agente/{name}"), name, subtitle: how.to_owned(), away });
    }
    for template in occupancy["templates"].as_array().into_iter().flatten() {
        let name = template["name"].as_str().unwrap_or_default().to_owned();
        let up = template["max_agents"].as_u64().unwrap_or(1);
        let away = (template["launcher_on"] != true).then(|| {
            format!(
                "O plugin {} está desligado: este modelo não sobe agentes",
                template["launcher"].as_str().unwrap_or_default()
            )
        });
        lanes.push(Lane {
            worker: format!("modelo/{name}"),
            name,
            subtitle: format!("modelo, até {up}"),
            away,
        });
    }
    let _ = now_ms;
    lanes
}

/// The page of the queue and the agents.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let occupancy = s.occupancy.get();
    let now_ms = occupancy["now_ms"].as_i64().unwrap_or(0);
    let now = now_ms.div_euclid(60_000);
    let builds: Vec<Json> = occupancy["builds"].as_array().cloned().unwrap_or_default();
    let rows = lanes(&occupancy, now_ms);
    let index_of = |worker: &str| rows.iter().position(|lane| lane.worker == worker);

    let resources: Vec<Resource> = rows
        .iter()
        .enumerate()
        .map(|(i, lane)| {
            let resource = Resource::new(i as u64 + 1, lane.name.clone()).subtitle(lane.subtitle.clone());
            match &lane.away {
                Some(why) => resource.unavailable(why.clone()),
                None => resource,
            }
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
                b["job"].as_str().unwrap_or_default().replace('/', " · "),
                b["number"].as_u64().unwrap_or(0),
                match status {
                    "running" => " · rodando",
                    "failed" => " · falhou",
                    _ => "",
                }
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
    // The two hours fill the card: the window, less the menu, the margins and the names.
    let menu = if s.narrow() { 72.0 } else { 248.0 };
    let quarters = 4.0 * HOURS as f32;
    let tick_width = ((s.width.get() - menu - 64.0 - 200.0) / quarters).max(48.0);
    let axis = State::new(TimeAxis {
        tick_width,
        ..TimeAxis::quarter_hours(TimePoint::from_minutes(now + 15 - 60 * i64::from(HOURS)), HOURS)
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
        .compact(true)
        .legend("A linha vertical é agora")
        .resources_title("AGENTES")
        .selection(s.selected_event.binding())
        .on_open(open);

    let waiting: Vec<&Json> = builds.iter().filter(|b| b["status"] == "queued").collect();
    let queue: AnyView = if waiting.is_empty() {
        Box::new(Text::new("Ninguém na fila.").foreground(Semantic::InkMuted))
    } else {
        let rows: Vec<AnyView> = waiting.iter().map(|b| queued_row(s, b, now_ms)).collect();
        Box::new(Card::new(VStack::new(rows).align(Align::Stretch)).padding(0.0))
    };

    let grid_rows: Vec<Vec<ravenrs::prelude::Value>> = agent_rows(&occupancy);
    let grid = DataGrid::new(
        GridData::new(grid_rows),
        [
            GridColumn::text("Agente").width(150.0),
            GridColumn::text("Como sobe").fill(),
            GridColumn::text("Rótulos").width(170.0),
            GridColumn::text("Executores").width(110.0),
            GridColumn::text("Memória").width(140.0).align(CellAlign::End),
            GridColumn::text("Estado").width(150.0).template(|value| state_badge(&value.key_text())),
        ],
    )
    .toolbar(false)
    .filter_row(false)
    .height(240.0);

    let header = {
        let mut header = PageHeader::new("Fila e agentes").description(format!(
            "Onde cada build roda agora. Contêineres e pods nascem por build e somem no fim. Horário do servidor (UTC), agora {}.",
            texto::clock(now_ms)
        ));
        if s.admin() {
            let add = s.clone();
            header =
                header.action(Button::new("Adicionar agente").icon(IconName::Plus).on_click(move || {
                    add.agent_dialog.set(true);
                }));
        }
        header
    };
    Box::new(
        VStack::new((
            header,
            Card::new(timeline).padding(0.0),
            Text::new(format!("Na fila · {}", waiting.len())).style(TextStyle::Headline),
            queue,
            grid,
            agent_dialog(s),
        ))
        .spacing(24.0)
        .padding(32.0)
        .align(Align::Stretch),
    )
}

/// The rows of the grid of agents: the executors of the server, the agents and the templates.
fn agent_rows(occupancy: &Json) -> Vec<Vec<ravenrs::prelude::Value>> {
    let mut rows = Vec::new();
    for e in occupancy["executors"].as_array().into_iter().flatten() {
        let busy = e["busy"].as_u64().unwrap_or(0);
        rows.push(vec![
            e["name"].as_str().unwrap_or_default().to_owned().into(),
            "executor do servidor · mesma máquina".to_owned().into(),
            "—".to_owned().into(),
            format!("{busy} de 1").into(),
            "no servidor".to_owned().into(),
            (if busy > 0 { "running" } else { "ready" }).to_owned().into(),
        ]);
    }
    for a in occupancy["agents"].as_array().into_iter().flatten() {
        let labels: Vec<&str> =
            a["labels"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
        let how = match a["launched_by"].as_str() {
            Some(template) => format!("pelo modelo {template}"),
            None => "agente · conecta para fora".to_owned(),
        };
        rows.push(vec![
            a["name"].as_str().unwrap_or_default().to_owned().into(),
            how.into(),
            labels.join(", ").into(),
            format!("{} de {}", a["busy"].as_u64().unwrap_or(0), a["executors"].as_u64().unwrap_or(1)).into(),
            a["memory_bytes"].as_u64().map_or_else(|| "—".to_owned(), texto::bytes).into(),
            (if a["connected"] == true { "connected" } else { "away" }).to_owned().into(),
        ]);
    }
    for t in occupancy["templates"].as_array().into_iter().flatten() {
        let labels: Vec<&str> =
            t["labels"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
        rows.push(vec![
            t["name"].as_str().unwrap_or_default().to_owned().into(),
            format!("{} · um agente por build", t["launcher"].as_str().unwrap_or_default()).into(),
            labels.join(", ").into(),
            format!("0 de {}", t["max_agents"].as_u64().unwrap_or(1)).into(),
            "só durante o build".to_owned().into(),
            (if t["launcher_on"] == true { "ready" } else { "plugin-off" }).to_owned().into(),
        ]);
    }
    rows
}

/// How an agent is, in a badge with its word.
fn state_badge(state: &str) -> AnyView {
    match state {
        "running" => jobs::status_badge("running"),
        "connected" => Box::new(Badge::new("Conectado").tone(Tone::Success).icon(IconName::Check)),
        "ready" => Box::new(Badge::new("Pronto").tone(Tone::Success).icon(IconName::Check)),
        "plugin-off" => Box::new(Badge::new("Plugin desligado").tone(Tone::Warning)),
        _ => Box::new(Badge::new("Desconectado").outline()),
    }
}

/// The dialog of "Adicionar agente": the name and the labels, and the command that brings it in
/// with a token used once.
fn agent_dialog(s: &Rc<S>) -> AnyView {
    if !s.agent_dialog.get() {
        return nada();
    }
    let token = s.agent_token.get();
    let make = {
        let s = s.clone();
        move || {
            let name = s.agent_name.get().trim().to_owned();
            let labels: Vec<String> = s
                .agent_labels
                .get()
                .split(',')
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty())
                .collect();
            s.send(
                "/api/ci/agents/token",
                &json!({ "name": name, "labels": labels, "executors": 1 }),
                |s, status, body| {
                    if status == 200 {
                        s.agent_token.set(body["token"].as_str().unwrap_or_default().to_owned());
                    } else {
                        s.message.set(body["message"].as_str().unwrap_or("não fez o token").to_owned());
                    }
                },
            );
        }
    };
    let close = {
        let s = s.clone();
        move || {
            s.agent_dialog.set(false);
            s.agent_token.set(String::new());
        }
    };
    let said: AnyView = if token.is_empty() {
        nada()
    } else {
        Box::new(
            VStack::new((
                Text::new("Rode na máquina do agente, em até 10 minutos:").foreground(Semantic::InkMuted),
                CodeBlock::new(format!(
                    "RAVEN_CI_AGENT_TOKEN={token} raven-ci-agent --servidor <endereço deste servidor> --nome {} --rotulos {}",
                    s.agent_name.get().trim(),
                    s.agent_labels.get().replace(' ', "")
                )),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
    };
    Box::new(
        Dialog::new(s.agent_dialog.binding(), "Adicionar agente")
            .description("O agente entra com um token de uso único e guarda a chave dele. Os rótulos dizem quais builds ele pega.")
            .width(520.0)
            .content(
                VStack::new((
                    TextField::new(s.agent_name.binding()).placeholder("Nome, como linux-1"),
                    TextField::new(s.agent_labels.binding()).placeholder("Rótulos, como linux, rust"),
                    said,
                ))
                .spacing(12.0)
                .align(Align::Stretch),
            )
            .action(Button::new("Fechar").style(ButtonStyle::Secondary).on_click(close))
            .action(Button::new("Fazer o token").icon(IconName::Key).on_click(make)),
    )
}

/// A build waiting: what it is, why it waits, and what lets it go: "Ligar o plugin" when a plugin
/// switched off holds it, "Subir na fila" otherwise, for those who may act.
fn queued_row(s: &Rc<S>, build: &Json, now_ms: i64) -> AnyView {
    let job = build["job"].as_str().unwrap_or_default().to_owned();
    let number = build["number"].as_u64().unwrap_or(0);
    let priority = build["priority"].as_i64().unwrap_or(0);
    let since = build["created_ms"].as_i64().map(|t| texto::ago(t, now_ms)).unwrap_or_default();
    let why = build["waiting"]["why"].as_str().unwrap_or("espera um executor livre").to_owned();
    let plugin = build["waiting"]["plugin"].as_str().map(str::to_owned);
    let action: AnyView = match (&plugin, s.admin(), s.runs()) {
        (Some(plugin), true, _) => {
            let (s, plugin) = (s.clone(), plugin.clone());
            Box::new(
                Button::new("Ligar o plugin").style(ButtonStyle::Ghost).size(ButtonSize::Small).on_click(
                    move || {
                        let name = plugin.clone();
                        s.send("/api/ci/plugins/enable", &json!({ "name": name }), move |s, status, body| {
                            if status == 200 {
                                s.message.set(format!("Plugin {name} ligado."));
                                s.load_shell();
                                s.go(Pagina::Fila);
                            } else {
                                s.message.set(body["message"].as_str().unwrap_or("não ligou").to_owned());
                            }
                        });
                    },
                ),
            )
        }
        (None, _, true) => {
            let (s, job) = (s.clone(), job.clone());
            Box::new(
                Button::new("Subir na fila")
                    .icon(IconName::ArrowUp)
                    .style(ButtonStyle::Secondary)
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        let body = json!({ "job": job, "number": number, "priority": priority + 1 });
                        s.send("/api/ci/priority", &body, |s, status, body| {
                            if status == 200 {
                                s.message.set("Prioridade subiu.".to_owned());
                                s.go(Pagina::Fila);
                            } else {
                                s.message.set(body["message"].as_str().unwrap_or("não subiu").to_owned());
                            }
                        });
                    }),
            )
        }
        _ => nada(),
    };
    Box::new(
        HStack::new((
            Icon::new(if plugin.is_some() { IconName::Plug } else { IconName::Clock })
                .color(Semantic::Warning),
            Text::new(format!("{} #{number}", job.replace('/', " · "))).font_weight(Weight::SEMIBOLD),
            Text::new(format!("{why} · pedido {since}")).foreground(Semantic::InkMuted).truncate(),
            Spacer::new(),
            action,
        ))
        .spacing(12.0)
        .padding(12.0)
        .align(Align::Center),
    )
}
