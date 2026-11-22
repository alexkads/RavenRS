//! The page of a build (P-02, #886; page T2 and interactions I2, I3 and I5): its stages drawn on a
//! read-only `FlowCanvas`, the log as it comes in a `LogView` with a section per stage, the tabs
//! of tests, artifacts, parameters and those of the plugins, the details, and Repetir and Cancelar.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, escaped, jobs, nada, plugin};
use crate::texto;

/// The width and height of a stage's card, and the room between two.
const CARD: (f32, f32) = (200.0, 76.0);
const GAP: f32 = 56.0;

impl S {
    /// Opens the build `number` of `job`: a log of its own, then what the server says of it.
    pub(super) fn open_build(self: &Rc<Self>, job: &str, number: u64) {
        let key = (job.to_owned(), number);
        if self.log_of.borrow().as_ref() != Some(&key) {
            self.log.clear();
            self.log_seq.set(0);
            self.log_section.borrow_mut().clear();
            self.build.set(Json::Null);
            self.build_tab.set(0);
            *self.log_of.borrow_mut() = Some(key);
        }
        self.reload_build(job, number);
    }

    /// Reads the build again, then the lines of its log after the last one shown.
    pub(super) fn reload_build(self: &Rc<Self>, job: &str, number: u64) {
        let path = format!("/api/ci/build?job={}&number={number}", escaped(job));
        let (job, wanted) = (job.to_owned(), (job.to_owned(), number));
        self.read(&path, move |s, body| {
            if s.log_of.borrow().as_ref() != Some(&wanted) {
                return;
            }
            let mut steps = std::collections::BTreeMap::new();
            for stage in body["stages"].as_array().into_iter().flatten() {
                let name = stage["name"].as_str().unwrap_or_default();
                for step in stage["steps"].as_array().into_iter().flatten() {
                    steps.insert(step["node"].as_str().unwrap_or_default().to_owned(), name.to_owned());
                }
            }
            *s.stage_of_step.borrow_mut() = steps;
            s.draw_stages(&body);
            s.build.set(body);
            let after = s.log_seq.get();
            let path = format!("/api/ci/log?job={}&number={number}&after={after}", escaped(&job));
            s.read(&path, move |s, lines| {
                if s.log_of.borrow().as_ref() != Some(&wanted) {
                    return;
                }
                for line in lines.as_array().into_iter().flatten() {
                    s.push_line(line);
                }
            });
        });
    }

    /// A line the live channel heard, when it is of the build open and new.
    pub(super) fn heard_line(&self, data: &Json) {
        let job = data["job"].as_str().unwrap_or_default();
        let number = data["number"].as_u64().unwrap_or(0);
        if self.log_of.borrow().as_ref().is_some_and(|(j, n)| j == job && *n == number) {
            self.push_line(data);
        }
    }

    /// Puts a line of the log on screen, opening the section of its stage when it changes.
    fn push_line(&self, line: &Json) {
        let seq = line["seq"].as_u64().unwrap_or(0);
        if seq <= self.log_seq.get() {
            return;
        }
        self.log_seq.set(seq);
        let step = line["step"].as_str().unwrap_or_default();
        let stage = self.stage_of_step.borrow().get(step).cloned().unwrap_or_default();
        if !stage.is_empty() && *self.log_section.borrow() != stage {
            self.log.section(&stage);
            *self.log_section.borrow_mut() = stage;
        }
        self.log.push(line["text"].as_str().unwrap_or_default());
    }

    /// The stages as cards joined left to right, drawn again only when they are others.
    fn draw_stages(&self, body: &Json) {
        let stages = body["stages"].as_array().cloned().unwrap_or_default();
        let names: Vec<String> =
            stages.iter().filter_map(|s| s["name"].as_str().map(str::to_owned)).collect();
        let signature = names.join("\u{1f}");
        if *self.graph_of.borrow() == signature {
            return;
        }
        let mut graph = FlowGraph::new();
        for (i, stage) in stages.iter().enumerate() {
            let id = format!("estagio-{i}");
            let kinds: Vec<String> = stage["steps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|step| step["kind"].as_str().map(str::to_uppercase))
                .collect();
            let node = FlowNode::new(
                &id,
                "estagio",
                stage["name"].as_str().unwrap_or_default(),
                i as f32 * (CARD.0 + GAP),
                0.0,
            )
            .subtitle(kinds.join(" · "))
            .size(CARD.0, CARD.1)
            .port(FlowPort::input("entra"))
            .port(FlowPort::output("sai"));
            graph = graph.node(node);
            if i > 0 {
                graph = graph.edge(FlowEdge::new(
                    format!("liga-{i}"),
                    FlowEndpoint::new(format!("estagio-{}", i - 1), "sai"),
                    FlowEndpoint::new(&id, "entra"),
                ));
            }
        }
        self.graph.set(graph);
        *self.graph_of.borrow_mut() = signature;
    }
}

/// How each stage went, on its card. While the build runs, the first stage still waiting is the
/// one running: the core marks a stage only when one of its steps ends.
fn run_view(stages: &[Json], reason: Option<&str>, running: bool) -> FlowRunView {
    let current = running
        .then(|| stages.iter().position(|st| st["status"] == "pending" || st["status"] == "running"))
        .flatten();
    stages.iter().enumerate().fold(FlowRunView::new(), |view, (i, stage)| {
        let said = if Some(i) == current { "running" } else { stage["status"].as_str().unwrap_or_default() };
        let status = match said {
            "done" => FlowNodeStatus::Success,
            "failed" => FlowNodeStatus::Error(reason.unwrap_or("falhou").to_owned()),
            "running" => FlowNodeStatus::Running,
            "skipped" => FlowNodeStatus::Skipped,
            "cancelled" => FlowNodeStatus::Cancelled,
            _ => FlowNodeStatus::Waiting("esperando".to_owned()),
        };
        view.node(format!("estagio-{i}"), FlowNodeRun::new(status))
    })
}

/// The page of the build `number` of `job`.
pub(super) fn page(s: &Rc<S>, job: &str, number: u64) -> AnyView {
    let body = s.build.get();
    if body.is_null() {
        return Box::new(
            VStack::new((PageHeader::new(format!("Build #{number}")).description(format!("{job}: lendo…")),))
                .padding(24.0)
                .align(Align::Stretch),
        );
    }
    let build = &body["build"];
    let status = build["status"].as_str().unwrap_or_default().to_owned();
    let stages = body["stages"].as_array().cloned().unwrap_or_default();
    let now = s.server.get()["now_ms"].as_i64().unwrap_or(0).max(build["created_ms"].as_i64().unwrap_or(0));
    let running = matches!(status.as_str(), "running" | "queued");

    let mut trail: Vec<String> = vec!["Jobs".to_owned()];
    trail.extend(job.split('/').map(str::to_owned));
    trail.push(format!("#{number}"));
    let back = {
        let s = s.clone();
        move |at: usize| {
            if at == 0 {
                s.go(Pagina::Jobs);
            }
        }
    };
    let breadcrumb = Breadcrumb::new(trail).on_navigate(back);

    let actor = build["actor"].as_str().map(|a| format!(" · por {a}")).unwrap_or_default();
    let when =
        build["created_ms"].as_i64().map(|t| format!(" · pedido {}", texto::ago(t, now))).unwrap_or_default();
    let description = format!("{}{actor}{when}", build["cause"].as_str().unwrap_or_default());
    let header = PageHeader::new(format!("Build #{number}"))
        .badge(jobs::status_badge(&status))
        .description(description)
        .action(actions(s, job, number, running));

    let done = stages.iter().filter(|st| st["status"] == "done").count();
    let current = stages
        .iter()
        .position(|st| st["status"] == "running" || st["status"] == "failed")
        .or_else(|| stages.iter().position(|st| st["status"] == "pending"))
        .unwrap_or(stages.len().saturating_sub(1));
    let label = match stages.get(current) {
        Some(stage) if !stages.is_empty() => format!(
            "Estágio {} de {} · {}",
            current + 1,
            stages.len(),
            stage["name"].as_str().unwrap_or_default()
        ),
        _ => "Sem estágios".to_owned(),
    };
    let share = if stages.is_empty() { 0.0 } else { done as f32 / stages.len() as f32 };
    let progress = ProgressBar::new(share).label(label).percent().tone(match status.as_str() {
        "failed" | "timed_out" => Tone::Danger,
        "succeeded" => Tone::Success,
        _ => Tone::Accent,
    });

    let reason = build["reason"].as_str();
    let canvas = FlowCanvas::new(s.graph.binding())
        .execution(run_view(&stages, reason, status == "running"))
        .read_only(true)
        .controls(true)
        .minimap(false)
        .label("Estágios do pipeline")
        .size(900.0, 170.0);

    let main = VStack::new((tabs(s, &body, job, number),)).align(Align::Stretch).grow(1.0);
    let aside = details(&body, now);
    Box::new(
        VStack::new((
            breadcrumb,
            header,
            progress,
            Card::new(canvas).padding(8.0),
            HStack::new((main, aside)).spacing(20.0).align(Align::Start),
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// Repetir (R) and, while it runs, Cancelar (⌘ .), for those who may act on builds.
fn actions(s: &Rc<S>, job: &str, number: u64, running: bool) -> AnyView {
    if !s.runs() {
        return nada();
    }
    let again = {
        let (s, job) = (s.clone(), job.to_owned());
        move || {
            let job = job.clone();
            s.send("/api/ci/rebuild", &json!({ "job": job, "number": number }), move |s, status, body| {
                match body["number"].as_u64().filter(|_| status == 200) {
                    Some(new) => {
                        s.message.set(format!("Build #{new} pedido, repetindo o #{number}."));
                        s.go(Pagina::Build(job.clone(), new));
                    }
                    None => s.message.set(body["message"].as_str().unwrap_or("não repetiu").to_owned()),
                }
            });
        }
    };
    let cancel = {
        let (s, job) = (s.clone(), job.to_owned());
        move || {
            let (asked, job) = (s.clone(), job.clone());
            s.confirm.ask(
                format!("Cancelar o build #{number}?"),
                "O processo e todos os filhos param agora. A pasta do build é apagada, mesmo que ele já tenha falhado.",
                "Cancelar build",
                move || {
                    let job = job.clone();
                    asked.send("/api/ci/cancel", &json!({ "job": job, "number": number }), move |s, status, body| {
                        if status == 200 {
                            s.message.set(format!("Build #{number} cancelado."));
                            s.reload_build(&job, number);
                        } else {
                            s.message.set(body["message"].as_str().unwrap_or("não cancelou").to_owned());
                        }
                    });
                },
            );
        }
    };
    let repeat = Button::new("Repetir")
        .icon(IconName::RotateCcw)
        .style(ButtonStyle::Secondary)
        .shortcut("R")
        .on_click(again);
    if running {
        Box::new(
            HStack::new((
                repeat,
                Button::new("Cancelar")
                    .icon(IconName::Square)
                    .style(ButtonStyle::Danger)
                    .shortcut("⌘ .")
                    .on_click(cancel),
            ))
            .spacing(8.0),
        )
    } else {
        Box::new(repeat)
    }
}

/// The tabs: the log, the tests, the artifacts, the parameters, and the tabs of the plugins.
fn tabs(s: &Rc<S>, body: &Json, job: &str, number: u64) -> AnyView {
    let attachments = body["attachments"].as_array().cloned().unwrap_or_default();
    let tests: Vec<&Json> = attachments.iter().filter(|a| a["kind"] == "junit").collect();
    let artifacts: Vec<&Json> = attachments.iter().filter(|a| a["kind"] != "junit").collect();
    let log = LogView::new(s.log.clone())
        .follow_end(true)
        .line_numbers(true)
        .searchable(true)
        .label("Log do build")
        .height(440.0);
    let mut tabs = Tabs::new(s.build_tab.binding())
        .tab("Log", log)
        .tab(
            format!("Testes · {}", tests.len()),
            attachment_grid(s, &tests, "Nenhum resultado de teste. O plugin junit põe os seus aqui."),
        )
        .tab(
            format!("Artefatos · {}", artifacts.len()),
            attachment_grid(s, &artifacts, "Nenhum artefato. O plugin artifacts põe os seus aqui."),
        )
        .tab("Parâmetros", parameters(&body["build"]["parameters"]));
    for screen in s.screens.get().as_array().into_iter().flatten().filter(|sc| sc["kind"] == "build_tab") {
        let caption = screen["caption"].as_str().unwrap_or_default().to_owned();
        tabs = tabs.tab(caption, plugin::screen_view(s, screen, Some((job, number))));
    }
    Box::new(tabs)
}

/// What a build left, in a grid, with the address that downloads each.
fn attachment_grid(s: &S, attachments: &[&Json], empty: &str) -> AnyView {
    let rows: Vec<Vec<ravenrs::prelude::Value>> = attachments
        .iter()
        .map(|a| {
            let id = a["id"].as_i64().unwrap_or(0);
            vec![
                a["name"].as_str().unwrap_or_default().to_owned().into(),
                a["mime"].as_str().unwrap_or_default().to_owned().into(),
                texto::bytes(a["size"].as_u64().unwrap_or(0)).into(),
                summary(&a["summary"]).into(),
                s.client.url_of(&format!("/api/ci/attachment?id={id}")).into(),
            ]
        })
        .collect();
    Box::new(
        DataGrid::new(
            GridData::new(rows),
            [
                GridColumn::text("Nome").fill(),
                GridColumn::text("Tipo").width(150.0),
                GridColumn::text("Tamanho").width(100.0),
                GridColumn::text("Resumo").width(220.0),
                GridColumn::text("Endereço").width(260.0),
            ],
        )
        .empty_text(empty)
        .height(260.0),
    )
}

/// A summary of an attachment in one line: `tests: 812 · failures: 0`.
fn summary(value: &Json) -> String {
    match value {
        Json::Object(fields) => {
            fields.iter().map(|(k, v)| format!("{k}: {}", plain(v))).collect::<Vec<_>>().join(" · ")
        }
        Json::Null => String::new(),
        other => plain(other),
    }
}

fn plain(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The values the build was asked with.
fn parameters(values: &Json) -> AnyView {
    let rows: Vec<PropertyRow> = values
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, value)| PropertyRow::new(name.clone(), plain(value)))
        .collect();
    if rows.is_empty() {
        return Box::new(Text::new("Este build não tem parâmetros.").foreground(Semantic::InkMuted));
    }
    Box::new(VStack::new(rows).spacing(4.0).align(Align::Stretch))
}

/// The details beside the log: status, cause, who, origin, priority and why it did not pass.
fn details(body: &Json, now: i64) -> AnyView {
    let build = &body["build"];
    let mut rows = vec![
        PropertyRow::new("Estado", texto::status_words(build["status"].as_str().unwrap_or_default())),
        PropertyRow::new("Gatilho", build["cause"].as_str().unwrap_or("—")),
        PropertyRow::new("Pedido por", build["actor"].as_str().unwrap_or("o servidor")),
        PropertyRow::new("Prioridade", build["priority"].as_i64().unwrap_or(0).to_string()),
    ];
    if let Some(origin) = build["origin"].as_u64() {
        rows.push(PropertyRow::new("Repete", format!("o build #{origin}")));
    }
    if let (Some(start), Some(end)) = (build["created_ms"].as_i64(), build["ended_ms"].as_i64()) {
        rows.push(PropertyRow::new("Duração", texto::duration(end - start)));
        rows.push(PropertyRow::new("Terminou", texto::ago(end, now)));
    }
    if build["joined"].as_u64().unwrap_or(0) > 0 {
        rows.push(PropertyRow::new("Pedidos juntados", build["joined"].as_u64().unwrap_or(0).to_string()));
    }
    let reason: AnyView = match build["reason"].as_str() {
        Some(why) => Box::new(Alert::new("Por que não passou", why).tone(Tone::Danger).compact()),
        None => nada(),
    };
    let stages: Vec<AnyView> = body["stages"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|stage| -> AnyView {
            Box::new(PropertyRow::new(
                stage["name"].as_str().unwrap_or_default(),
                texto::stage_words(stage["status"].as_str().unwrap_or_default()),
            ))
        })
        .collect();
    Box::new(
        VStack::new((Card::new(
            VStack::new((
                Text::new("Detalhes").style(TextStyle::Headline),
                VStack::new(rows).spacing(4.0).align(Align::Stretch),
                reason,
                Text::new("ESTÁGIOS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                VStack::new(stages).spacing(4.0).align(Align::Stretch),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
        .padding(16.0),))
        .width(300.0)
        .align(Align::Stretch),
    )
}
