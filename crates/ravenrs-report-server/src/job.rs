//! Scheduled jobs: what to render, when, and where it goes.

use std::collections::{BTreeMap, BTreeSet};

use jiff::tz::TimeZone;
use ravenrs_report::Criteria;
use serde::{Deserialize, Serialize};

use crate::cron::Schedule;
use crate::store::valid_name;

/// The most recipients one delivery may have.
const MAX_RECIPIENTS: usize = 500;
/// The most retries a job may ask for.
const MAX_RETRIES: u32 = 10;

/// A scheduled report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Job {
    /// The stored layout to render, by name; its latest version at each run.
    pub layout: String,
    /// When to run, as five cron fields.
    pub schedule: String,
    /// The time zone of the schedule and of `Today()`, such as `America/Recife`.
    #[serde(default = "utc")]
    pub timezone: String,
    /// Parameter values, fixed or computed at each run.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterValue>,
    /// The files to produce.
    pub formats: Vec<Format>,
    /// Where they go.
    pub deliveries: Vec<Delivery>,
    /// Sends nothing when the report has no rows.
    #[serde(default)]
    pub skip_if_empty: bool,
    /// How many times to try again after a failure.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Keeps each run's files on the server for this many days; zero keeps none.
    #[serde(default)]
    pub retention_days: u32,
    /// A disabled job runs only when asked.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn utc() -> String {
    "UTC".to_owned()
}

fn default_retries() -> u32 {
    2
}

fn enabled() -> bool {
    true
}

/// A parameter's value in a job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    /// Computed when the job runs, such as `AddMonths(Today(), -1)`.
    Expression {
        /// The expression.
        expression: String,
    },
    /// A fixed value.
    Fixed(serde_json::Value),
}

/// An output file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// PDF.
    Pdf,
    /// Excel workbook.
    Xlsx,
    /// Comma- or semicolon-separated text.
    Csv,
}

/// Where a job's files go.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Delivery {
    /// As email attachments.
    Email {
        /// The recipients.
        to: Vec<String>,
        /// Sends one message per recipient, who then do not see each other.
        #[serde(default)]
        separately: bool,
        /// The subject; `{report}` and `{date}` are filled in.
        #[serde(default)]
        subject: Option<String>,
        /// The text of the message; `{report}` and `{date}` are filled in.
        #[serde(default)]
        body: Option<String>,
    },
    /// As files in a folder under the server's `outputDir`.
    Folder {
        /// The folder, relative to `outputDir`.
        path: String,
    },
}

impl Job {
    /// Checks everything that can be checked before the job runs.
    pub fn validate(&self) -> Result<(), String> {
        if !valid_name(&self.layout) {
            return Err(format!("'{}' is not a valid layout name", self.layout));
        }
        Schedule::parse(&self.schedule)?;
        TimeZone::get(&self.timezone).map_err(|_| format!("unknown time zone '{}'", self.timezone))?;
        if self.formats.is_empty() {
            return Err("a job needs at least one format".to_owned());
        }
        if self.formats.iter().collect::<BTreeSet<_>>().len() != self.formats.len() {
            return Err("a format is listed twice".to_owned());
        }
        if self.deliveries.is_empty() {
            return Err("a job needs at least one delivery".to_owned());
        }
        if self.retries > MAX_RETRIES {
            return Err(format!("a job can retry at most {MAX_RETRIES} times"));
        }
        for (name, value) in &self.parameters {
            if let ParameterValue::Expression { expression } = value {
                Criteria::parse(expression).map_err(|e| format!("parameter {name}: {e}"))?;
            }
        }
        for delivery in &self.deliveries {
            match delivery {
                Delivery::Email { to, .. } => {
                    if to.is_empty() || to.len() > MAX_RECIPIENTS {
                        return Err(format!("an email delivery needs 1 to {MAX_RECIPIENTS} recipients"));
                    }
                    for address in to {
                        address
                            .parse::<lettre::message::Mailbox>()
                            .map_err(|_| format!("'{address}' is not a valid email address"))?;
                    }
                }
                Delivery::Folder { path } => {
                    safe_relative_path(path)?;
                }
            }
        }
        Ok(())
    }
}

/// The parts of a relative path, refusing anything that could leave its base.
pub(crate) fn safe_relative_path(path: &str) -> Result<Vec<&str>, String> {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".").collect();
    let bad = path.starts_with(['/', '\\'])
        || path.contains('\0')
        || path.contains(':')
        || parts.is_empty()
        || parts.iter().any(|p| *p == ".." || p.chars().any(char::is_control));
    if bad {
        return Err(format!("'{path}' must be a relative folder without '..'"));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn job() -> serde_json::Value {
        json!({
            "layout": "sales",
            "schedule": "0 8 * * MON",
            "timezone": "America/Recife",
            "parameters": { "Region": "Sul", "From": { "expression": "AddMonths(Today(), -1)" } },
            "formats": ["pdf", "xlsx"],
            "deliveries": [
                { "type": "email", "to": ["Ana <ana@example.com>"], "separately": true, "subject": "{report} {date}" },
                { "type": "folder", "path": "vendas/semanal" }
            ],
            "skipIfEmpty": true,
            "retentionDays": 30
        })
    }

    fn with(key: &str, value: serde_json::Value) -> Result<(), String> {
        let mut json = job();
        json[key] = value;
        let job: Job = serde_json::from_value(json).map_err(|e| e.to_string())?;
        job.validate()
    }

    #[test]
    fn a_complete_job_reads_and_validates() -> Result<(), String> {
        let parsed: Job = serde_json::from_value(job()).map_err(|e| e.to_string())?;
        parsed.validate()?;
        assert_eq!(parsed.retries, 2);
        assert!(parsed.enabled);
        assert_eq!(parsed.parameters["Region"], ParameterValue::Fixed(json!("Sul")));
        assert!(matches!(parsed.parameters["From"], ParameterValue::Expression { .. }));
        let again: Job = serde_json::from_str(&serde_json::to_string(&parsed).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        assert_eq!(again, parsed);
        Ok(())
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(with("schedule", json!("every monday")).is_err());
        assert!(with("timezone", json!("Mars/Olympus")).is_err());
        assert!(with("layout", json!("../secret")).is_err());
        assert!(with("formats", json!([])).is_err());
        assert!(with("formats", json!(["pdf", "pdf"])).is_err());
        assert!(with("formats", json!(["docx"])).is_err());
        assert!(with("deliveries", json!([])).is_err());
        assert!(with("deliveries", json!([{ "type": "email", "to": ["not an address"] }])).is_err());
        assert!(with("deliveries", json!([{ "type": "email", "to": [] }])).is_err());
        assert!(with("retries", json!(99)).is_err());
        assert!(with("parameters", json!({ "A": { "expression": "1 +" } })).is_err());
        assert!(with("typo", json!(1)).is_err(), "unknown fields are refused");
    }

    #[test]
    fn folders_cannot_leave_the_output_folder() {
        for bad in ["../x", "a/../../x", "/etc", "\\x", "C:\\x", "a\\..\\..\\x", "", ".", "a\0b"] {
            assert!(safe_relative_path(bad).is_err(), "{bad}");
        }
        assert_eq!(safe_relative_path("a/./b/"), Ok(vec!["a", "b"]));
    }
}
