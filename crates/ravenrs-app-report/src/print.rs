//! Stored reports as things to hand out: on these records, this many of each, to the printer or to a
//! file. Labels are the usual case, and a report of any size can go the same way.

use ravenrs_app::{Object, Value};
use ravenrs_app_ui::Session;
use ravenrs_report::{Document, ReportEngine};
use ravenrs_report_label::{LabelJob, compile};

use crate::stored::{REPORT_DEFINITION, ReportWords, design_report, every_record, report_of, rows_of};

/// The reports kept for this entity, by what people call them.
pub fn stored_reports_for(session: &Session, entity: &str) -> Vec<Object> {
    every_record(session, REPORT_DEFINITION, None)
        .into_iter()
        .filter(|report| report.get("target_entity") == Value::Text(entity.to_owned()))
        .collect()
}

/// Opens the designer on a stored report, for changing its layout.
pub fn design_stored_report(session: &Session, report: &Object, words: &ReportWords) -> Result<(), String> {
    design_report(session, report, words)
}

/// Each record as many times as asked, as the rows of a report.
fn repeated(records: &[(Object, u32)]) -> Vec<Object> {
    records.iter().flat_map(|(record, copies)| std::iter::repeat_n(record.clone(), *copies as usize)).collect()
}

/// A stored report over these records, each as many times as asked.
pub fn render_stored_report(
    session: &Session,
    report: &Object,
    records: &[(Object, u32)],
    words: &ReportWords,
) -> Result<Document, String> {
    let (layout, entity) = report_of(session, report, words)?;
    let rows = rows_of(session, &entity, &repeated(records));
    let document = ReportEngine::new().render(&layout, &rows);
    match document.errors.first() {
        Some(problem) => Err(problem.clone()),
        None => Ok(document),
    }
}

/// What a label printer takes for this document.
pub fn label_bytes(document: &Document, job: &LabelJob) -> Result<Vec<u8>, String> {
    compile(document, job).map_err(|error| error.to_string())
}

/// Sends what a printer takes, out of the way of the screens, and says how it went.
#[cfg(not(target_arch = "wasm32"))]
pub fn send_to_printer(
    session: &Session,
    title: impl Into<String>,
    bytes: Vec<u8>,
    output: impl ravenrs_report_label::LabelOutput + Send + 'static,
    done: impl FnOnce(&Session, Result<(), String>) + 'static,
) {
    session.run_background(
        title,
        move |_progress| output.send(&bytes).map_err(|error| error.to_string()),
        done,
    );
}
