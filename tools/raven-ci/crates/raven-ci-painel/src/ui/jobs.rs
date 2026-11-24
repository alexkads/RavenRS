//! The jobs (P-01, #885; page T1 and interaction I1, as #919 draws them): four numbers (the queue,
//! what runs and in which stage, the agents, the memory), the notice of a plugin switched off that
//! a build waits for, every job in a grid with its last build, its agent, its duration and the
//! trend of the last ones (a double click or Enter opens the last build; Disparar is in the
//! palette, ⌘K), cards on a phone, and the dialog that asks for a build with its parameters and
//! says how many builds wait before it.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, casca, nada};
use crate::texto;

/// A cell of the grid.
type Cell = ravenrs::prelude::Value;

/// The page of the jobs.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let overview = s.overview.get();
    let occupancy = s.occupancy.get();
    let server = s.server.get();
    let now = overview["now_ms"].as_i64().unwrap_or(0);
    let jobs: Vec<Json> = overview["jobs"].as_array().cloned().unwrap_or_default();
    let waiting: Vec<Json> = occupancy["builds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["status"] == "queued")
        .cloned()
        .collect();

    let queued = overview["queued"].as_u64().unwrap_or(0);
    let oldest = overview["oldest_queued_ms"].as_i64().map_or_else(
        || "Ninguém esperando".to_owned(),
        |t| format!("O mais antigo espera {}", texto::ago(t, now)),
    );
    let running = overview["running"].as_u64().unwrap_or(0);
    let running_now = overview["running_now"].as_array().cloned().unwrap_or_default();
    let running_detail = match running_now.first() {
        Some(build) => format!(
            "{}{}",
            build["job"].as_str().unwrap_or_default().replace('/', " · "),
            build["stage"].as_str().map(|st| format!(", estágio {st}")).unwrap_or_default()
        ),
        None => "Nenhum build em andamento".to_owned(),
    };
    let (ready, total, kinds) = agents_summary(&server, &occupancy);
    let memory = server["memory_bytes"].as_u64().map_or_else(|| "—".to_owned(), texto::bytes);
    let columns = if s.width.get() < casca::TWO_COLUMNS_BELOW { 2 } else { 4 };
    let numbers = Grid::new(
        columns,
        vec![
            stat("NA FILA", queued.to_string(), oldest),
            stat("RODANDO", running.to_string(), running_detail),
            stat("AGENTES", format!("{ready} de {total}"), kinds),
            stat("MEMÓRIA DO SERVIDOR", memory, "Lida do processo, ao vivo; meta ≤ 60 MB".to_owned()),
        ],
    )
    .spacing(24.0);

    let header = {
        let mut header = PageHeader::new("Jobs")
            .description("Pipelines deste servidor, com o último build de cada um.")
            .badge(Badge::new(jobs.len().to_string()));
        if s.admin() {
            let new_job = s.clone();
            header = header.action(Button::new("Novo job").icon(IconName::Plus).on_click(move || {
                new_job.config_job.set(None);
                new_job.config_new.set(true);
                new_job.go(Pagina::Configuracao);
            }));
        }
        header
    };

    // A plugin switched off that a build in the queue waits for (I5).
    let notice: AnyView =
        match waiting.iter().find_map(|b| b["waiting"]["plugin"].as_str().map(str::to_owned)) {
            Some(plugin) => {
                let how_many = waiting.iter().filter(|b| b["waiting"]["plugin"] == plugin.as_str()).count();
                let open = s.clone();
                Box::new(
                    Alert::message(format!(
                        "O plugin {plugin} está desligado: {how_many} {} na fila por ele.",
                        if how_many == 1 { "build espera" } else { "builds esperam" }
                    ))
                    .tone(Tone::Warning)
                    .compact()
                    .action("Abrir plugins", move || open.go(Pagina::Plugins)),
                )
            }
            None => nada(),
        };

    let rows: Vec<Vec<Cell>> = jobs.iter().map(|job| row_of(job, &waiting, now)).collect();
    let rows_shown = rows.len().clamp(1, 12);
    let list: AnyView = if s.phone() {
        Box::new(VStack::new(rows.iter().map(|row| phone_card(s, row)).collect::<Vec<_>>()).spacing(12.0))
    } else {
        let opener = {
            let s = s.clone();
            move |row: &Vec<Cell>| open_last(&s, &row[0].key_text())
        };
        let grid = DataGrid::new(
            GridData::new(rows),
            [
                GridColumn::text("Job").hidden(),
                GridColumn::text("Estado").width(130.0).template(|value| status_badge(&value.key_text())),
                GridColumn::text("Job").fill().template(|value| job_name(&value.key_text())),
                GridColumn::text("Último build").width(110.0),
                GridColumn::text("Gatilho").width(190.0),
                GridColumn::text("Agente").width(160.0),
                GridColumn::text("Duração").width(110.0).align(CellAlign::End),
                GridColumn::text("Últimos 10").width(140.0).template(|value| sparkline(&value.key_text())),
                GridColumn::text("Quando").width(100.0),
            ],
        )
        .key_column(0)
        .selected_keys(s.job_keys.binding())
        .striped(true)
        .empty_text("Nenhum job ainda. Crie um com Novo job.")
        .name_column(2)
        .on_row_open(opener)
        // As tall as its rows, as T1 draws it: the bar (50), the titles and the filters (38 each)
        // and 36 a row; past twelve rows it scrolls.
        .height(GRID_TOP + 36.0 * rows_shown as f32 + 1.0);
        Box::new(grid)
    };
    Box::new(
        VStack::new((header, numbers, notice, list))
            .spacing(24.0)
            .padding(if s.phone() { Insets::uniform(16.0) } else { super::PAGE_PADDING })
            .align(Align::Stretch),
    )
}

/// The agents ready and known, and in words what they are: the executors of the server, the
/// agents connected and the templates whose launcher is on.
fn agents_summary(server: &Json, occupancy: &Json) -> (usize, usize, String) {
    let executors = usize::try_from(server["executors"].as_u64().unwrap_or(0)).unwrap_or(0);
    let agents = occupancy["agents"].as_array().cloned().unwrap_or_default();
    let templates = occupancy["templates"].as_array().cloned().unwrap_or_default();
    let connected = agents.iter().filter(|a| a["connected"] == true).count();
    let launchable = templates.iter().filter(|t| t["launcher_on"] == true).count();
    let mut kinds = Vec::new();
    if executors > 0 {
        kinds.push("o servidor".to_owned());
    }
    if !agents.is_empty() {
        kinds.push(if agents.len() == 1 {
            "1 agente".to_owned()
        } else {
            format!("{} agentes", agents.len())
        });
    }
    kinds.extend(templates.iter().filter_map(|t| t["name"].as_str().map(str::to_owned)));
    let said = if kinds.is_empty() { "Só os executores do servidor".to_owned() } else { join_words(&kinds) };
    (
        usize::from(executors > 0) + connected + launchable,
        usize::from(executors > 0) + agents.len() + templates.len(),
        said,
    )
}

/// `a, b e c`.
fn join_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => capitalized(one),
        [rest @ .., last] => capitalized(&format!("{} e {last}", rest.join(", "))),
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The row of a job: its key, status, name, last build, trigger, agent, duration, trend, when.
fn row_of(job: &Json, waiting: &[Json], now: i64) -> Vec<Cell> {
    let last = &job["last"];
    let name = job["name"].as_str().unwrap_or_default().to_owned();
    let status = last["status"].as_str().unwrap_or("none").to_owned();
    let trend: Vec<String> = job["trend_ms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_i64)
        .map(|ms| ms.to_string())
        .collect();
    let agent = match waiting.iter().find(|b| b["job"] == name.as_str()) {
        Some(build) if status == "queued" => {
            build["waiting"]["why"].as_str().map(short_wait).unwrap_or_else(|| "na fila".to_owned())
        }
        _ => last["worker"].as_str().map(agent_words).unwrap_or_else(|| "—".to_owned()),
    };
    vec![
        name.clone().into(),
        status.into(),
        name.into(),
        last["number"].as_u64().map_or_else(|| "—".to_owned(), |n| format!("#{n}")).into(),
        last["cause"].as_str().unwrap_or("Nunca rodou").to_owned().into(),
        agent.into(),
        last["duration_ms"]
            .as_i64()
            .filter(|_| last["status"] != "queued")
            .map_or_else(|| "—".to_owned(), texto::duration)
            .into(),
        trend.join(",").into(),
        last["created_ms"].as_i64().map_or_else(|| "—".to_owned(), |t| texto::ago(t, now)).into(),
    ]
}

/// Who ran a build, as the grid says it: `executor-1` of the server, an agent by its name.
pub(super) fn agent_words(worker: &str) -> String {
    worker.strip_prefix("agente/").map_or_else(|| format!("servidor · {worker}"), str::to_owned)
}

/// The reason a build waits, short enough for a cell: `espera rótulo docker`.
fn short_wait(why: &str) -> String {
    match why.split_once("rótulo ") {
        Some((_, rest)) => format!("espera rótulo {}", rest.split([' ', ',']).next().unwrap_or(rest)),
        None => why.to_owned(),
    }
}

/// A job's name as the grid shows it: what is above it, then the job, muted (`apps · site`).
fn job_name(name: &str) -> AnyView {
    match name.rsplit_once('/') {
        Some((above, own)) => Box::new(
            HStack::new((
                Text::new(above.replace('/', " · ")).font_weight(Weight::SEMIBOLD),
                Text::new(format!("· {own}")).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        ),
        None => Box::new(Text::new(name.to_owned()).font_weight(Weight::SEMIBOLD)),
    }
}

/// A job on a phone (T7): a card with its status, its name and its last build, that opens it.
fn phone_card(s: &Rc<S>, row: &[Cell]) -> AnyView {
    let key = row[0].key_text();
    let open = {
        let (s, key) = (s.clone(), key.clone());
        move || open_last(&s, &key)
    };
    Box::new(
        Card::new(
            VStack::new((
                HStack::new((job_name(&key), Spacer::new(), status_badge(&row[1].key_text())))
                    .align(Align::Center),
                Text::new(format!("{} · {} · {}", row[3].key_text(), row[4].key_text(), row[8].key_text()))
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(6.0)
            .align(Align::Stretch),
        )
        .padding(16.0)
        .on_click(open),
    )
}

/// How tall a grid is above its rows: the bar, the titles and the filters.
const GRID_TOP: f32 = 50.0 + 38.0 + 38.0;

/// A number of the top of the page, in a card.
fn stat(label: &str, value: String, detail: String) -> AnyView {
    Box::new(Card::new(Stat::new(label, value).caption(detail)))
}

/// The badge of a build's status.
pub(super) fn status_badge(status: &str) -> AnyView {
    let badge = Badge::new(texto::status_words(status))
        .tone(texto::status_tone(status))
        .icon(texto::status_icon(status));
    // An interrupted build has a color of its own, apart from failing and cancelling.
    Box::new(if status == "interrupted" { badge.color(texto::INTERRUPTED) } else { badge })
}

/// The durations of the last builds, oldest first, as a line.
fn sparkline(values: &str) -> AnyView {
    let points: Vec<f64> =
        values.split(',').filter_map(|v| v.parse::<f64>().ok()).map(|ms| ms / 1000.0).collect();
    if points.len() < 2 {
        return Box::new(
            Text::new(if points.is_empty() { "—" } else { "1 build" }).foreground(Semantic::InkMuted),
        );
    }
    Box::new(
        Chart::new(ChartKind::Sparkline)
            .series("Duração (s)", points)
            .size(120.0, 24.0)
            .hide_legend()
            .alt("Duração dos últimos builds"),
    )
}

/// Opens the last build of `job`, or says it never ran.
fn open_last(s: &Rc<S>, job: &str) {
    let overview = s.overview.get();
    let last = overview["jobs"]
        .as_array()
        .and_then(|jobs| jobs.iter().find(|j| j["name"] == job))
        .and_then(|j| j["last"]["number"].as_u64());
    match last {
        Some(number) => s.go(Pagina::Build(job.to_owned(), number)),
        None => s.message.set(format!("{job} ainda não rodou. Use Disparar.")),
    }
}

/// The parameters `job` asks for, as the form shows them.
fn fields_of(s: &S, job: &str) -> Vec<ParamField> {
    let overview = s.overview.get();
    let parameters = overview["jobs"]
        .as_array()
        .and_then(|jobs| jobs.iter().find(|j| j["name"] == job))
        .and_then(|j| j["parameters"].as_array().cloned())
        .unwrap_or_default();
    parameters
        .iter()
        .map(|p| {
            let name = p["name"].as_str().unwrap_or_default();
            let caption = p["caption"].as_str().filter(|c| !c.is_empty()).unwrap_or(name);
            // A field says the parameter's name, as the steps read it (BRANCH, E2E); a switch says
            // what it does (T6).
            let label = if p["kind"] == "bool" { caption } else { name };
            let kind = match p["kind"].as_str() {
                Some("bool") => ParamKind::Boolean,
                Some("choice") => ParamKind::Choice(
                    p["choices"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Json::as_str)
                        .map(|c| (c.to_owned(), c.to_owned()))
                        .collect(),
                ),
                Some("choices") => ParamKind::MultiChoice(
                    p["choices"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Json::as_str)
                        .map(|c| (c.to_owned(), c.to_owned()))
                        .collect(),
                ),
                _ => ParamKind::Text,
            };
            let field = ParamField::new(name, label, kind);
            match &p["default"] {
                Json::Bool(b) => field.default(*b),
                Json::String(t) if !t.is_empty() => field.default(t.clone()),
                list @ Json::Array(_) => field.default(list.clone()),
                _ => field,
            }
        })
        .collect()
}

/// Asks for a build of `job`: with no parameters it goes straight to the queue, with some the
/// dialog asks for them first (I1).
pub(super) fn ask_build(s: &Rc<S>, job: &str) {
    if fields_of(s, job).is_empty() {
        send_build(s, job, serde_json::Map::new());
        return;
    }
    s.dispatch_job.set(job.to_owned());
    s.dispatch_values.set(ParamValues::new());
    s.dispatch_error.set(String::new());
    s.dispatch_open.set(true);
}

/// Sends the request for a build of `job` to the plugin api-trigger.
fn send_build(s: &Rc<S>, job: &str, parameters: serde_json::Map<String, Json>) {
    let job = job.to_owned();
    s.send("/api/ci/run", &json!({ "job": job, "parameters": parameters }), move |s, status, body| {
        if (200..300).contains(&status) {
            let number = body["number"].as_u64().unwrap_or(0);
            s.dispatch_open.set(false);
            s.message.set(format!("Build {number} de {job} na fila."));
            s.go(s.page.get());
        } else {
            let why = body["message"].as_str().unwrap_or("o servidor recusou").to_owned();
            s.dispatch_error.set(why.clone());
            s.message.set(why);
        }
    });
}

/// The dialog of I1: the parameters of the job, Cancelar and Disparar (⌘ Enter).
pub(super) fn dispatch_dialog(s: &Rc<S>) -> AnyView {
    if !s.dispatch_open.get() {
        return nada();
    }
    let job = s.dispatch_job.get();
    let fields = fields_of(s, &job);
    let fire = {
        let (s, job, fields) = (s.clone(), job.clone(), fields.clone());
        move || {
            let values = s.dispatch_values.get();
            let mut parameters = serde_json::Map::new();
            for field in &fields {
                let value = values
                    .0
                    .get(&field.name)
                    .cloned()
                    .or_else(|| field.default.clone())
                    .unwrap_or(Json::Null);
                parameters.insert(field.name.clone(), value);
            }
            send_build(&s, &job, parameters);
        }
    };
    let close = {
        let s = s.clone();
        move || s.dispatch_open.set(false)
    };
    let error = s.dispatch_error.get();
    let problem: AnyView = if error.is_empty() {
        nada()
    } else {
        Box::new(Alert::new("Não entrou na fila", error).tone(Tone::Danger))
    };
    let ahead = s.overview.get()["queued"].as_u64().unwrap_or(0);
    let queue = Alert::message(match ahead {
        0 => "Ninguém na fila: começa assim que um executor ficar livre.".to_owned(),
        1 => "1 build na fila antes deste".to_owned(),
        n => format!("{n} builds na fila antes deste"),
    })
    .tone(Tone::Neutral)
    .compact();
    Box::new(
        Dialog::new(s.dispatch_open.binding(), format!("Disparar {job}"))
            .description("Os parâmetros vêm do pipeline. O build entra na fila do servidor.")
            .width(460.0)
            .close_on_backdrop(false)
            .content(
                VStack::new((
                    ParameterForm::new(fields, s.dispatch_values.binding()).live(true),
                    queue,
                    problem,
                ))
                .spacing(12.0)
                .align(Align::Stretch),
            )
            .action(Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(close))
            .action(Button::new("Disparar").icon(IconName::Play).shortcut("⌘ Enter").on_click(fire)),
    )
}
