//! The example of a plugin of Raven CI written in Rust with the standard traits, as a program of its
//! own: `raven-ci plugins compilar tools/raven-ci/plugins/exemplo-rust` builds it, writes its
//! `plugin.toml` and installs it, with no new `raven-ci`.
//!
//! It is the same code a plugin compiled into the server has (a [`CiPlugin`] and its points); only
//! its `main` is [`serve`](raven_ci_plugin_api::serve). It brings:
//!
//! - the step `exemplo-saudar`, which greets in the log of the build, reads a secret through the
//!   [`Host`] when asked and writes the greeting to `saudacao.txt` in the folder of the build;
//! - the publisher `exemplo-relatorio`, which keeps a file of the folder of the build with it;
//! - the trigger `exemplo-intervalo`, which asks for builds of a job every so many milliseconds;
//! - the pipeline format `exemplo-linhas`, one step per line, so a job can use the rest.
//!
//! `tools/raven-ci/docs/writing-a-plugin.md`, section "Um plugin Rust compilado pelo sistema",
//! explains it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    Attachment, BuildInfo, BuildRequest, BuildScope, Builds, CiError, CiPlugin, FormatError, Host,
    PLUGIN_KEY, Permission, Pipeline, PipelineFormat, Publisher, STAGE_KEY, Trigger, Use,
};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_flows::{Catalog, FlowDefinition, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec, Ports};
use serde_json::{Value, json};

/// The name of the plugin, as pipelines and the list of plugins say it.
const NAME: &str = "exemplo-rust";

/// The host the plugin got in `start`, for the step that reads a secret.
type SharedHost = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin: what a crate compiled into the server would be, unchanged.
#[derive(Default)]
struct Exemplo {
    host: SharedHost,
    watching: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
}

impl Module for Exemplo {
    fn name(&self) -> &str {
        NAME
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for Exemplo {
    fn summary(&self) -> String {
        "Exemplo em Rust: um passo, um publicador, um gatilho e um formato, compilado pelo raven-ci."
            .to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.log("exemplo-rust ligado");
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        for stop in self.watching.lock().unwrap_or_else(PoisonError::into_inner).values() {
            stop.store(true, Ordering::SeqCst);
        }
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Saudar { host: self.host.clone() });
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        vec![Arc::new(Relatorio)]
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        vec![Arc::new(Intervalo { watching: self.watching.clone() })]
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        vec![Arc::new(Linhas)]
    }
}

/// The step `exemplo-saudar`.
///
/// Parameters: `quem` (who to greet), `segredo` (the name of a secret to read through the host),
/// `cair` (ends the program in the middle of the call, to show the server stays up).
struct Saudar {
    host: SharedHost,
}

impl NodeKind for Saudar {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("exemplo-saudar", 1).title("Saudar").input("main").output("main")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        if cx.parameter("cair").and_then(Value::as_bool) == Some(true) {
            // What a plugin with a bug does: the server fails this call and goes on.
            std::process::exit(3);
        }
        let build = BuildScope::of(cx)?;
        let node = cx.node().id.clone();
        let quem = cx.parameter("quem").and_then(Value::as_str).unwrap_or("mundo");
        let saudacao = format!("Olá, {quem}!");
        build.log(&node, &saudacao);
        if let Some(name) = cx.parameter("segredo").and_then(Value::as_str) {
            let host = self.host.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let host = host.ok_or_else(|| NodeError::new("o plugin não foi ligado"))?;
            let value = host.secret(name).map_err(|e| NodeError::new(e.to_string()))?;
            // The host hides the value in the log of the build: this line shows ****.
            build.log(&node, &format!("o segredo {name} é {value}, com {} letras", value.chars().count()));
        }
        let file = build.context().workspace.join("saudacao.txt");
        let mut text = std::fs::read_to_string(&file).unwrap_or_default();
        text.push_str(&saudacao);
        text.push('\n');
        std::fs::write(&file, text).map_err(|e| NodeError::new(format!("{}: {e}", file.display())))?;
        let items = cx
            .input("main")
            .iter()
            .map(|item| {
                let mut json = item.json.clone();
                if let Value::Object(fields) = &mut json {
                    fields.insert("saudacao".to_owned(), json!(saudacao));
                }
                item.derive(json)
            })
            .collect();
        Ok(Ports::from([("main".to_owned(), items)]))
    }
}

/// The publisher `exemplo-relatorio`: keeps the file `arquivo` of the folder of the build
/// (`saudacao.txt` by default), with how many lines it has.
struct Relatorio;

impl Publisher for Relatorio {
    fn id(&self) -> &str {
        "exemplo-relatorio"
    }

    fn publish(
        &self,
        build: &BuildInfo,
        settings: &Value,
        workspace: &std::path::Path,
    ) -> Result<Vec<Attachment>, CiError> {
        let name = settings.get("arquivo").and_then(Value::as_str).unwrap_or("saudacao.txt");
        let bytes = std::fs::read(workspace.join(name)).map_err(|e| {
            CiError::failed(format!("o build {}#{} não deixou {name}: {e}", build.job, build.number))
        })?;
        let lines = bytes.iter().filter(|b| **b == b'\n').count();
        Ok(vec![Attachment::new("relatorio", name, "text/plain", bytes).summary(json!({ "linhas": lines }))])
    }
}

/// The trigger `exemplo-intervalo`: asks for a build of the job every `a_cada_ms` milliseconds,
/// `vezes` times.
struct Intervalo {
    watching: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
}

impl Trigger for Intervalo {
    fn id(&self) -> &str {
        "exemplo-intervalo"
    }

    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        let every = match settings.get("a_cada_ms") {
            None => 1000,
            Some(value) => value.as_u64().filter(|ms| *ms >= 50).ok_or_else(|| {
                CiError::settings("a_cada_ms", "tem que ser um número de milissegundos, de 50 para cima")
            })?,
        };
        let times = match settings.get("vezes") {
            None => 1,
            Some(value) => {
                value.as_u64().ok_or_else(|| CiError::settings("vezes", "tem que ser um número inteiro"))?
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(before) =
            self.watching.lock().unwrap_or_else(PoisonError::into_inner).insert(job.to_owned(), stop.clone())
        {
            before.store(true, Ordering::SeqCst);
        }
        let job = job.to_owned();
        std::thread::spawn(move || {
            for time in 1..=times {
                std::thread::sleep(Duration::from_millis(every));
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let _ = builds.request(BuildRequest::new(&job, format!("exemplo-intervalo, vez {time}")));
            }
        });
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        if let Some(stop) = self.watching.lock().unwrap_or_else(PoisonError::into_inner).remove(job) {
            stop.store(true, Ordering::SeqCst);
        }
    }
}

/// The pipeline format `exemplo-linhas`: one step per line, `saudar quem=Ana segredo=token`, and
/// `publicar arquivo=saudacao.txt`; `#` starts a comment.
struct Linhas;

impl PipelineFormat for Linhas {
    fn id(&self) -> &str {
        "exemplo-linhas"
    }

    fn file_names(&self) -> Vec<String> {
        vec![".raven-ci-exemplo".to_owned()]
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let mut flow = FlowDefinition::new(job, job).node(NodeDef::new("start", "manual"));
        let mut publishers = Vec::new();
        let mut errors = Vec::new();
        let mut last = "start".to_owned();
        for (at, line) in text.lines().enumerate() {
            let mut words = line.split('#').next().unwrap_or_default().split_whitespace();
            let Some(verb) = words.next() else { continue };
            let mut settings = serde_json::Map::new();
            for word in words {
                match word.split_once('=') {
                    Some((key, "sim" | "true")) => settings.insert(key.to_owned(), json!(true)),
                    Some((key, value)) => settings.insert(key.to_owned(), json!(value)),
                    None => {
                        errors.push(FormatError::at(at + 1, format!("{word}: escreva nome=valor")));
                        None
                    }
                };
            }
            match verb {
                "saudar" => {
                    let id = format!("saudar-{}", at + 1);
                    let mut node = NodeDef::new(&id, "exemplo-saudar")
                        .parameter(STAGE_KEY, json!("Saudações"))
                        .parameter(PLUGIN_KEY, json!(NAME));
                    for (key, value) in settings {
                        node = node.parameter(key, value);
                    }
                    flow = flow.node(node).connect(&last, "main", &id, "main");
                    last = id;
                }
                "publicar" => publishers.push(Use::new("exemplo-relatorio", NAME, Value::Object(settings))),
                other => {
                    errors.push(FormatError::at(at + 1, format!("{other}: o passo é saudar ou publicar")))
                }
            }
        }
        if last == "start" {
            errors.push(FormatError::whole("o pipeline não tem nenhum saudar"));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut pipeline = Pipeline::new(flow, "start");
        pipeline.publishers = publishers;
        Ok(pipeline)
    }
}

fn main() {
    raven_ci_plugin_api::serve(Exemplo::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_says_each_point_with_its_id() {
        let file =
            raven_ci_plugin_api::manifest(&Exemplo::default(), "./raven-ci-exemplo-rust").expect("fits");
        assert_eq!(file.name, NAME);
        assert_eq!(file.permissions, ["secrets"]);
        assert_eq!(file.steps[0].kind, "exemplo-saudar");
        assert_eq!(file.publishers[0].id, "exemplo-relatorio");
        assert_eq!(file.triggers[0].id, "exemplo-intervalo");
        assert_eq!(file.pipeline_formats[0].file_names, [".raven-ci-exemplo"]);
        assert_eq!(file.api.as_deref(), Some(raven_ci_plugin_api::protocol::API_VERSION));
    }

    #[test]
    fn the_format_reads_one_step_per_line_and_says_every_error() {
        let pipeline = Linhas
            .read("j", "saudar quem=Ana\nsaudar cair=sim # cai\npublicar arquivo=x.txt\n")
            .expect("reads");
        assert_eq!(pipeline.flow.nodes.len(), 3);
        assert_eq!(pipeline.publishers[0].settings["arquivo"], "x.txt");
        let errors = Linhas.read("j", "voar\nsaudar ana").expect_err("wrong");
        assert_eq!(errors.iter().map(|e| e.line).collect::<Vec<_>>(), [Some(1), Some(2)]);
    }
}
