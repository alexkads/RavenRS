//! Every build of the server, the newest first (#919): the page Histórico of the boards' menu, a
//! `DataGrid` with the status, the job, the number, the trigger, the agent, the commit, the
//! duration and when, that opens a build on Enter or a double click.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::Value as Json;

use super::{Pagina, S, jobs};
use crate::texto;

/// A cell of the grid.
type Cell = ravenrs::prelude::Value;

/// The page of the history.
pub(super) fn page(s: &Rc<S>) -> AnyView {
    let history = s.history.get();
    let now = history["now_ms"].as_i64().unwrap_or(0);
    let builds: Vec<Json> = history["builds"].as_array().cloned().unwrap_or_default();
    let rows: Vec<Vec<Cell>> = builds
        .iter()
        .map(|b| {
            let job = b["job"].as_str().unwrap_or_default().to_owned();
            let number = b["number"].as_u64().unwrap_or(0);
            vec![
                format!("{job}\u{1f}{number}").into(),
                b["status"].as_str().unwrap_or_default().to_owned().into(),
                job.into(),
                format!("#{number}").into(),
                b["cause"].as_str().unwrap_or_default().to_owned().into(),
                b["worker"].as_str().map(jobs::agent_words).unwrap_or_else(|| "—".to_owned()).into(),
                b["commit"]
                    .as_str()
                    .map(|c| c.chars().take(7).collect::<String>())
                    .unwrap_or_else(|| "—".to_owned())
                    .into(),
                b["duration_ms"]
                    .as_i64()
                    .filter(|_| b["status"] != "queued")
                    .map_or_else(|| "—".to_owned(), texto::duration)
                    .into(),
                b["created_ms"].as_i64().map_or_else(|| "—".to_owned(), |t| texto::ago(t, now)).into(),
            ]
        })
        .collect();
    let open = {
        let s = s.clone();
        move |row: &Vec<Cell>| {
            let key = row[0].key_text();
            if let Some((job, number)) =
                key.split_once('\u{1f}').and_then(|(j, n)| Some((j.to_owned(), n.parse().ok()?)))
            {
                s.go(Pagina::Build(job, number));
            }
        }
    };
    let grid = DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Build").hidden(),
            GridColumn::text("Estado").width(130.0).template(|value| jobs::status_badge(&value.key_text())),
            GridColumn::text("Job").fill(),
            GridColumn::text("Build").width(90.0),
            GridColumn::text("Gatilho").width(190.0),
            GridColumn::text("Agente").width(160.0),
            GridColumn::text("Commit").width(100.0),
            GridColumn::text("Duração").width(110.0).align(CellAlign::End),
            GridColumn::text("Quando").width(100.0),
        ],
    )
    .key_column(0)
    .striped(true)
    .empty_text("Nenhum build ainda.")
    .on_row_open(open)
    .height(560.0);
    Box::new(
        VStack::new((
            PageHeader::new("Histórico")
                .description("Todos os builds do servidor, o mais novo primeiro. Enter abre o build.")
                .badge(Badge::new(builds.len().to_string())),
            grid,
        ))
        .spacing(24.0)
        .padding(32.0)
        .align(Align::Stretch),
    )
}
