//! Jobs of many branches: one job per branch and pull request a [`BranchSource`] finds.
//!
//! # How it works
//!
//! A job whose settings say `{"branches": {"id": "github", ...}}` is a model. [`Ci::scan`] asks the
//! source its heads and keeps one job per head, `<model>/<head>` (`repo/main`, `repo/PR-12`):
//!
//! - **Its pipeline** is the file of the head named by `branches.file` (by default the first file
//!   name of the model's format, `.raven-ci.toml`), read with [`BranchSource::file`]; with none, the
//!   model's own pipeline.
//! - **Its settings** are the model's, but `branches`, with `scm` made from the head: the branch,
//!   or the pull request with `merge_onto_target` (true unless the model's `scm` says otherwise),
//!   and `$head`, what the scan last saw. The status of a pull request goes to its own commit, not
//!   to the merge, as a provider shows it on the pull request.
//! - **A build** is asked for when a head is new or its commit changed since the last scan.
//! - **A head that is gone** has its job removed once `branches.orphan_days` days passed since the
//!   scan first missed it (0, at once, unless said). Its builds stay until their retention.

use std::sync::Arc;

use raven_ci_plugin_api::{BranchSource, BuildRequest, CiError, Head, HeadKind};
use serde_json::{Map, Value, json};

use crate::{Ci, JobSpec};

/// The key of a job's settings where the scan keeps what it saw of the head.
pub const HEAD_KEY: &str = "$head";

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

impl Ci {
    fn source(&self, id: &str) -> Result<Arc<dyn BranchSource>, CiError> {
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.branch_sources())
            .find(|source| source.id() == id)
            .ok_or_else(|| {
                CiError::unavailable(format!("falta o plugin {id}: o job de muitas branches o usa"))
            })
    }

    /// Looks again at the heads of the job of many branches `job` (see the [module](self)); what
    /// changed, in words.
    pub fn scan(&self, job: &str) -> Result<Vec<String>, CiError> {
        let model = self.job(job)?;
        let branches =
            model.settings.get("branches").cloned().ok_or_else(|| {
                CiError::settings("branches", format!("o job {job} não é de muitas branches"))
            })?;
        let id = branches.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let source = self.source(&id)?;
        let heads = source.discover(&branches)?;
        let file = match branches.get("file").and_then(Value::as_str) {
            Some(file) => Some(file.to_owned()),
            None => self
                .registry
                .on()
                .iter()
                .flat_map(|plugin| plugin.pipeline_formats())
                .find(|format| format.id() == model.format)
                .and_then(|format| format.file_names().into_iter().next()),
        };
        let mut changes = Vec::new();
        let prefix = format!("{job}/");
        for head in &heads {
            let name = format!("{prefix}{}", head.name);
            let before = self.job(&name).ok();
            let seen = before
                .as_ref()
                .and_then(|child| child.settings.pointer(&format!("/{HEAD_KEY}/commit")).cloned());
            let pipeline = match &file {
                Some(file) => source.file(&branches, head, file)?.unwrap_or_else(|| model.pipeline.clone()),
                None => model.pipeline.clone(),
            };
            let mut child =
                JobSpec::new(&name, &model.format, pipeline).settings(child_settings(&model, head));
            child.title = Some(head.name.clone());
            child.keep_builds = model.keep_builds;
            child.keep_days = model.keep_days;
            self.save_job(child)?;
            let moved = seen.as_ref().and_then(Value::as_str) != Some(head.commit.as_str());
            if moved {
                let short: String = head.commit.chars().take(7).collect();
                let cause = if before.is_some() {
                    format!("mudança em {}: {short}", head.name)
                } else {
                    format!("novo: {} em {short}", head.name)
                };
                let ticket = self.request_build(BuildRequest::new(&name, cause))?;
                changes.push(format!("{name}: build #{} pedido", ticket.number));
            }
        }
        // The heads that are gone.
        let now = self.store.now_ms();
        let days = branches.get("orphan_days").and_then(Value::as_i64).unwrap_or(0).max(0);
        let names: Vec<String> = heads.iter().map(|head| format!("{prefix}{}", head.name)).collect();
        for child in self.jobs()? {
            let ours = child.name.starts_with(&prefix)
                && !child.name[prefix.len()..].contains('/')
                && child.settings.get(HEAD_KEY).is_some();
            if !ours || names.contains(&child.name) {
                continue;
            }
            let gone = child.settings.pointer(&format!("/{HEAD_KEY}/gone_ms")).and_then(Value::as_i64);
            match gone {
                Some(since) if now - since >= days * DAY_MS => {
                    self.delete_job(&child.name)?;
                    changes.push(format!("{}: removido, o head não existe mais", child.name));
                }
                Some(_) => {}
                None if days == 0 => {
                    self.delete_job(&child.name)?;
                    changes.push(format!("{}: removido, o head não existe mais", child.name));
                }
                None => {
                    let mut kept = child.clone();
                    if let Some(Value::Object(head)) = kept.settings.get_mut(HEAD_KEY) {
                        head.insert("gone_ms".to_owned(), json!(now));
                    }
                    self.save_job(kept)?;
                    changes.push(format!("{}: o head sumiu; sai em {days} dia(s)", child.name));
                }
            }
        }
        Ok(changes)
    }
}

/// The settings of the job of `head`, made from the model's.
fn child_settings(model: &JobSpec, head: &Head) -> Value {
    let mut settings = match &model.settings {
        Value::Object(object) => object.clone(),
        _ => Map::new(),
    };
    settings.remove("branches");
    let mut scm = match settings.remove("scm") {
        Some(Value::Object(scm)) => scm,
        _ => Map::new(),
    };
    let reference = match &head.kind {
        HeadKind::PullRequest { number, target, .. } => {
            if !scm.contains_key("merge_onto_target") {
                scm.insert("merge_onto_target".to_owned(), json!(true));
            }
            json!({ "pull_request": { "number": number, "target": target } })
        }
        _ => json!({ "branch": head.name }),
    };
    scm.insert("reference".to_owned(), reference);
    if !scm.is_empty() {
        settings.insert("scm".to_owned(), Value::Object(scm));
    }
    settings.insert(HEAD_KEY.to_owned(), json!({ "commit": head.commit, "kind": head.kind }));
    Value::Object(settings)
}
