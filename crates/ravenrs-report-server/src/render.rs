//! Resolves a layout's data and parameters, and writes its output files.

use std::collections::{BTreeMap, HashMap};
use std::io::Read as _;
use std::time::Duration;

use jiff::Zoned;
use ravenrs_report::{Criteria, DataSourceKind, DateTime, RenderOptions, Report, ReportEngine, Value};
use serde_json::{Value as Json, json};

use crate::config::{Config, DataHost};
use crate::job::{Format, ParameterValue};

/// The largest response a `url` data source may return.
const MAX_DATA_BYTES: u64 = 64 * 1024 * 1024;
/// How long a `url` data source may take.
const DATA_TIMEOUT: Duration = Duration::from_secs(60);

/// A produced file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputFile {
    /// Its format.
    pub format: Format,
    /// Its name.
    pub name: String,
    /// Its media type.
    pub mime: &'static str,
    /// Its content.
    pub bytes: Vec<u8>,
}

/// What a render produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    /// The report's title.
    pub title: String,
    /// The files, one per format.
    pub files: Vec<OutputFile>,
    /// How many rows the report printed.
    pub rows: usize,
    /// Problems in the report; the files are still produced.
    pub warnings: Vec<String>,
}

/// The media type of a format.
pub fn mime(format: Format) -> &'static str {
    match format {
        Format::Pdf => "application/pdf",
        Format::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Format::Csv => "text/csv; charset=utf-8",
    }
}

/// A date as a report parameter value.
fn date_text(d: DateTime) -> String {
    let (year, month, day) = d.ymd();
    let (hour, minute, second) = d.hms();
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// A moment as the local date and time reports see.
pub fn local_date_time(now: &Zoned) -> Option<DateTime> {
    let mut date = DateTime::from_ymd(i32::from(now.year()), now.month() as u32, now.day() as u32)?;
    date.seconds = now.hour() as u32 * 3600 + now.minute() as u32 * 60 + now.second() as u32;
    Some(date)
}

/// Computes a job's parameter values for a run at `now`.
pub fn parameter_values(
    parameters: &BTreeMap<String, ParameterValue>,
    now: DateTime,
) -> Result<HashMap<String, Json>, String> {
    let env = ravenrs_report::criteria::Environment::new().at(now);
    let mut values = HashMap::new();
    for (name, value) in parameters {
        let json = match value {
            ParameterValue::Fixed(json) => json.clone(),
            ParameterValue::Expression { expression } => {
                let criteria = Criteria::parse(expression).map_err(|e| format!("parameter {name}: {e}"))?;
                let scope = ravenrs_report::criteria::Scope::<Json>::new(&env);
                match scope.evaluate(&criteria).map_err(|e| format!("parameter {name}: {e}"))? {
                    Value::Null => Json::Null,
                    Value::Bool(b) => json!(b),
                    Value::Number(n) => json!(n),
                    Value::Date(d) => json!(date_text(d)),
                    other => json!(other.to_text()),
                }
            }
        };
        values.insert(name.clone(), json);
    }
    Ok(values)
}

/// The data for a layout: stored in it, or fetched from an allowed host.
pub fn resolve_data(
    report: &Report,
    parameters: &HashMap<String, Json>,
    config: &Config,
) -> Result<Json, String> {
    let source = match &report.data_source {
        Some(name) => report
            .data_sources
            .iter()
            .find(|s| &s.name == name)
            .ok_or_else(|| format!("the data source '{name}' is not in the layout"))?,
        None => match report.data_sources.first() {
            Some(source) => source,
            None => return Ok(Json::Null),
        },
    };
    match &source.kind {
        DataSourceKind::Json { data } => Ok(data.clone()),
        DataSourceKind::Url { url, headers } => fetch(url, headers, parameters, &config.data_hosts),
        DataSourceKind::Sql { connection, .. } => {
            Err(format!("SQL data sources are not supported yet (connection '{connection}')"))
        }
        _ => Err("this kind of data source is not supported".to_owned()),
    }
}

/// Percent-encodes everything but unreserved characters, so a value cannot
/// change which host or path a URL points at.
fn encode_component(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Fills in `{?Name}` with encoded parameter values.
fn fill_url(url: &str, parameters: &HashMap<String, Json>) -> String {
    let mut out = url.to_owned();
    for (name, value) in parameters {
        let text = match value {
            Json::String(s) => s.clone(),
            other => other.to_string(),
        };
        out = out.replace(&format!("{{?{name}}}"), &encode_component(&text));
    }
    out
}

/// The allowed host a URL points at. Layouts can come from anyone with API
/// access, so only hosts in the server's configuration can be reached.
fn allowed_host<'c>(url: &str, hosts: &'c [DataHost]) -> Result<&'c DataHost, String> {
    let uri: ureq::http::Uri = url.parse().map_err(|_| format!("'{url}' is not a valid URL"))?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) {
        return Err(format!("'{url}' must use http or https"));
    }
    let authority = uri.authority().map_or("", |a| a.as_str());
    if authority.contains('@') {
        return Err(format!("'{url}' must not contain credentials"));
    }
    let host = uri.host().unwrap_or_default();
    hosts
        .iter()
        .find(|allowed| allowed.host.eq_ignore_ascii_case(host))
        .ok_or_else(|| format!("the host '{host}' is not in the server's dataHosts"))
}

fn fetch(
    url: &str,
    headers: &BTreeMap<String, String>,
    parameters: &HashMap<String, Json>,
    hosts: &[DataHost],
) -> Result<Json, String> {
    let url = fill_url(url, parameters);
    let host = allowed_host(&url, hosts)?;
    // A redirect could lead away from the allowed host, so none is followed.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(DATA_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.get(&url).header("Accept", "application/json");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    for (name, variable) in &host.headers_env {
        let value =
            std::env::var(variable).map_err(|_| format!("the environment variable {variable} is not set"))?;
        request = request.header(name, &value);
    }
    let mut response = request.call().map_err(|e| format!("could not read {}: {e}", host.host))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{} answered {status}", host.host));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_DATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("could not read {}: {e}", host.host))?;
    if bytes.len() as u64 > MAX_DATA_BYTES {
        return Err(format!("{} returned more than {} MB", host.host, MAX_DATA_BYTES / (1024 * 1024)));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("{} did not return JSON: {e}", host.host))
}

/// Renders `report` over `data` into one file per format. CPU-bound: run it off the async threads.
pub fn render(
    report: &Report,
    data: &Json,
    parameters: HashMap<String, Json>,
    now: DateTime,
    formats: &[Format],
    date_label: &str,
) -> Result<Rendered, String> {
    let options = RenderOptions { parameters, now: Some(now) };
    let mut engine = ReportEngine::new();
    let rows = engine.count_rows(report, data, &options);
    let document = engine.render_with(report, data, &options);
    let mut files = Vec::with_capacity(formats.len());
    for format in formats {
        let bytes = match format {
            Format::Pdf => ravenrs_report_pdf::to_pdf(&document).map_err(|e| e.to_string())?,
            Format::Xlsx => ravenrs_report_xlsx::to_xlsx(&document).map_err(|e| e.to_string())?,
            // A byte order mark, so Excel reads the accents as UTF-8.
            Format::Csv => format!("\u{feff}{}", ravenrs_report_xlsx::to_csv(&document)).into_bytes(),
        };
        files.push(OutputFile {
            format: *format,
            name: crate::store::document_file_name(&document.title, date_label, *format),
            mime: mime(*format),
            bytes,
        });
    }
    Ok(Rendered { title: document.title, files, rows, warnings: document.errors })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts() -> Vec<DataHost> {
        vec![DataHost { host: "api.example.com".to_owned(), headers_env: BTreeMap::new() }]
    }

    #[test]
    fn only_configured_hosts_can_be_reached() {
        let hosts = hosts();
        assert!(allowed_host("https://api.example.com/x", &hosts).is_ok());
        assert!(allowed_host("https://API.example.com/x", &hosts).is_ok());
        for bad in [
            "https://evil.example.com/x",
            "http://169.254.169.254/latest/meta-data",
            "http://localhost/",
            "file:///etc/passwd",
            "https://api.example.com@evil.com/",
            "https://evil.com/api.example.com",
            "api.example.com/x",
        ] {
            assert!(allowed_host(bad, &hosts).is_err(), "{bad}");
        }
    }

    #[test]
    fn parameter_values_cannot_change_the_host() {
        let values =
            HashMap::from([("Id".to_owned(), json!("x@evil.com/../a b")), ("N".to_owned(), json!(5))]);
        let url = fill_url("https://{?Id}/items?n={?N}&id={?Id}", &values);
        assert_eq!(url, "https://x%40evil.com%2F..%2Fa%20b/items?n=5&id=x%40evil.com%2F..%2Fa%20b");
        assert!(allowed_host(&url, &hosts()).is_err());
    }

    #[test]
    fn expressions_compute_values_at_the_run_time() -> Result<(), String> {
        let now = DateTime::from_ymd(2026, 3, 31).ok_or("date")?;
        let parameters = BTreeMap::from([
            ("Fixed".to_owned(), ParameterValue::Fixed(json!("Sul"))),
            (
                "From".to_owned(),
                ParameterValue::Expression { expression: "AddMonths(Today(), -1)".to_owned() },
            ),
            ("Sum".to_owned(), ParameterValue::Expression { expression: "1 + 2".to_owned() }),
        ]);
        let values = parameter_values(&parameters, now)?;
        assert_eq!(values["Fixed"], json!("Sul"));
        assert_eq!(values["From"], json!("2026-02-28T00:00:00"));
        assert_eq!(values["Sum"], json!(3.0));
        Ok(())
    }
}
