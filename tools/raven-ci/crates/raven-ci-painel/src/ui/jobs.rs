//! The jobs (P-01, #885; page T1 and interaction I1): four numbers, the folders in a tree, every
//! job in a grid with its last build, its duration and the trend of the last ones, and the dialog
//! that asks for a build with its parameters.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, nada};
use crate::texto;

/// A cell of the grid.
type Cell = ravenrs::prelude::Value;

/// The page of the jobs.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let overview = s.overview.get();
    let server = s.server.get();
    let now = overview["now_ms"].as_i64().unwrap_or(0);
    let jobs: Vec<Json> = overview["jobs"].as_array().cloned().unwrap_or_default();
    let folder = s.folder.get();
    let shown: Vec<&Json> = jobs
        .iter()
        .filter(|job| match &folder {
            None => true,
            Some(f) => {
                job["folder"].as_str().is_some_and(|mine| mine == f || mine.starts_with(&format!("{f}/")))
            }
        })
        .collect();

    let queued = overview["queued"].as_u64().unwrap_or(0);
    let oldest = overview["oldest_queued_ms"].as_i64().map_or_else(
        || "Ninguém esperando".to_owned(),
        |t| format!("O mais antigo espera {}", texto::ago(t, now)),
    );
    let running = overview["running"].as_u64().unwrap_or(0);
    let executors = server["executors"].as_u64().unwrap_or(0);
    let memory = server["memory_bytes"].as_u64().map_or_else(|| "—".to_owned(), texto::bytes);
    let numbers = Grid::new(
        4,
        vec![
            stat("NA FILA", queued.to_string(), oldest),
            stat("RODANDO", running.to_string(), "Builds em andamento agora".to_owned()),
            stat(
                "EXECUTORES",
                format!("{} de {executors}", executors.saturating_sub(running.min(executors))),
                "Livres agora".to_owned(),
            ),
            stat("MEMÓRIA DO SERVIDOR", memory, "Lida do processo, ao vivo".to_owned()),
        ],
    )
    .spacing(12.0);

    let rows: Vec<Vec<Cell>> = shown
        .iter()
        .map(|job| {
            let last = &job["last"];
            let status = last["status"].as_str().unwrap_or("none").to_owned();
            let name = job["name"].as_str().unwrap_or_default().to_owned();
            let title = job["title"].as_str().map_or_else(|| name.clone(), str::to_owned);
            let trend: Vec<String> = job["trend_ms"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_i64)
                .map(|ms| ms.to_string())
                .collect();
            vec![
                name.into(),
                status.into(),
                title.into(),
                last["number"].as_u64().map_or_else(|| "—".to_owned(), |n| format!("#{n}")).into(),
                last["cause"].as_str().unwrap_or("Nunca rodou").to_owned().into(),
                last["duration_ms"].as_i64().map_or_else(|| "—".to_owned(), texto::duration).into(),
                trend.join(",").into(),
                last["created_ms"].as_i64().map_or_else(|| "—".to_owned(), |t| texto::ago(t, now)).into(),
            ]
        })
        .collect();
    let opener = {
        let s = s.clone();
        move |row: &Vec<Cell>| open_last(&s, &row[0].key_text())
    };
    let grid = DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Job").hidden(),
            GridColumn::text("Estado").width(130.0).template(|value| status_badge(&value.key_text())),
            GridColumn::text("Job").fill(),
            GridColumn::text("Último build").width(110.0),
            GridColumn::text("Gatilho").width(200.0),
            GridColumn::text("Duração").width(120.0),
            GridColumn::text("Últimos 10").width(140.0).template(|value| sparkline(&value.key_text())),
            GridColumn::text("Quando").width(110.0),
        ],
    )
    .key_column(0)
    .selected_keys(s.job_keys.binding())
    .striped(true)
    .empty_text("Nenhum job ainda. Grave um pipeline em Configuração.")
    .on_row_open(opener)
    .height(420.0);

    let tree = folders_tree(s, &overview);
    let actions = {
        let (open, dispatch) = (s.clone(), s.clone());
        HStack::new((
            Button::new("Abrir o último build")
                .icon(IconName::FileText)
                .style(ButtonStyle::Secondary)
                .on_click(move || match open.job_keys.get().into_iter().next() {
                    Some(job) => open_last(&open, &job),
                    None => open.message.set("Escolha um job na grade.".to_owned()),
                }),
            Button::new("Disparar").icon(IconName::Play).shortcut("D").on_click(move || {
                match dispatch.job_keys.get().into_iter().next() {
                    Some(job) => ask_build(&dispatch, &job),
                    None => dispatch.message.set("Escolha na grade o job a disparar.".to_owned()),
                }
            }),
        ))
        .spacing(8.0)
    };
    let header = PageHeader::new("Jobs")
        .description("Pipelines deste servidor, com o último build de cada um.")
        .badge(Badge::new(jobs.len().to_string()))
        .action(actions);
    let waiting = jobs.iter().filter(|j| j["last"]["status"] == "queued").count();
    let notice: AnyView = if waiting > 0 && running == 0 && executors > 0 {
        Box::new(
            Alert::message(format!(
                "{waiting} job(s) na fila e nenhum rodando: veja a fila e os executores."
            ))
            .tone(Tone::Warning),
        )
    } else {
        nada()
    };
    Box::new(
        VStack::new((
            header,
            numbers,
            notice,
            HStack::new((tree, VStack::new((grid,)).align(Align::Stretch).grow(1.0)))
                .spacing(16.0)
                .align(Align::Start),
        ))
        .spacing(20.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// A number of the top of the page, in a card.
fn stat(label: &str, value: String, detail: String) -> AnyView {
    Box::new(
        Card::new(
            VStack::new((Stat::new(label, value), Text::new(detail).foreground(Semantic::InkMuted)))
                .spacing(4.0),
        )
        .padding(16.0),
    )
}

/// The badge of a build's status.
pub(super) fn status_badge(status: &str) -> AnyView {
    Box::new(
        Badge::new(texto::status_words(status))
            .tone(texto::status_tone(status))
            .icon(texto::status_icon(status)),
    )
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

/// The folders of jobs as a tree; choosing one shows only its jobs.
fn folders_tree(s: &Rc<S>, overview: &Json) -> AnyView {
    let mut root = TreeNode::new("", "Todos os jobs").icon(IconName::Layers);
    let folders = overview["folders"].as_array().cloned().unwrap_or_default();
    let mut paths: Vec<String> =
        folders.iter().filter_map(|f| f["path"].as_str().map(str::to_owned)).collect();
    paths.sort();
    // Each folder under its parent: the deepest are put in first, then hung from the one above.
    let mut nodes: Vec<(String, TreeNode)> = Vec::new();
    for path in paths.iter().filter(|p| !p.is_empty()) {
        let label = path.rsplit('/').next().unwrap_or(path);
        let count = folders
            .iter()
            .find(|f| f["path"] == path.as_str())
            .and_then(|f| f["jobs"].as_array().map(Vec::len))
            .unwrap_or(0);
        nodes.push((
            path.clone(),
            TreeNode::new(path.clone(), format!("{label} ({count})")).icon(IconName::Package),
        ));
    }
    while let Some((path, node)) = nodes.pop() {
        match path.rsplit_once('/').map(|(parent, _)| parent.to_owned()) {
            Some(parent) if nodes.iter().any(|(p, _)| *p == parent) => {
                if let Some((_, above)) = nodes.iter_mut().find(|(p, _)| *p == parent) {
                    *above = std::mem::replace(above, TreeNode::new("", "")).child(node);
                }
            }
            _ => root = root.child(node),
        }
    }
    let chosen = s.folder.get().unwrap_or_default();
    let selected = State::new(Some(chosen));
    let choose = {
        let s = s.clone();
        move |key: &str| s.folder.set((!key.is_empty()).then(|| key.to_owned()))
    };
    let expanded = State::new(paths.iter().cloned().chain([String::new()]).collect());
    Box::new(
        VStack::new((
            Text::new("PASTAS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            TreeView::new(vec![root])
                .selected(selected.binding())
                .expanded(expanded.binding())
                .on_open(choose)
                .label("Pastas de jobs")
                .height(300.0),
        ))
        .spacing(8.0)
        .width(220.0),
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
                _ => ParamKind::Text,
            };
            let field = ParamField::new(name, caption, kind);
            match &p["default"] {
                Json::Bool(b) => field.default(*b),
                Json::String(t) if !t.is_empty() => field.default(t.clone()),
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
    Box::new(
        Dialog::new(s.dispatch_open.binding(), format!("Disparar {job}"))
            .description("Os parâmetros vêm do pipeline. O build entra na fila do servidor.")
            .width(460.0)
            .close_on_backdrop(false)
            .content(
                VStack::new((ParameterForm::new(fields, s.dispatch_values.binding()).live(true), problem))
                    .spacing(12.0)
                    .align(Align::Stretch),
            )
            .action(Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(close))
            .action(Button::new("Disparar").icon(IconName::Play).shortcut("⌘ Enter").on_click(fire)),
    )
}
