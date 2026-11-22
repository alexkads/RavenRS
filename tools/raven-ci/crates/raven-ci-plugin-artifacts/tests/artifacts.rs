//! The plugin artifacts (#879): the search by patterns, refusing what leaves the folder; a build
//! that keeps its files and says which pattern found nothing; and the routes that list and give
//! the bytes back, refusing an attachment that is not an artifact.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{Attachment, BuildRequest, BuildStatus, CiPlugin, HttpRequest};
use raven_ci_plugin_artifacts::{ArtifactsPlugin, find};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use serde_json::Value;

struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(name: &str) -> Folder {
    let dir = std::env::temp_dir().join(format!("raven-ci-artifacts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    std::fs::write(path, text).expect("written");
}

#[test]
fn the_patterns_find_each_file_once_and_refuse_what_leaves_the_folder() {
    let ws = folder("busca");
    write(&ws.0, "dist/site.tar.gz", "x");
    write(&ws.0, "dist/notas.txt", "x");
    write(&ws.0, "relatorios/a/b/index.html", "x");
    write(&ws.0, "raiz.txt", "x");
    let said = Mutex::new(Vec::new());
    let log = |line: &str| said.lock().expect("lock").push(line.to_owned());
    let patterns: Vec<String> =
        ["dist/*", "**/*.html", "dist/*.txt", "nada/*.zip", "../fora/*", "/etc/passwd", "*.txt"]
            .iter()
            .map(|p| (*p).to_owned())
            .collect();
    let names: Vec<String> = find(&ws.0, &patterns, &log).into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["dist/notas.txt", "dist/site.tar.gz", "relatorios/a/b/index.html", "raiz.txt"]);
    let said = said.lock().expect("lock").join("\n");
    assert!(said.contains("nada/*.zip: nada achado"), "{said}");
    assert!(said.contains("../fora/*: recusado"), "{said}");
    assert!(said.contains("/etc/passwd: recusado"), "{said}");
    assert!(!said.contains("dist/*.txt: nada"), "a file found twice is still found: {said}");
}

#[test]
#[cfg(unix)]
fn a_link_that_leads_out_of_the_folder_is_refused() {
    let ws = folder("link");
    let outside = folder("fora");
    write(&outside.0, "segredo.txt", "não");
    std::os::unix::fs::symlink(outside.0.join("segredo.txt"), ws.0.join("link.txt")).expect("a link");
    let said = Mutex::new(Vec::new());
    let found = find(&ws.0, &["*.txt".to_owned()], &|line| said.lock().expect("lock").push(line.to_owned()));
    assert!(found.is_empty(), "{found:?}");
    assert!(said.lock().expect("lock").join("\n").contains("leva para fora"));
}

fn get(plugin: &ArtifactsPlugin, path: &str, query: &[(&str, String)]) -> (u16, String, Vec<u8>) {
    let route = plugin.routes().into_iter().find(|r| r.path == path).expect("the route");
    let request = HttpRequest {
        method: "GET".to_owned(),
        path: path.to_owned(),
        query: query.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect(),
        actor: Some("ana".to_owned()),
        ..HttpRequest::default()
    };
    let answer = (route.handler)(&request);
    (answer.status, answer.content_type, answer.body)
}

#[test]
fn a_build_keeps_its_files_says_what_found_nothing_and_the_routes_give_them_back() {
    let plugin = Arc::new(ArtifactsPlugin::new());
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.share_attachments();
    for one in [plugin.clone() as Arc<dyn CiPlugin>, Arc::new(ShellPlugin::new()), Arc::new(TomlFormatPlugin)]
    {
        ci.add_plugin(one);
    }
    ci.apply_plugins().expect("on");
    let pipeline = "artifacts = [\"*.txt\", \"nada/*.zip\"]\n\n[[stages]]\nname = \"Gera\"\nsteps = [\"echo um> um.txt\", \"echo dois> dois.txt\"]\n";
    ci.save_job(JobSpec::new("apps/site", "toml", pipeline)).expect("the job");
    ci.request_build(BuildRequest::new("apps/site", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let log: Vec<String> =
        ci.build_log("apps/site", 1, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert_eq!(ci.build("apps/site", 1).expect("the build").status, BuildStatus::Succeeded, "{log:?}");
    assert!(log.iter().any(|l| l.contains("nada/*.zip: nada achado")), "{log:?}");
    let kept = ci.attachments("apps/site", 1).expect("attachments");
    let names: Vec<&str> = kept.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["dois.txt", "um.txt"]);
    assert!(kept.iter().all(|a| a.kind == "artifact" && a.summary["size"].as_u64() == Some(a.size)));

    let (status, _, body) =
        get(&plugin, "/api/ci/artifacts", &[("job", "apps/site".to_owned()), ("number", "1".to_owned())]);
    assert_eq!(status, 200);
    let list: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(list.as_array().map(Vec::len), Some(2), "{list}");
    let id = list[1]["id"].as_i64().expect("an id");
    let (status, kind, bytes) = get(&plugin, "/api/ci/artifact", &[("id", id.to_string())]);
    assert_eq!((status, kind.as_str()), (200, "text/plain; charset=utf-8"));
    assert_eq!(String::from_utf8_lossy(&bytes).trim(), "um");

    let junit = ci
        .attach("apps/site", 1, &Attachment::new("junit", "testes", "application/json", b"{}".to_vec()))
        .expect("kept");
    assert_eq!(get(&plugin, "/api/ci/artifact", &[("id", junit.to_string())]).0, 404, "not an artifact");
    assert_eq!(get(&plugin, "/api/ci/artifact", &[("id", "999".to_owned())]).0, 404);
    assert_eq!(get(&plugin, "/api/ci/artifacts", &[("job", "apps/site".to_owned())]).0, 400, "no number");
}

#[test]
fn a_build_whose_patterns_find_nothing_says_so_and_allow_empty_lets_it_be() {
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.share_attachments();
    for one in [
        Arc::new(ArtifactsPlugin::new()) as Arc<dyn CiPlugin>,
        Arc::new(ShellPlugin::new()),
        Arc::new(TomlFormatPlugin),
    ] {
        ci.add_plugin(one);
    }
    ci.apply_plugins().expect("on");
    let pipeline =
        "artifacts = [\"dist/*.tar.gz\"]\n\n[[stages]]\nname = \"Nada\"\nsteps = [\"echo nada\"]\n";
    ci.save_job(JobSpec::new("vazio", "toml", pipeline)).expect("the job");
    ci.request_build(BuildRequest::new("vazio", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let log =
        ci.build_log("vazio", 1, 0).expect("log").into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    assert!(log.contains("nenhum artefato achado em dist/*.tar.gz"), "{log}");
    assert!(ci.attachments("vazio", 1).expect("attachments").is_empty());
}
