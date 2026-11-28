//! Jobs of many branches: one job per branch and pull request a [`BranchSource`] finds.
//!
//! # How it works
//!
//! A job whose settings say `{"branches": {"id": "github", ...}}` is a model. [`Ci::scan`] asks the
//! source its heads and keeps one job per head, `<model>/<head>` (`repo/main`, `repo/PR-12`):
//!
//! - **Its pipeline** is the file of the head named by `branches.file` (by default the first file
//!   name of the model's format, `.raven-ci.toml`), read with [`BranchSource::file`]. A pull request
//!   built merged onto its target that has no such file takes its target's, as the merge would
//!   (and as Jenkins reads the `Jenkinsfile` of the merge): a pull request opened before the file
//!   came runs the pipeline of the branch it goes into (#958). With no file anywhere, the model's
//!   own pipeline.
//! - **Its settings** are the model's, but `branches`, with `scm` made from the head: the branch,
//!   or the pull request with `merge_onto_target` (true unless the model's `scm` says otherwise),
//!   and `$head`, what the scan last saw. The status of a pull request goes to its own commit, not
//!   to the merge, as a provider shows it on the pull request.
//! - **A build** is asked for when a head is new or its commit changed since the last scan.
//! - **A head that is gone** has its job removed once `branches.orphan_days` days passed since the
//!   scan first missed it (0, at once, unless said). Its builds stay until their retention.

use std::sync::Arc;

use raven_ci_plugin_api::{BranchSource, BuildRequest, CiError, Head, HeadKind, Reference};
use serde_json::{Map, Value, json};

use crate::{Ci, JobSpec};

/// The key of a job's settings where the scan keeps what it saw of the head.
pub const HEAD_KEY: &str = "$head";

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

impl Ci {
    pub(crate) fn source(&self, id: &str) -> Result<Arc<dyn BranchSource>, CiError> {
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.branch_sources())
            .find(|source| source.id() == id)
            .ok_or_else(|| {
                CiError::unavailable(format!(
                    "falta o plugin {id}: o job o usa para ler as branches ou o pipeline do repositório"
                ))
            })
    }

    /// The first file name the format `id` reads: `.raven-ci.toml` for `toml`.
    pub(crate) fn first_file_of(&self, id: &str) -> Option<String> {
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.pipeline_formats())
            .find(|format| format.id() == id)
            .and_then(|format| format.file_names().into_iter().next())
    }

    /// The format that reads the file `path`, by its name: `jenkinsfile` for `ci/Jenkinsfile`.
    pub(crate) fn format_of_file(&self, path: &str) -> Option<String> {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.pipeline_formats())
            .find(|format| format.file_names().iter().any(|file| file == name))
            .map(|format| format.id().to_owned())
    }

    /// Looks again at the heads of the job of many branches `job` (see the [module](self)); what
    /// changed, in words.
    pub fn scan(&self, job: &str) -> Result<Vec<String>, CiError> {
        let model = self.job(job)?;
        let branches =
            model.settings.get("branches").cloned().ok_or_else(|| {
                CiError::settings("branches", format!("o job {job} não é de muitas branches"))
            })?;
        let branches = with_scm(branches, &model.settings);
        let id = branches.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let source = self.source(&id)?;
        let heads = source.discover(&branches)?;
        let file = match branches.get("file").and_then(Value::as_str) {
            Some(file) => Some(file.to_owned()),
            None => self.first_file_of(&model.format),
        };
        // The format of the children is the file's: a `Jenkinsfile` is read as one even when the
        // model's own pipeline is TOML.
        let format =
            file.as_deref().and_then(|file| self.format_of_file(file)).unwrap_or(model.format.clone());
        let mut changes = Vec::new();
        let prefix = format!("{job}/");
        for head in &heads {
            let name = format!("{prefix}{}", head.name);
            let before = self.job(&name).ok();
            let seen = before
                .as_ref()
                .and_then(|child| child.settings.pointer(&format!("/{HEAD_KEY}/commit")).cloned());
            let found = match &file {
                Some(file) => match source.file(&branches, head, file)? {
                    Some(text) => Some(text),
                    None => match merged_onto(head, &heads, &model.settings) {
                        Some(target) => source.file(&branches, &target, file)?,
                        None => None,
                    },
                },
                None => None,
            };
            let (pipeline, format) = match found {
                Some(text) => (text, format.clone()),
                None => (model.pipeline.clone(), model.format.clone()),
            };
            let mut child = JobSpec::new(&name, &format, pipeline).settings(child_settings(&model, head));
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

/// Where the pipeline of a build came from, when it came from the repository: kept with the
/// build and said in its log.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PipelineFrom {
    /// The file: `Jenkinsfile`.
    pub file: String,
    /// The commit it was read at; empty when the source did not say.
    pub commit: String,
    /// The branch or the pull request: `main`, `PR-12`.
    pub head: String,
}

impl Ci {
    /// The pipeline of `job` read from its repository, when its settings say `pipeline_from =
    /// "scm"` or name a file in `scm.file`: the format (by the file's name), the text and where it
    /// came from. `None` when the job keeps its pipeline itself.
    ///
    /// The file is read by the [`BranchSource`] with the id of the job's `scm` (`git` reads any
    /// server), at the tip of the branch the build fetches: `scm.reference`, else the last part of
    /// the job's name, as the build does.
    pub(crate) fn pipeline_from_scm(
        &self,
        job: &JobSpec,
    ) -> Result<Option<(String, String, PipelineFrom)>, CiError> {
        let settings = &job.settings;
        let named = settings.pointer("/scm/file").and_then(Value::as_str).map(str::to_owned);
        let asked = settings.get("pipeline_from").and_then(Value::as_str);
        if let Some(other) = asked.filter(|asked| !matches!(*asked, "scm" | "job")) {
            return Err(CiError::settings(
                "pipeline_from",
                format!("\"{other}\" não existe: use \"scm\" (o arquivo do repositório) ou \"job\""),
            ));
        }
        if asked == Some("job") || (asked.is_none() && named.is_none()) {
            return Ok(None);
        }
        let scm = settings.get("scm").cloned().ok_or_else(|| {
            CiError::settings("pipeline_from", "o pipeline vem do repositório, mas o job não diz o scm")
        })?;
        let id = scm.get("id").and_then(Value::as_str).unwrap_or("git").to_owned();
        let source = self.source(&id)?;
        let file = named
            .or_else(|| self.first_file_of(&job.format))
            .ok_or_else(|| CiError::settings("scm.file", "diga o arquivo do pipeline no repositório"))?;
        let format = self.format_of_file(&file).unwrap_or_else(|| job.format.clone());
        let reference: Option<Reference> =
            scm.get("reference").cloned().and_then(|value| serde_json::from_value(value).ok());
        let branch_of_name = || job.name.rsplit('/').next().unwrap_or(&job.name).to_owned();
        let head = match reference {
            Some(Reference::PullRequest { number, target }) => Head {
                name: format!("PR-{number}"),
                kind: HeadKind::PullRequest { number, target, from_fork: false },
                commit: String::new(),
            },
            Some(Reference::Commit(commit)) => Head { name: commit.clone(), kind: HeadKind::Branch, commit },
            Some(Reference::Branch(branch)) => self.tip(&source, &scm, &branch)?,
            Some(_) | None => self.tip(&source, &scm, &branch_of_name())?,
        };
        let text = source
            .file(&scm, &head, &file)?
            .ok_or_else(|| CiError::failed(format!("o repositório não tem {file} em {}", head.name)))?;
        let from = PipelineFrom { file, commit: head.commit.clone(), head: head.name.clone() };
        Ok(Some((format, text, from)))
    }

    /// The head of `branch` as the source finds it now.
    fn tip(&self, source: &Arc<dyn BranchSource>, scm: &Value, branch: &str) -> Result<Head, CiError> {
        let mut only = scm.clone();
        if let Value::Object(object) = &mut only {
            object.insert("branches".to_owned(), json!([branch]));
            object.insert("pull_requests".to_owned(), json!(false));
        }
        source
            .discover(&only)?
            .into_iter()
            .find(|head| head.name == branch)
            .ok_or_else(|| CiError::failed(format!("a branch {branch} não existe no repositório")))
    }
}

/// The head of the branch a pull request is built merged onto, to read its pipeline from when the
/// pull request has none (#958): the one the scan found, with its commit, or else the branch by its
/// name. `None` for a branch, and for a pull request the model builds alone
/// (`scm.merge_onto_target = false`), whose pipeline can only be its own.
fn merged_onto(head: &Head, heads: &[Head], settings: &Value) -> Option<Head> {
    let HeadKind::PullRequest { target, .. } = &head.kind else {
        return None;
    };
    if settings.pointer("/scm/merge_onto_target").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let found = heads.iter().find(|other| matches!(other.kind, HeadKind::Branch) && other.name == *target);
    Some(found.cloned().unwrap_or_else(|| Head {
        name: target.clone(),
        kind: HeadKind::Branch,
        // A provider that reads by commit takes a branch's name as well (`?ref=main`).
        commit: target.clone(),
    }))
}

/// The settings of a source with what it can take from the job's `scm` when it does not say it
/// itself: the address and the credential, so a job of many branches over plain git names its
/// repository once.
pub(crate) fn with_scm(source: Value, settings: &Value) -> Value {
    let Value::Object(mut source) = source else { return source };
    for field in ["url", "credential"] {
        if !source.contains_key(field)
            && let Some(value) = settings.pointer(&format!("/scm/{field}"))
        {
            source.insert(field.to_owned(), value.clone());
        }
    }
    Value::Object(source)
}

/// The settings of the job of `head`, made from the model's.
fn child_settings(model: &JobSpec, head: &Head) -> Value {
    let mut settings = match &model.settings {
        Value::Object(object) => object.clone(),
        _ => Map::new(),
    };
    settings.remove("branches");
    // The scan is the model's: a job of a branch that kept it was scanned by the cron as a model
    // and failed every time ("não é de muitas branches").
    if let Some(Value::Array(triggers)) = settings.get_mut("triggers") {
        triggers.retain(|trigger| trigger.get("id").and_then(Value::as_str) != Some("scan"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_job_of_a_branch_keeps_the_triggers_of_its_model_but_the_scan() {
        let mut model = JobSpec::new("repo", "toml", "x");
        model.settings = json!({
            "branches": { "id": "github", "repository": "dono/repo" },
            "scm": { "id": "git", "url": "https://x/repo.git", "depth": 1 },
            "triggers": [{ "id": "api" }, { "id": "scan", "every": "5m" }],
        });
        let head = Head { name: "main".into(), kind: HeadKind::Branch, commit: "abc".into() };
        let child = child_settings(&model, &head);
        assert_eq!(child["triggers"], json!([{ "id": "api" }]));
        assert_eq!(child["scm"]["reference"], json!({ "branch": "main" }));
        assert_eq!(child["scm"]["depth"], 1);
        assert!(child.get("branches").is_none());
    }
}
