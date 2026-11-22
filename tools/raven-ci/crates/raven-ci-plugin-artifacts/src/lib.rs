//! The plugin `artifacts` of Raven CI: what a build makes, kept with it and downloaded later.
//!
//! # Its role
//!
//! The folder of a build is removed when it ends. What has to outlive it (a package, a report,
//! the evidence of the tests) is taken out first by this [`Publisher`], kept by the core with the
//! build (and forgotten with it, by the job's retention), and given back over the API, as
//! `archiveArtifacts` and the artifacts page of Jenkins do.
//!
//! # How it works
//!
//! - **The patterns** come from `settings.paths`, a text or a list: `artifacts = ["dist/*.tar.gz",
//!   "relatorios/**/*.html"]` in a `.raven-ci.toml`. They are relative to the folder of the build
//!   and take `*` (inside one folder), `**` (any folders) and `?`. A pattern that is absolute or
//!   goes up with `..` is refused, and so is a file whose link leads out of the folder.
//! - **Each file** is an [`Attachment`] of the kind `artifact`, named by its path in the folder
//!   (`/` on every system), with its type by its extension and `{"size": N}` as its summary.
//!   A file bigger than `max_mb` (512 by default) is left out, saying so.
//! - **What found nothing** goes to the log of the build, pattern by pattern. When no pattern
//!   found anything the publisher fails, unless `allow_empty = true`: a build whose package went
//!   missing should say it.
//! - **The routes**, for anyone signed in: `GET /api/ci/artifacts?job=&number=` lists the
//!   artifacts of a build (`id`, `name`, `size`, `mime`), and `GET /api/ci/artifact?id=` gives the
//!   bytes of one with its type. The CLI's `raven-ci evidencia` uses both.
//! - **[`find`]** is the search itself, public: the plugin `junit` finds its reports with it.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{
    Attachment, BuildInfo, BuildScope, CiError, CiPlugin, Host, HttpRequest, HttpResponse, Publisher, Route,
};
use ravenrs_app::{Module, ModuleManifest};
use serde_json::{Value, json};

/// The kind of the attachments it keeps.
pub const KIND: &str = "artifact";
/// The biggest file kept unless the settings say: 512 MB.
pub const MAX_MB: u64 = 512;

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `artifacts`.
#[derive(Default)]
pub struct ArtifactsPlugin {
    host: Shared,
}

impl std::fmt::Debug for ArtifactsPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactsPlugin").finish_non_exhaustive()
    }
}

impl ArtifactsPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for ArtifactsPlugin {
    fn name(&self) -> &str {
        "artifacts"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }
}

impl CiPlugin for ArtifactsPlugin {
    fn summary(&self) -> String {
        "Guarda os arquivos do build pelos padrões e dá a lista e os bytes pela API.".to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        vec![Arc::new(Artifacts)]
    }

    fn routes(&self) -> Vec<Route> {
        let (list, one) = (self.host.clone(), self.host.clone());
        vec![
            Route::new("GET", "/api/ci/artifacts", move |request| answer(&list, request, listing)),
            Route::new("GET", "/api/ci/artifact", move |request| answer(&one, request, download)),
        ]
    }
}

/// A file a pattern found in the folder of a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Where it is.
    pub path: PathBuf,
    /// Its path in the folder, with `/`: `dist/site.tar.gz`.
    pub name: String,
}

/// The patterns of `settings.paths`: a text, or a list of texts.
pub fn patterns(settings: &Value) -> Vec<String> {
    match settings.get("paths") {
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        _ => Vec::new(),
    }
}

/// The files of `workspace` that `patterns` find, each once, in the order the patterns and then
/// the names come. What could not be taken (a pattern that finds nothing, one refused, a link
/// that leads out of the folder) is said to `log`.
pub fn find(workspace: &Path, patterns: &[String], log: &dyn Fn(&str)) -> Vec<Found> {
    let root = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
    let options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for pattern in patterns {
        let relative = Path::new(pattern);
        if pattern.trim().is_empty()
            || relative.is_absolute()
            || relative.components().any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            log(&format!("{pattern}: recusado; o padrão é relativo à pasta do build, sem .."));
            continue;
        }
        let full =
            format!("{}/{}", glob::Pattern::escape(&workspace.to_string_lossy()), pattern.replace('\\', "/"));
        let paths = match glob::glob_with(&full, options) {
            Ok(paths) => paths,
            Err(e) => {
                log(&format!("{pattern}: o padrão não vale: {e}"));
                continue;
            }
        };
        let mut any = false;
        for path in paths.flatten() {
            if !path.is_file() {
                continue;
            }
            let real = path.canonicalize().unwrap_or_else(|_| path.clone());
            let (Ok(_), Ok(inside)) = (real.strip_prefix(&root), path.strip_prefix(workspace)) else {
                log(&format!("{}: recusado; o link leva para fora da pasta do build", path.display()));
                continue;
            };
            let name = inside
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            any = true;
            if seen.insert(name.clone()) {
                found.push(Found { path, name });
            }
        }
        if !any {
            log(&format!("{pattern}: nada achado"));
        }
    }
    found
}

/// The type of a file by its extension.
pub fn mime_of(name: &str) -> &'static str {
    let extension = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match extension.as_str() {
        "txt" | "log" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// The publisher `artifacts`. See the [crate](crate).
struct Artifacts;

impl Publisher for Artifacts {
    fn id(&self) -> &str {
        "artifacts"
    }

    fn publish(
        &self,
        build: &BuildInfo,
        settings: &Value,
        workspace: &Path,
    ) -> Result<Vec<Attachment>, CiError> {
        let scope = BuildScope::of_build(&build.job, build.number);
        let log = |line: &str| {
            if let Some(scope) = &scope {
                scope.log("artifacts", line);
            }
        };
        let wanted = patterns(settings);
        if wanted.is_empty() {
            return Err(CiError::settings("paths", "diga os padrões dos arquivos: paths = [\"dist/*\"]"));
        }
        let max_bytes = settings.get("max_mb").and_then(Value::as_u64).unwrap_or(MAX_MB) * 1024 * 1024;
        let mut kept = Vec::new();
        for file in find(workspace, &wanted, &log) {
            let size = std::fs::metadata(&file.path).map(|m| m.len()).unwrap_or(0);
            if size > max_bytes {
                log(&format!(
                    "{}: {size} bytes, acima do limite de {} MB; fica de fora",
                    file.name,
                    max_bytes / 1024 / 1024
                ));
                continue;
            }
            let bytes =
                std::fs::read(&file.path).map_err(|e| CiError::failed(format!("ler {}: {e}", file.name)))?;
            kept.push(
                Attachment::new(KIND, file.name.clone(), mime_of(&file.name), bytes)
                    .summary(json!({ "size": size })),
            );
        }
        if kept.is_empty() && settings.get("allow_empty").and_then(Value::as_bool) != Some(true) {
            return Err(CiError::failed(format!("nenhum artefato achado em {}", wanted.join(", "))));
        }
        Ok(kept)
    }
}

fn refuse(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &json!({ "error": "ci", "message": message.into() }))
}

fn from_error(e: &CiError) -> HttpResponse {
    match e {
        CiError::Unavailable(_) => refuse(404, e.to_string()),
        CiError::Settings { .. } => refuse(400, e.to_string()),
        _ => refuse(500, e.to_string()),
    }
}

/// Answers `request` with `what`, through the host while the plugin is on.
fn answer(
    host: &Shared,
    request: &HttpRequest,
    what: fn(&dyn Host, &HttpRequest) -> Result<HttpResponse, CiError>,
) -> HttpResponse {
    let host = host.lock().unwrap_or_else(PoisonError::into_inner).clone();
    match host {
        Some(host) => what(host.as_ref(), request).unwrap_or_else(|e| from_error(&e)),
        None => refuse(404, "o plugin artifacts não está ligado"),
    }
}

fn wanted<'r>(request: &'r HttpRequest, name: &str) -> Result<&'r str, CiError> {
    request.query.get(name).map(String::as_str).ok_or_else(|| CiError::settings(name, "falta na consulta"))
}

/// `GET /api/ci/artifacts?job=&number=`.
fn listing(host: &dyn Host, request: &HttpRequest) -> Result<HttpResponse, CiError> {
    let job = wanted(request, "job")?;
    let number: u64 = wanted(request, "number")?
        .parse()
        .map_err(|_| CiError::settings("number", "tem que ser um número"))?;
    let list: Vec<Value> = host
        .attachments(job, Some(number), KIND)?
        .into_iter()
        .map(|a| json!({ "id": a.id, "name": a.name, "size": a.size, "mime": a.mime }))
        .collect();
    Ok(HttpResponse::json(200, &Value::Array(list)))
}

/// `GET /api/ci/artifact?id=`.
fn download(host: &dyn Host, request: &HttpRequest) -> Result<HttpResponse, CiError> {
    let id: i64 =
        wanted(request, "id")?.parse().map_err(|_| CiError::settings("id", "tem que ser um número"))?;
    let (info, bytes) = host.attachment(id)?;
    if info.kind != KIND {
        return Err(CiError::unavailable(format!("o anexo {id} não é um artefato")));
    }
    Ok(HttpResponse { status: 200, content_type: info.mime, body: bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_comes_from_the_extension() {
        assert_eq!(mime_of("dist/site.TAR.GZ"), "application/gzip");
        assert_eq!(mime_of("relatorio.html"), "text/html; charset=utf-8");
        assert_eq!(mime_of("sem-extensao"), "application/octet-stream");
        assert_eq!(patterns(&json!({ "paths": "a/*" })), ["a/*"]);
        assert_eq!(patterns(&json!({ "paths": ["a", 1, "b"] })), ["a", "b"]);
    }
}
