//! The configuration of a server as code (#898): its jobs, folders and plugins in one file kept in
//! a repository, applied with the server running.
//!
//! # Its role
//!
//! A CI made by hand, job by job, cannot be made again. The file says what the server has; applying
//! it makes the server match, and the history in the repository says who changed what.
//!
//! # How it works
//!
//! - **The file** is TOML:
//!
//!   ```toml
//!   url = "https://ci.empresa.dev"
//!
//!   [plugins.cron]
//!   enabled = true
//!   settings = { tick_ms = 1000 }
//!
//!   [folders]
//!   apps = "Aplicativos"
//!
//!   [[jobs]]
//!   name = "apps/site"
//!   file = "pipelines/site.raven-ci.toml"   # or pipeline = """…"""; format = "toml" unless said
//!   keep_builds = 30
//!   settings = { triggers = [{ id = "cron", every = "1h" }, { id = "api" }] }
//!   ```
//!
//!   A field it does not know is refused, naming it; `file` is read from beside the file.
//! - **[`Ci::apply_config`]** makes the server match it: plugins switched on or off and
//!   configured, folders titled, each job made or changed (one that did not change is left as it
//!   is). A job no more in the file stays unless asked to remove it. Credentials (in the vault)
//!   and builds are never touched.
//! - **The `url`** is kept by the core ([`Ci::url`]), for the links of builds.
//! - **The triggers** of each job made, changed or removed are watched again at once
//!   ([`Ci::watch_job`]) when the core is shared, so a server running takes the change without
//!   a restart; when a plugin was switched or configured (it starts again), every job's are.

use std::collections::BTreeMap;
use std::path::Path;

use raven_ci_plugin_api::CiError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Ci, JobSpec, PluginState};

/// The name of the file of a server, by custom.
pub const CONFIG_FILE: &str = "raven-ci.toml";

/// What a server has, as its file says.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// The address people reach the server at, for the links of builds.
    #[serde(default)]
    pub url: Option<String>,
    /// Each plugin switched on or off and its configuration, by name.
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginConfig>,
    /// The title of each folder, by its path.
    #[serde(default)]
    pub folders: BTreeMap<String, String>,
    /// The jobs.
    #[serde(default)]
    pub jobs: Vec<JobConfig>,
}

/// A plugin, as the file says it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    /// Switched on or off; as it is when not said.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Its configuration; as it is when not said.
    #[serde(default)]
    pub settings: Option<Value>,
}

/// A job, as the file says it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobConfig {
    /// Its path: `apps/site`.
    pub name: String,
    /// What people see.
    #[serde(default)]
    pub title: Option<String>,
    /// The format of its pipeline; `toml` when not said.
    #[serde(default)]
    pub format: Option<String>,
    /// The pipeline, in the file.
    #[serde(default)]
    pub pipeline: Option<String>,
    /// The pipeline, in a file beside this one; read into `pipeline` by [`ServerConfig::read`].
    #[serde(default)]
    pub file: Option<String>,
    /// How many builds it keeps.
    #[serde(default)]
    pub keep_builds: Option<u32>,
    /// For how many days it keeps a build.
    #[serde(default)]
    pub keep_days: Option<u32>,
    /// The settings of the plugins it names: `scm`, `triggers`, `status`.
    #[serde(default)]
    pub settings: Option<Value>,
    /// A build for every request, even one equal to a build in the queue.
    #[serde(default)]
    pub every_request: bool,
}

impl JobConfig {
    /// The job it says, once its pipeline is in it.
    pub fn spec(&self) -> Result<JobSpec, CiError> {
        let pipeline = match (&self.pipeline, &self.file) {
            (Some(pipeline), None) => pipeline.clone(),
            (None, Some(file)) => {
                return Err(CiError::settings(
                    format!("jobs.{}.file", self.name),
                    format!("{file} não foi lido: aplique o arquivo pelo caminho, ou mande o pipeline"),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(CiError::settings(format!("jobs.{}", self.name), "diga pipeline ou file, um só"));
            }
            (None, None) => {
                return Err(CiError::settings(
                    format!("jobs.{}", self.name),
                    "falta o pipeline (pipeline ou file)",
                ));
            }
        };
        let mut spec = JobSpec::new(
            self.name.clone(),
            self.format.clone().unwrap_or_else(|| "toml".to_owned()),
            pipeline,
        );
        spec.title = self.title.clone();
        spec.keep_builds = self.keep_builds;
        spec.keep_days = self.keep_days;
        spec.settings = match &self.settings {
            None => Value::Object(Map::new()),
            Some(settings @ Value::Object(_)) => settings.clone(),
            Some(_) => return Err(CiError::settings(format!("jobs.{}.settings", self.name), "é uma tabela")),
        };
        spec.coalesce = !self.every_request;
        spec.check()?;
        Ok(spec)
    }
}

impl ServerConfig {
    /// The configuration in `text`, TOML. A field it does not know is refused, with its line.
    pub fn parse(text: &str) -> Result<Self, CiError> {
        let config: Self = toml::from_str(text).map_err(|e| CiError::settings(CONFIG_FILE, e.to_string()))?;
        config.check()?;
        Ok(config)
    }

    /// The configuration in the file at `path`, with the `file` of each job read from beside it.
    pub fn read(path: &Path) -> Result<Self, CiError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CiError::settings(CONFIG_FILE, format!("{}: {e}", path.display())))?;
        let mut config = Self::parse(&text)?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        for job in &mut config.jobs {
            if let (None, Some(file)) = (&job.pipeline, job.file.take()) {
                let at = base.join(&file);
                job.pipeline = Some(std::fs::read_to_string(&at).map_err(|e| {
                    CiError::settings(format!("jobs.{}.file", job.name), format!("{}: {e}", at.display()))
                })?);
            }
        }
        Ok(config)
    }

    /// Two jobs with one name, or a job with no name, are refused.
    pub fn check(&self) -> Result<(), CiError> {
        let mut seen = std::collections::BTreeSet::new();
        for job in &self.jobs {
            if job.name.trim().is_empty() {
                return Err(CiError::settings("jobs", "um job sem nome"));
            }
            if !seen.insert(job.name.as_str()) {
                return Err(CiError::settings(format!("jobs.{}", job.name), "aparece duas vezes"));
            }
        }
        Ok(())
    }
}

impl Ci {
    /// The address people reach this server at, as its configuration says.
    pub fn url(&self) -> Result<Option<String>, CiError> {
        self.store.setting("url")
    }

    /// Makes this server match `config`, and says each thing that changed: plugins switched and
    /// configured, folders titled, jobs made, changed and, when `remove`, removed. A job no more
    /// in the file stays (and is said) unless `remove`. Credentials and builds are not touched.
    /// The triggers of each job that changed are watched again when the core is
    /// [`share`](Self::share)d. Every job is checked before anything is saved.
    pub fn apply_config(&self, config: &ServerConfig, remove: bool) -> Result<Vec<String>, CiError> {
        config.check()?;
        let specs: Vec<JobSpec> = config.jobs.iter().map(JobConfig::spec).collect::<Result<_, _>>()?;
        let mut changes = Vec::new();
        if let Some(url) = &config.url
            && self.url()?.as_deref() != Some(url.as_str())
        {
            self.store.set_setting("url", url)?;
            changes.push(format!("url: {url}"));
        }
        let mut plugins_changed = false;
        for (name, plugin) in &config.plugins {
            if let Some(enabled) = plugin.enabled {
                let on = self.registry.info(name)?.state == PluginState::On;
                if enabled != on {
                    let said = if enabled { self.enable_plugin(name)? } else { self.disable_plugin(name)? };
                    changes.extend(said.into_iter().map(|c| format!("plugin {c}")));
                    plugins_changed = true;
                }
            }
            if let Some(settings) = &plugin.settings
                && self.store.plugin(name)?.settings != *settings
            {
                self.configure_plugin(name, settings.clone())?;
                // A setting can leave the plugin refused (a folder it cannot make, say): said here,
                // or the build that needs it is the first to tell, and only in the server's log.
                match self.registry.info(name)?.state {
                    PluginState::Refused(why) => {
                        changes.push(format!(
                            "plugin {name}: configuração aplicada, mas o plugin ficou recusado: {why}"
                        ));
                    }
                    _ => changes.push(format!("plugin {name}: configuração aplicada")),
                }
                plugins_changed = true;
            }
        }
        for (path, title) in &config.folders {
            self.set_folder_title(path, title)?;
        }
        let shared = self.shared.get().is_some();
        let mut changed = Vec::new();
        for spec in specs {
            let before = self.job(&spec.name).ok();
            if before.as_ref() == Some(&spec) {
                continue;
            }
            let name = spec.name.clone();
            self.save_job(spec)?;
            changes.push(format!("{name}: {}", if before.is_some() { "mudado" } else { "criado" }));
            changed.push(name);
        }
        let wanted: Vec<&str> = config.jobs.iter().map(|job| job.name.as_str()).collect();
        for job in self.jobs()? {
            // The job of a branch belongs to its job of many branches (`<model>/<head>`), whose
            // scan makes and removes it: while its model is in the file it is never "out of the
            // file" nor removed here. When the model goes, it goes with it.
            let of_a_model_kept = job.settings.get(crate::multibranch::HEAD_KEY).is_some()
                && wanted
                    .iter()
                    .any(|model| job.name.strip_prefix(model).is_some_and(|rest| rest.starts_with('/')));
            if wanted.contains(&job.name.as_str()) || of_a_model_kept {
                continue;
            }
            if remove {
                self.delete_job(&job.name)?;
                changes.push(format!("{}: removido", job.name));
                changed.push(job.name);
            } else {
                changes
                    .push(format!("{}: fora do arquivo; fica (aplique com remover para tirá-lo)", job.name));
            }
        }
        // A plugin switched or configured starts again and forgets what it watched: every job then.
        if plugins_changed {
            changed = self.jobs()?.into_iter().map(|job| job.name).chain(changed).collect();
            changed.sort();
            changed.dedup();
        }
        if shared {
            for name in &changed {
                changes.extend(self.watch_job(name).into_iter().map(|p| format!("aviso: {p}")));
            }
        }
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_says_its_jobs_and_refuses_what_it_does_not_know() {
        let config = ServerConfig::parse(
            "url = \"http://ci\"\n[plugins.cron]\nenabled = true\nsettings = { tick_ms = 50 }\n[folders]\napps = \"Aplicativos\"\n[[jobs]]\nname = \"apps/site\"\npipeline = \"x\"\nkeep_builds = 3\nsettings = { triggers = [{ id = \"api\" }] }\n",
        )
        .expect("reads");
        assert_eq!(config.url.as_deref(), Some("http://ci"));
        assert_eq!(config.plugins["cron"].settings, Some(serde_json::json!({ "tick_ms": 50 })));
        let spec = config.jobs[0].spec().expect("a job");
        assert_eq!((spec.format.as_str(), spec.keep_builds), ("toml", Some(3)));
        assert_eq!(spec.settings["triggers"][0]["id"], "api");
        let unknown = ServerConfig::parse("[[jobs]]\nname = \"a\"\npipeline = \"x\"\nagenda = 1\n")
            .expect_err("refused");
        assert!(unknown.to_string().contains("agenda"), "{unknown}");
        let twice = ServerConfig::parse(
            "[[jobs]]\nname = \"a\"\npipeline = \"x\"\n[[jobs]]\nname = \"a\"\npipeline = \"y\"\n",
        );
        assert!(twice.expect_err("refused").to_string().contains("duas vezes"));
        let none = JobConfig { name: "b".into(), ..JobConfig::default() }.spec().expect_err("no pipeline");
        assert!(none.to_string().contains("falta o pipeline"), "{none}");
    }

    /// The jobs a scan made for the branches of a job of many branches are not the file's to keep or
    /// remove (#803, test of the real world: `aplicar --remover` would have removed each branch).
    #[test]
    fn the_jobs_of_branches_are_neither_out_of_the_file_nor_removed() {
        let ci = Ci::in_memory().expect("a core");
        let mut branch = JobSpec::new("repo/main", "toml", "x");
        branch.settings = serde_json::json!({ (crate::multibranch::HEAD_KEY): { "commit": "abc" } });
        ci.save_job(branch).expect("saved");
        ci.save_job(JobSpec::new("velho", "toml", "x")).expect("saved");
        let config = ServerConfig::parse("[[jobs]]\nname = \"repo\"\npipeline = \"x\"\n").expect("reads");
        let said = ci.apply_config(&config, true).expect("applied");
        assert!(said.iter().any(|line| line == "velho: removido"), "{said:?}");
        assert!(!said.iter().any(|line| line.starts_with("repo/main")), "{said:?}");
        assert!(ci.job("repo/main").is_ok(), "the branch stays");
        // Without its model in the file, the branch goes with it.
        let config = ServerConfig::parse("[[jobs]]\nname = \"outro\"\npipeline = \"x\"\n").expect("reads");
        let said = ci.apply_config(&config, true).expect("applied");
        assert!(said.iter().any(|line| line == "repo/main: removido"), "{said:?}");
        assert!(ci.job("repo/main").is_err(), "the branch went with its model");
    }
}
