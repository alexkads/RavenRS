//! The page of a build (P-02, #886; page T2 and interactions I2, I3 and I5, as #919 draws them):
//! its stages on a read-only `FlowCanvas` whose cards say how each went and how long it took (a
//! click shows only that stage's log), the log as it comes in a `LogView` with a section per stage
//! that says its kind and time, the tones of `PASS`, `SLOW` and `FAIL`, "Seguir o fim" and "Baixar
//! log", the tabs of tests, artifacts, parameters and the plugins of the build, the details beside
//! the log (under it in a narrow window), Repetir and Cancelar, and the stages as a list on a phone
//! (T7).

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
            self.log_sections.borrow_mut().clear();
            self.stage_only.set(None);
            self.stage_pick.set(FlowSelection::default());
            self.stage_picked.borrow_mut().clear();
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
            s.describe_sections(&body);
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
                let body = s.build.get();
                s.describe_sections(&body);
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
            let at = self.log.section(&stage);
            self.log_sections.borrow_mut().insert(stage.clone(), at);
            *self.log_section.borrow_mut() = stage;
        }
        self.log.push(line["text"].as_str().unwrap_or_default());
    }

    /// Says on the title of each stage's section of the log how it ran: its kinds and its time.
    fn describe_sections(&self, body: &Json) {
        let sections = self.log_sections.borrow();
        for stage in body["stages"].as_array().into_iter().flatten() {
            let name = stage["name"].as_str().unwrap_or_default();
            let Some(at) = sections.get(name) else { continue };
            let mut parts = vec![kinds_of(stage)];
            if let Some(took) = stage_time(body, name) {
                parts.push(texto::duration(took));
            }
            self.log.section_detail(*at, &parts.join(" · "));
        }
    }

    /// The stages as cards joined left to right, drawn again only when they or their times are
    /// others: each card says its kind above its name, and its time or its state under it.
    fn draw_stages(&self, body: &Json) {
        let stages = body["stages"].as_array().cloned().unwrap_or_default();
        let lines: Vec<String> = stages
            .iter()
            .map(|st| {
                format!(
                    "{}|{}|{:?}",
                    st["name"],
                    st["status"],
                    stage_time(body, st["name"].as_str().unwrap_or(""))
                )
            })
            .collect();
        let signature = lines.join("\u{1f}");
        if *self.graph_of.borrow() == signature {
            return;
        }
        let mut graph = FlowGraph::new();
        for (i, stage) in stages.iter().enumerate() {
            let id = format!("estagio-{i}");
            let name = stage["name"].as_str().unwrap_or_default();
            let detail = match (stage["status"].as_str(), stage_time(body, name)) {
                (Some("running"), Some(took)) => format!("{} · rodando", texto::duration(took)),
                (Some("running"), None) => "rodando".to_owned(),
                (Some("skipped"), _) => "pulado".to_owned(),
                (_, Some(took)) => texto::duration(took),
                _ => String::new(),
            };
            let mut node = FlowNode::new(&id, &id, name, i as f32 * (CARD.0 + GAP), 0.0)
                .size(CARD.0, CARD.1)
                .port(FlowPort::input("entra"))
                .port(FlowPort::output("sai"));
            if !detail.is_empty() {
                node = node.subtitle(detail);
            }
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

    /// A click on a stage of the diagram shows only its log; a click on the empty canvas, or
    /// "Mostrar tudo" in the log, shows every line again.
    fn follow_pick(&self, stages: &[Json]) {
        let picked = self.stage_pick.get().nodes;
        if *self.stage_picked.borrow() == picked {
            return;
        }
        *self.stage_picked.borrow_mut() = picked.clone();
        let name = picked
            .first()
            .and_then(|id| id.strip_prefix("estagio-"))
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| stages.get(i))
            .and_then(|st| st["name"].as_str().map(str::to_owned));
        self.stage_only.set(name);
    }
}

/// How long the stage `name` ran, by the lines its steps wrote.
fn stage_time(body: &Json, name: &str) -> Option<i64> {
    body["stage_times"]
        .as_array()?
        .iter()
        .find(|t| t["stage"] == name)
        .and_then(|t| Some(t["end_ms"].as_i64()? - t["start_ms"].as_i64()?))
}

/// What a stage runs, as its card and its log say it: its kinds, each once (`sh`, `cargo`).
fn kinds_of(stage: &Json) -> String {
    let mut kinds: Vec<String> = Vec::new();
    for step in stage["steps"].as_array().into_iter().flatten() {
        let kind = step["kind"].as_str().unwrap_or_default();
        let kind = kind.rsplit('/').next().unwrap_or(kind).to_owned();
        if !kind.is_empty() && !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds.join(" · ")
}

/// The icon of a stage's card by what it runs.
fn kind_icon(kinds: &str) -> IconName {
    if kinds.contains("docker") || kinds.contains("container") {
        IconName::Package
    } else if kinds.contains("git") || kinds.contains("checkout") {
        IconName::GitBranch
    } else if kinds.contains("kubectl") {
        IconName::Layers
    } else {
        IconName::Terminal
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
            "interrupted" => FlowNodeStatus::Warning("interrompido: o agente sumiu".to_owned()),
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
                .padding(32.0)
                .align(Align::Stretch),
        );
    }
    let build = &body["build"];
    let status = build["status"].as_str().unwrap_or_default().to_owned();
    let stages = body["stages"].as_array().cloned().unwrap_or_default();
    s.follow_pick(&stages);
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
    // On a phone the place of the build is a muted line, as the board T7 draws it.
    let breadcrumb: AnyView = if s.phone() {
        Box::new(Text::new(job.replace('/', " · ")).style(TextStyle::Callout).foreground(Semantic::InkMuted))
    } else {
        Box::new(Breadcrumb::new(trail).on_navigate(back))
    };

    let mut said = vec![build["cause"].as_str().unwrap_or_default().to_owned()];
    if let Some(commit) = build["commit"].as_str() {
        said.push(format!("commit {}", commit.chars().take(7).collect::<String>()));
    }
    if let Some(worker) = build["worker"].as_str() {
        said.push(format!("agente {}", jobs::agent_words(worker)));
    }
    if let Some(actor) = build["actor"].as_str() {
        said.push(format!("por {actor}"));
    }
    let began = body["started_ms"].as_i64().or_else(|| build["created_ms"].as_i64());
    if let Some(t) = began {
        said.push(if running {
            format!("começou {}", texto::ago(t, now))
        } else {
            format!("pedido {}", texto::ago(t, now))
        });
    }
    // On a phone the title and its state on one line, as T7 draws them, and the buttons go to the
    // foot of the page.
    let header: AnyView = if s.phone() {
        Box::new(PageHeader::new(format!("Build #{number}")).badge(jobs::status_badge(&status)).compact())
    } else {
        Box::new(
            PageHeader::new(format!("Build #{number}"))
                .badge(jobs::status_badge(&status))
                .description(said.join(" · "))
                .action(actions(s, job, number, running, false)),
        )
    };

    let done = stages.iter().filter(|st| st["status"] == "done").count();
    let current = stages
        .iter()
        .position(|st| st["status"] == "running" || st["status"] == "failed")
        .or_else(|| stages.iter().position(|st| st["status"] == "pending"))
        .unwrap_or(stages.len().saturating_sub(1));
    let label = match stages.get(current) {
        Some(stage) => format!(
            "Estágio {} de {} · {}",
            current + 1,
            stages.len(),
            stage["name"].as_str().unwrap_or_default()
        ),
        None => "Sem estágios".to_owned(),
    };
    let share = if stages.is_empty() { 0.0 } else { done as f32 / stages.len() as f32 };
    let mut progress = ProgressBar::new(share).label(label).percent().tone(match status.as_str() {
        "failed" | "timed_out" => Tone::Danger,
        "succeeded" => Tone::Success,
        "unstable" => Tone::Warning,
        "interrupted" | "cancelled" => Tone::Neutral,
        _ => Tone::Accent,
    });
    if let Some(tests) = tests_said(&body) {
        progress = progress.detail(tests).detail_inline(true);
    }

    let reason = build["reason"].as_str();
    let diagram: AnyView = if s.phone() {
        stage_list(s, &body, &stages)
    } else {
        let mut canvas = FlowCanvas::new(s.graph.binding())
            .execution(run_view(&stages, reason, status == "running"))
            .status_icons(true)
            .selection(s.stage_pick.binding())
            .read_only(true)
            .controls(true)
            .minimap(false)
            .label("Estágios do pipeline");
        for (i, stage) in stages.iter().enumerate() {
            let kinds = kinds_of(stage);
            canvas = canvas.kind(format!("estagio-{i}"), FlowKind::new(kind_icon(&kinds), kinds));
        }
        // A strip of 198 as wide as the page leaves it, with what a click does beside the zoom.
        Box::new(canvas.height(198.0).hint("Clique num estágio para ver só o log dele"))
    };

    let main = VStack::new((tabs(s, &body, job, number),)).align(Align::Stretch).grow(1.0);
    let aside = details(&body);
    let lower: AnyView = if s.phone() {
        // On a phone the log is the page; the details stay for a wider window (T7).
        Box::new(main)
    } else if s.narrow() {
        // In a narrow window the details go under the log (I7).
        Box::new(VStack::new((main, aside)).spacing(24.0).align(Align::Stretch))
    } else {
        Box::new(HStack::new((main, aside)).spacing(24.0).align(Align::Start))
    };
    let foot: AnyView = if s.phone() { actions(s, job, number, running, true) } else { nada() };
    let explained = explanation(s, job, number);
    Box::new(
        VStack::new((breadcrumb, header, progress, diagram, explained, lower, foot))
            .spacing(20.0)
            .padding(if s.phone() { Insets::uniform(16.0) } else { super::PAGE_PADDING })
            .align(Align::Stretch),
    )
}

/// What the tests of the build say, for the bar: `812 de 1.360 testes` while they run, the
/// counts when they ended.
fn tests_said(body: &Json) -> Option<String> {
    let junit = body["attachments"].as_array()?.iter().find(|a| a["kind"] == "junit")?;
    let tests = junit["summary"]["tests"].as_u64()?;
    let failures = junit["summary"]["failures"].as_u64().unwrap_or(0);
    Some(if failures > 0 {
        format!("{tests} testes · {failures} falharam")
    } else {
        format!("{tests} testes")
    })
}

/// The stages as a list, on a phone (T7): each with how it went and its time; a tap shows only
/// its log.
fn stage_list(s: &Rc<S>, body: &Json, stages: &[Json]) -> AnyView {
    let rows: Vec<AnyView> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| -> AnyView {
            let name = stage["name"].as_str().unwrap_or_default().to_owned();
            // The icon says how it went, and the line says it in words to a screen reader.
            let icon = match stage["status"].as_str() {
                Some("done") => IconName::CircleCheck,
                Some("running") => IconName::LoaderCircle,
                Some("failed") => IconName::CircleAlert,
                _ => kind_icon(&kinds_of(stage)),
            };
            let state = texto::stage_words(stage["status"].as_str().unwrap_or_default());
            let took = stage_time(body, &name).map(texto::duration).unwrap_or_default();
            let pick = {
                let (s, id) = (s.clone(), format!("estagio-{i}"));
                move || s.stage_pick.set(FlowSelection { nodes: vec![id.clone()], edges: Vec::new() })
            };
            let tone = match stage["status"].as_str() {
                Some("done") => Tone::Success,
                Some("running") => Tone::Accent,
                Some("failed") => Tone::Danger,
                _ => Tone::Neutral,
            };
            // One line, as the board draws it: the time when it has one, else the state in words.
            let end = if took.is_empty() { state.to_owned() } else { took };
            Box::new(ListItem::new(name).icon(icon).tone(tone).trailing(end).on_click(pick))
        })
        .collect();
    Box::new(List::new(rows).compact())
}

/// Whether the plugin `explain` is on in the server: the button that explains a failure needs it.
fn explains(s: &Rc<S>) -> bool {
    s.server.get()["plugins_on"].as_array().is_some_and(|on| on.iter().any(|p| p == "explain"))
}

/// Asks the server to explain the failure of `job #number` with the local AI (#942): the plugin
/// `explain` runs the model outside the server and keeps the answer with the build, so a second
/// ask gives the one kept at once.
fn explain(s: &Rc<S>, job: &str, number: u64) {
    s.explanation.set(json!({ "job": job, "number": number, "state": "gerando" }));
    let path = format!("/api/ci/build/explain?job={}&number={number}", encode(job));
    let (job, kept) = (job.to_owned(), s.explanation.clone());
    s.send(&path, &json!({}), move |_, status, body| {
        let state = if status == 200 { "pronta" } else { "erro" };
        kept.set(json!({
            "job": job, "number": number, "state": state,
            "texto": body["texto"], "modelo": body["modelo"], "ms": body["ms"], "message": body["message"],
        }));
    });
}

/// `text` as the value of a query.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// The explanation of this build, above the tabs (T2 of D-01): a card with the text, the model and
/// how long it took; while the model writes it, a line that says so; nothing for another build.
fn explanation(s: &Rc<S>, job: &str, number: u64) -> AnyView {
    let asked = s.explanation.get();
    if asked["job"] != job || asked["number"] != number {
        return nada();
    }
    let body: AnyView = match asked["state"].as_str() {
        Some("gerando") => Box::new(
            HStack::new((
                Spinner::new(),
                Text::new("A IA local está lendo o resumo do build. Isso pode levar alguns minutos.")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(8.0)
            .align(Align::Center),
        ),
        Some("pronta") => {
            let model = asked["modelo"].as_str().unwrap_or("modelo local");
            let seconds =
                asked["ms"].as_u64().map_or_else(String::new, |ms| format!(" · {} s", ms.div_ceil(1000)));
            Box::new(
                VStack::new((
                    Text::new(asked["texto"].as_str().unwrap_or_default().trim().to_owned()),
                    Text::new(format!("{model}{seconds} · gerada na máquina, sem enviar nada para fora"))
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            )
        }
        _ => Box::new(
            Alert::new(
                "Não deu para explicar",
                asked["message"].as_str().unwrap_or("o servidor não respondeu"),
            )
            .tone(Tone::Danger)
            .compact(),
        ),
    };
    Box::new(Card::new(
        VStack::new((
            HStack::new((
                Icon::new(IconName::Sparkles).size(16.0).color(Semantic::Accent),
                Text::new("Por que falhou").style(TextStyle::Headline),
            ))
            .spacing(8.0)
            .align(Align::Center),
            body,
        ))
        .spacing(10.0)
        .align(Align::Stretch),
    ))
}

/// Repetir (R) and, while it runs, Cancelar (⌘ .), for those who may act on builds. On a phone
/// they go under the page, as wide as it and 48 pt tall (T7). A build that failed has also
/// Explicar a falha, when the plugin `explain` is on (#942).
fn actions(s: &Rc<S>, job: &str, number: u64, running: bool, phone: bool) -> AnyView {
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
    if phone {
        let mut buttons: Vec<AnyView> = Vec::new();
        if running {
            buttons.push(Box::new(
                Button::new("Cancelar build")
                    .icon(IconName::Square)
                    .style(ButtonStyle::Danger)
                    .size(ButtonSize::Large)
                    .full_width()
                    .on_click(cancel),
            ));
        }
        buttons.push(Box::new(
            Button::new("Repetir")
                .icon(IconName::RotateCcw)
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Large)
                .full_width()
                .on_click(again),
        ));
        return Box::new(VStack::new(buttons).spacing(12.0).align(Align::Stretch));
    }
    let repeat =
        Button::new("Repetir").icon(IconName::RotateCcw).style(ButtonStyle::Secondary).on_click(again);
    let failed = s.build.get()["build"]["status"] == "failed";
    if failed && explains(s) {
        let (s, job) = (s.clone(), job.to_owned());
        let generating = s.explanation.get()["state"] == "gerando";
        let ask = Button::new("Explicar a falha")
            .icon(IconName::Sparkles)
            .style(ButtonStyle::Secondary)
            .disabled(generating)
            .on_click(move || explain(&s, &job, number));
        return Box::new(HStack::new((ask, repeat)).spacing(8.0));
    }
    if running {
        Box::new(
            HStack::new((
                repeat,
                Button::new("Cancelar").icon(IconName::Square).style(ButtonStyle::Danger).on_click(cancel),
            ))
            .spacing(8.0),
        )
    } else {
        Box::new(repeat)
    }
}

/// The tabs: the log, the tests, the artifacts, the parameters, and the plugins of the build with
/// what each puts on the page.
fn tabs(s: &Rc<S>, body: &Json, job: &str, number: u64) -> AnyView {
    let attachments = body["attachments"].as_array().cloned().unwrap_or_default();
    let tests: Vec<&Json> = attachments.iter().filter(|a| a["kind"] == "junit").collect();
    // The summary of the build for sessions of AI (#941), first among what the build left.
    let summary_row = json!({
        "name": "resumo.md (para IA)",
        "mime": "text/markdown",
        "summary": "o que falhou, as últimas linhas, os testes e os diagnósticos; raven-ci resumo JOB NUMERO",
        "url": s.client.url_of(&format!("/api/ci/build/summary?job={}&number={number}&format=md", escaped(job))),
    });
    let mut artifacts: Vec<&Json> = vec![&summary_row];
    artifacts.extend(attachments.iter().filter(|a| a["kind"] != "junit"));
    let log = LogView::new(s.log.clone())
        .follow_end(true)
        .searchable(true)
        .keyword("PASS", Tone::Success)
        .keyword("SLOW", Tone::Warning)
        .keyword("FAIL", Tone::Danger)
        .keyword("RUN", Tone::Accent)
        .only_section(s.stage_only.binding())
        .download(format!("{}-{number}.log", job.replace('/', "-")))
        .label("Log do build")
        // On a phone only the lines, as T7 draws them.
        .controls(!s.phone())
        .line_numbers(!s.phone())
        .height(if s.phone() { 326.0 } else { 268.0 });
    let plugins: Vec<String> = body["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(str::to_owned))
        .collect();
    let mut plugin_parts: Vec<AnyView> = vec![Box::new(
        HStack::new(plugins.iter().map(|p| Badge::new(p.clone())).collect::<Vec<_>>()).spacing(8.0).wrap(),
    )];
    for screen in s.screens.get().as_array().into_iter().flatten().filter(|sc| sc["kind"] == "build_tab") {
        let caption = screen["caption"].as_str().unwrap_or_default().to_owned();
        plugin_parts.push(Box::new(Text::new(caption).style(TextStyle::Headline)));
        plugin_parts.push(plugin::screen_view(s, screen, Some((job, number))));
    }
    let tabs = Tabs::new(s.build_tab.binding())
        .tab("Log", log)
        .tab(
            if s.phone() { "Testes".to_owned() } else { format!("Testes · {}", tests.len()) },
            attachment_grid(s, &tests, "Nenhum resultado de teste. O plugin junit põe os seus aqui."),
        )
        .tab(
            if s.phone() { "Artefatos".to_owned() } else { format!("Artefatos · {}", artifacts.len() - 1) },
            attachment_grid(s, &artifacts, "Nenhum artefato. O plugin artifacts põe os seus aqui."),
        )
        .tab("Parâmetros", parameters(&body["build"]["parameters"]));
    // A phone has room for four tabs on a line (T7); the plugins' tab is for a wider window.
    if s.phone() {
        return Box::new(tabs);
    }
    Box::new(tabs.tab(
        format!("Plugins · {}", plugins.len()),
        VStack::new(plugin_parts).spacing(12.0).align(Align::Stretch),
    ))
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
                a["size"].as_u64().map(texto::bytes).unwrap_or_default().into(),
                summary(&a["summary"]).into(),
                a["url"]
                    .as_str()
                    .map_or_else(|| s.client.url_of(&format!("/api/ci/attachment?id={id}")), str::to_owned)
                    .into(),
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

/// The details beside the log: the agent, the lock, the folder, the status on the pull request,
/// how it went and why it did not pass, and the plugins of the build.
fn details(body: &Json) -> AnyView {
    let build = &body["build"];
    let plugins: Vec<String> = body["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(str::to_owned))
        .collect();
    // The rows T2 draws: the agent, the lock, the folder and the status on the pull request; the
    // state, the trigger and the time are in the header.
    let mut rows = vec![PropertyRow::new(
        "Agente",
        build["worker"].as_str().map(jobs::agent_words).unwrap_or_else(|| "—".to_owned()),
    )];
    if plugins.iter().any(|p| p == "lock") {
        rows.push(PropertyRow::new("Trava", "pelo plugin lock"));
    }
    rows.push(PropertyRow::new("Workspace", "apagado no fim"));
    if plugins.iter().any(|p| p == "github" || p == "azure-devops") {
        rows.push(PropertyRow::new(
            "Status no PR",
            texto::status_words(build["status"].as_str().unwrap_or_default()),
        ));
    }
    if let Some(origin) = build["origin"].as_u64() {
        rows.push(PropertyRow::new("Repete", format!("o build #{origin}")));
    }
    if build["joined"].as_u64().unwrap_or(0) > 0 {
        rows.push(PropertyRow::new("Pedidos juntados", build["joined"].as_u64().unwrap_or(0).to_string()));
    }
    let reason: AnyView = match build["reason"].as_str() {
        Some(why) => Box::new(Alert::new("Por que não passou", why).tone(Tone::Danger).compact()),
        None => nada(),
    };
    Box::new(
        VStack::new((Card::new(
            VStack::new((
                Text::new("Detalhes").style(TextStyle::Headline),
                VStack::new(rows).spacing(12.0).align(Align::Stretch),
                reason,
                Divider::horizontal(),
                Text::new("PLUGINS NESTE BUILD").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                HStack::new(plugins.iter().map(|p| Badge::new(p.clone()).tone(Tone::Neutral)).collect::<Vec<_>>())
                    .spacing(6.0)
                    .wrap(),
                Text::new("Cada plugin pode pôr uma aba nesta página. O junit põe a aba Testes; o artifacts, a aba Artefatos.")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
        .padding(24.0),))
        .width(300.0)
        .align(Align::Stretch),
    )
}
