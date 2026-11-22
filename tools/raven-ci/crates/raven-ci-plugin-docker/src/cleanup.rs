//! What a build leaves on the engine, and its removal at the end of the build.
//!
//! Every container a step of this plugin starts has the label [`LABEL`]` = <run>`; every compose
//! project is `raven-ci-<run>`. The first step of the plugin in a build asks the build's scope
//! for one [`BuildScope::at_end`] that removes, however the build ends, every container (with
//! its anonymous volumes), network and volume with the label, and brings down every project the
//! build brought up, with its volumes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

use raven_ci_plugin_api::BuildScope;

use crate::engine::{Engine, slug};

/// The label every container, network and volume of a build has, with the build's run.
pub const LABEL: &str = "raven-ci.build";

/// A compose project a build brought up, and how to bring it down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Its name: `raven-ci-<run>`.
    pub name: String,
    /// The `-f` files it was brought up with.
    pub files: Vec<String>,
    /// The folder it was brought up in.
    pub dir: PathBuf,
}

#[derive(Default)]
struct Builds {
    /// The runs whose cleanup was asked of their scope.
    watched: BTreeSet<String>,
    /// The projects each run brought up.
    projects: BTreeMap<String, Vec<Project>>,
}

fn builds() -> &'static Mutex<Builds> {
    static BUILDS: OnceLock<Mutex<Builds>> = OnceLock::new();
    BUILDS.get_or_init(Mutex::default)
}

/// The name of the compose project of the run `run`.
pub fn project_name(run: &str) -> String {
    format!("raven-ci-{}", slug(run))
}

/// Asks `build`'s scope, once per build, to remove what the build left on `engine` at its end.
pub fn watch(build: &BuildScope, engine: &Engine) {
    let run = build.context().run.clone();
    if !builds().lock().unwrap_or_else(PoisonError::into_inner).watched.insert(run.clone()) {
        return;
    }
    let engine = engine.clone();
    build.at_end(format!("remover contêineres, redes e volumes do build ({})", engine.name()), move || {
        clean(&engine, &run);
    });
}

/// Keeps `project` as brought up by `run`, to bring it down at the end.
pub fn brought_up(run: &str, project: Project) {
    let mut builds = builds().lock().unwrap_or_else(PoisonError::into_inner);
    let list = builds.projects.entry(run.to_owned()).or_default();
    if !list.iter().any(|p| p.name == project.name) {
        list.push(project);
    }
}

/// Forgets the project `name` of `run`: it was brought down by a step.
pub fn brought_down(run: &str, name: &str) {
    if let Some(list) = builds().lock().unwrap_or_else(PoisonError::into_inner).projects.get_mut(run) {
        list.retain(|p| p.name != name);
    }
}

/// The projects `run` brought up and did not bring down.
pub fn projects(run: &str) -> Vec<Project> {
    builds().lock().unwrap_or_else(PoisonError::into_inner).projects.get(run).cloned().unwrap_or_default()
}

/// The arguments of `compose` for `project`: `compose -p <name> -f <file>…`.
pub fn compose_args(project: &str, files: &[String]) -> Vec<String> {
    let mut args = vec!["compose".to_owned(), "-p".to_owned(), project.to_owned()];
    for file in files {
        args.push("-f".to_owned());
        args.push(file.clone());
    }
    args
}

/// Removes what `run` left on `engine`: its projects, then its containers, networks and volumes.
pub fn clean(engine: &Engine, run: &str) {
    let projects = {
        let mut builds = builds().lock().unwrap_or_else(PoisonError::into_inner);
        builds.watched.remove(run);
        builds.projects.remove(run).unwrap_or_default()
    };
    for project in projects {
        let mut args = compose_args(&project.name, &project.files);
        args.extend(["down", "-v", "--remove-orphans"].map(str::to_owned));
        let _ = engine.command(args).dir(&project.dir).timeout(crate::engine::QUICK).output();
    }
    let filter = format!("label={LABEL}={run}");
    let containers = engine.lines(["ps", "-aq", "--filter", &filter]);
    if !containers.is_empty() {
        let _ = engine.ask(["rm", "-f", "-v"].into_iter().map(str::to_owned).chain(containers));
    }
    let networks = engine.lines(["network", "ls", "-q", "--filter", &filter]);
    if !networks.is_empty() {
        let _ = engine.ask(["network", "rm"].into_iter().map(str::to_owned).chain(networks));
    }
    let volumes = engine.lines(["volume", "ls", "-q", "--filter", &filter]);
    if !volumes.is_empty() {
        let _ = engine.ask(["volume", "rm", "-f"].into_iter().map(str::to_owned).chain(volumes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_is_kept_once_and_forgotten_when_brought_down() {
        let project = Project { name: project_name("run-A/1"), files: vec![], dir: PathBuf::from(".") };
        assert_eq!(project.name, "raven-ci-run-a-1");
        brought_up("run-A/1", project.clone());
        brought_up("run-A/1", project.clone());
        assert_eq!(projects("run-A/1"), std::slice::from_ref(&project));
        brought_down("run-A/1", &project.name);
        assert!(projects("run-A/1").is_empty());
    }

    #[test]
    fn the_arguments_of_compose_name_the_project_and_each_file() {
        assert_eq!(
            compose_args("raven-ci-x", &["a.yml".into(), "b.yml".into()]),
            ["compose", "-p", "raven-ci-x", "-f", "a.yml", "-f", "b.yml"]
        );
    }
}
