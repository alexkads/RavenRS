//! From the tree of a declarative `Jenkinsfile` to the text of a `.raven-ci.toml`.
//!
//! Every part of the file has one of three ends, and none is guessed:
//! - **converted** into the TOML;
//! - **ignored**, with why: Raven CI already does it alone (`checkout scm`, `cleanWs()`), or it
//!   belongs elsewhere (`triggers` are set on the job);
//! - **not converted**, with its line and why: Groovy code, credentials bound by hand, steps with
//!   no counterpart. The TOML is still written for the rest.
//!
//! Some parts are converted with a difference worth saying (a `timeout` of a stage becomes the
//! limit of each of its steps); they are converted and noted as a **warning**.

use std::fmt::Write as _;

use crate::groovy::{self, Arg, At, Part, Stmt, Value};

/// How a part of the file ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    /// Converted, with a difference worth saying.
    Warning,
    /// Left out because Raven CI does it alone or it belongs elsewhere.
    Ignored,
    /// Not converted: the pipeline would not do the same without it.
    NotConverted,
}

impl NoteKind {
    fn word(self) -> &'static str {
        match self {
            Self::Warning => "aviso",
            Self::Ignored => "ignorado",
            Self::NotConverted => "não convertido",
        }
    }
}

/// A part of the `Jenkinsfile` that did not become the TOML as it was, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// How it ended.
    pub kind: NoteKind,
    /// The line in the `Jenkinsfile`, from 1.
    pub line: usize,
    /// The column, from 1.
    pub column: usize,
    /// What it is, as written: `script { }`, `withCredentials`.
    pub what: String,
    /// Why, in words.
    pub why: String,
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "linha {}:{}, {}: {} ({})", self.line, self.column, self.kind.word(), self.what, self.why)
    }
}

/// The TOML of a `Jenkinsfile`, and what did not become it as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversion {
    /// The `.raven-ci.toml`, the notes written as comments at its start.
    pub toml: String,
    /// Every note, in the order of the file.
    pub notes: Vec<Note>,
}

impl Conversion {
    /// Whether everything was converted (warnings and ignored parts do not count).
    pub fn complete(&self) -> bool {
        self.pending().next().is_none()
    }

    /// The parts not converted.
    pub fn pending(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(|note| note.kind == NoteKind::NotConverted)
    }
}

#[derive(Clone, Debug, Default)]
struct Step {
    script: String,
    name: Option<String>,
    timeout: Option<String>,
    retry: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct Container {
    image: String,
    args: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct Stage {
    name: String,
    branches: Vec<String>,
    pull_request: Option<bool>,
    parameter: Option<String>,
    timeout: Option<String>,
    retry: Option<u64>,
    lock: Option<String>,
    container: Option<Container>,
    steps: Vec<Step>,
    parallel: Vec<(String, Vec<Step>)>,
}

#[derive(Clone, Debug)]
struct Parameter {
    name: String,
    kind: &'static str,
    default: Option<Literal>,
    description: Option<String>,
    choices: Vec<String>,
}

#[derive(Clone, Debug)]
enum Literal {
    Text(String),
    Bool(bool),
}

#[derive(Default)]
struct Out {
    timeout: Option<String>,
    keep: Option<u64>,
    label: Option<String>,
    container: Option<Container>,
    environment: Vec<(String, String)>,
    parameters: Vec<Parameter>,
    stages: Vec<Stage>,
    post: Vec<(&'static str, Vec<Step>)>,
    artifacts: Vec<String>,
    junit: Vec<String>,
}

/// What a block around steps gives them: `dir`, `timeout`, `retry`.
#[derive(Clone, Default)]
struct Around {
    dir: Vec<String>,
    timeout: Option<String>,
    retry: Option<u64>,
}

struct Converter {
    out: Out,
    notes: Vec<Note>,
}

impl Converter {
    fn note(&mut self, kind: NoteKind, at: At, what: impl Into<String>, why: impl Into<String>) {
        self.notes.push(Note { kind, line: at.line, column: at.column, what: what.into(), why: why.into() });
    }

    fn pending(&mut self, at: At, what: impl Into<String>, why: impl Into<String>) {
        self.note(NoteKind::NotConverted, at, what, why);
    }

    fn unknown(&mut self, stmt: &Stmt, place: &str) {
        self.pending(stmt.at, shown(stmt), format!("{place} não tem {} no Raven CI", stmt.name));
    }

    fn pipeline(&mut self, pipeline: &Stmt) {
        for stmt in pipeline.body() {
            match stmt.name.as_str() {
                "agent" => self.agent(stmt),
                "environment" => self.environment(stmt),
                "parameters" => self.parameters(stmt),
                "options" => self.options(stmt, None),
                "triggers" => {
                    for trigger in stmt.body() {
                        let value =
                            trigger.positional().next().and_then(|a| a.value.text()).unwrap_or_default();
                        let how = match trigger.name.as_str() {
                            "cron" => format!("ponha cron = {value:?} no job (plugin cron)"),
                            "pollSCM" => format!("ponha poll = {value:?} no job (plugin cron)"),
                            _ => "os gatilhos ficam no job, não no pipeline".to_owned(),
                        };
                        self.note(NoteKind::Ignored, trigger.at, shown(trigger), how);
                    }
                }
                "stages" => {
                    for stage in stmt.body() {
                        if stage.name == "stage" {
                            if let Some(stage) = self.stage(stage) {
                                self.out.stages.push(stage);
                            }
                        } else {
                            self.unknown(stage, "stages");
                        }
                    }
                }
                "post" => self.post(stmt),
                "tools" => {
                    self.pending(stmt.at, "tools { }", "o Raven CI não instala ferramentas; use um container")
                }
                "libraries" => {
                    self.pending(stmt.at, "libraries { }", "bibliotecas compartilhadas são código Groovy")
                }
                _ => self.unknown(stmt, "o pipeline"),
            }
        }
    }

    fn agent(&mut self, stmt: &Stmt) {
        if let Some(arg) = stmt.positional().next() {
            match &arg.value {
                Value::Path(word) if word == "any" || word == "none" => {}
                _ => self.pending(stmt.at, shown(stmt), "agent diz any, none ou um bloco"),
            }
            return;
        }
        for kind in stmt.body() {
            match kind.name.as_str() {
                "label" => match kind.positional().next().and_then(|a| a.value.text()) {
                    Some(label) => self.out.label = Some(label),
                    None => self.pending(kind.at, shown(kind), "o rótulo tem que ser um texto"),
                },
                "node" => {
                    for inner in kind.body() {
                        match (inner.name.as_str(), inner.positional().next().and_then(|a| a.value.text())) {
                            ("label", Some(label)) => self.out.label = Some(label),
                            _ => self.unknown(inner, "agent { node }"),
                        }
                    }
                }
                "docker" => {
                    if let Some(container) = self.docker(kind) {
                        self.out.container = Some(container);
                    }
                }
                _ => self.pending(
                    kind.at,
                    shown(kind),
                    "o agente do Raven CI é any, um rótulo ou uma imagem docker",
                ),
            }
        }
    }

    /// `docker 'img'` or `docker { image 'img'; args '…' }`.
    fn docker(&mut self, stmt: &Stmt) -> Option<Container> {
        if let Some(image) = stmt.positional().next().and_then(|a| a.value.text()) {
            return Some(Container { image, args: None });
        }
        let mut container = Container::default();
        for field in stmt.body() {
            let text = field.positional().next().and_then(|a| a.value.text());
            match (field.name.as_str(), text) {
                ("image", Some(image)) => container.image = image,
                ("args", Some(args)) => container.args = Some(args),
                ("reuseNode", _) => self.note(
                    NoteKind::Ignored,
                    field.at,
                    shown(field),
                    "o container já roda na pasta do build",
                ),
                _ => self.unknown(field, "docker"),
            }
        }
        if container.image.is_empty() {
            self.pending(stmt.at, "docker", "falta a imagem (image)");
            return None;
        }
        Some(container)
    }

    fn environment(&mut self, stmt: &Stmt) {
        for entry in stmt.body() {
            match &entry.assigned {
                Some(Value::Str(text)) => match text.plain() {
                    Some(value) => self.out.environment.push((entry.name.clone(), value)),
                    None => self.pending(
                        entry.at,
                        format!("{} = \"…${{…}}\"", entry.name),
                        "o valor usa interpolação do Groovy; escreva o valor pronto",
                    ),
                },
                Some(Value::Number(number)) => {
                    self.out.environment.push((entry.name.clone(), number.clone()))
                }
                Some(Value::Call(call)) if call.name == "credentials" => self.pending(
                    entry.at,
                    format!("{} = credentials(…)", entry.name),
                    "credenciais no ambiente: cadastre no cofre (raven-ci credencial) e leia no passo",
                ),
                _ => self.pending(entry.at, shown(entry), "environment só aceita NOME = 'texto'"),
            }
        }
    }

    fn parameters(&mut self, stmt: &Stmt) {
        for parameter in stmt.body() {
            let kind = match parameter.name.as_str() {
                "booleanParam" => "bool",
                "string" | "text" => "text",
                "choice" => "choice",
                _ => {
                    self.unknown(parameter, "parameters");
                    continue;
                }
            };
            let Some(name) = parameter.named("name").and_then(|a| a.value.text()) else {
                self.pending(parameter.at, shown(parameter), "o parâmetro precisa de name");
                continue;
            };
            let default = parameter.named("defaultValue").and_then(|arg| match &arg.value {
                Value::Bool(b) => Some(Literal::Bool(*b)),
                value => value.text().map(Literal::Text),
            });
            let choices = match parameter.named("choices").map(|a| &a.value) {
                Some(Value::List(items)) => items.iter().filter_map(Value::text).collect(),
                Some(value) => {
                    value.text().map(|t| t.lines().map(str::to_owned).collect()).unwrap_or_default()
                }
                None => Vec::new(),
            };
            for arg in &parameter.args {
                if let Some(other) = arg.name.as_deref()
                    && !["name", "defaultValue", "description", "choices", "trim"].contains(&other)
                {
                    self.pending(
                        arg.at,
                        format!("{}: {other}", parameter.name),
                        "o parâmetro não tem essa opção",
                    );
                }
            }
            self.out.parameters.push(Parameter {
                name,
                kind,
                default,
                description: parameter.named("description").and_then(|a| a.value.text()),
                choices,
            });
        }
    }

    /// The `options` of the pipeline (`stage` none) or of a stage.
    fn options(&mut self, stmt: &Stmt, mut stage: Option<&mut Stage>) {
        for option in stmt.body() {
            match option.name.as_str() {
                "timeout" => match self.duration(option) {
                    Some(limit) => match stage.as_deref_mut() {
                        Some(stage) => {
                            self.note(
                                NoteKind::Warning,
                                option.at,
                                shown(option),
                                "o limite vale para cada passo do estágio, não para o estágio inteiro",
                            );
                            stage.timeout = Some(limit);
                        }
                        None => self.out.timeout = Some(limit),
                    },
                    None => self.pending(option.at, shown(option), "não entendi a duração"),
                },
                "buildDiscarder" => self.discarder(option),
                "retry" if stage.is_some() => {
                    match option.positional().next().and_then(|a| a.value.text()).and_then(|t| t.parse::<u64>().ok()) {
                        Some(times) => {
                            self.note(
                                NoteKind::Warning,
                                option.at,
                                shown(option),
                                "cada passo é tentado de novo, não o estágio inteiro",
                            );
                            if let Some(stage) = stage.as_deref_mut() {
                                stage.retry = times.checked_sub(1).filter(|n| *n > 0);
                            }
                        }
                        None => self.pending(option.at, shown(option), "retry precisa de um número"),
                    }
                }
                "lock" if stage.is_some() => match lock_name(option) {
                    Some(name) => {
                        if let Some(stage) = stage.as_deref_mut() {
                            stage.lock = Some(name);
                        }
                    }
                    None => self.pending(option.at, shown(option), "lock precisa do nome da trava"),
                },
                "timestamps" => {
                    self.note(NoteKind::Ignored, option.at, shown(option), "o log já guarda a hora de cada linha");
                }
                "ansiColor" => {
                    self.note(NoteKind::Ignored, option.at, shown(option), "o visualizador de log já mostra as cores");
                }
                "disableConcurrentBuilds" => self.pending(
                    option.at,
                    shown(option),
                    "o Raven CI não limita os builds de um job; use lock nos estágios que não podem correr juntos",
                ),
                "skipDefaultCheckout" => {
                    self.pending(option.at, shown(option), "o núcleo sempre busca o código antes dos passos");
                }
                _ => self.unknown(option, "options"),
            }
        }
    }

    fn discarder(&mut self, option: &Stmt) {
        let rotator = option.positional().next().and_then(|arg| match &arg.value {
            Value::Call(call) if call.name == "logRotator" => Some(call.as_ref().clone()),
            _ => None,
        });
        let Some(rotator) = rotator else {
            self.pending(option.at, shown(option), "buildDiscarder só é convertido com logRotator");
            return;
        };
        for arg in &rotator.args {
            match arg.name.as_deref() {
                Some("numToKeepStr" | "numToKeep") => {
                    match arg.value.text().and_then(|t| t.parse::<u64>().ok()) {
                        Some(keep) => self.out.keep = Some(keep),
                        None => self.pending(arg.at, "numToKeepStr", "tem que ser um número"),
                    }
                }
                Some(other) => {
                    let empty = arg.value.text().is_some_and(|t| t.is_empty() || t == "-1");
                    if !empty {
                        self.pending(
                            arg.at,
                            format!("logRotator: {other}"),
                            "o .raven-ci.toml só guarda quantos builds ficam (keep)",
                        );
                    }
                }
                None => self.pending(arg.at, "logRotator", "diga os argumentos pelo nome"),
            }
        }
    }

    /// `timeout(time: 30, unit: 'MINUTES')` or `timeout(30)` as `30m`.
    fn duration(&self, stmt: &Stmt) -> Option<String> {
        let time = stmt
            .named("time")
            .or_else(|| stmt.positional().next())
            .and_then(|a| a.value.text())
            .and_then(|t| t.parse::<u64>().ok())?;
        let unit = stmt.named("unit").map_or(Some("MINUTES".to_owned()), |a| match &a.value {
            Value::Path(path) => path.rsplit('.').next().map(str::to_owned),
            value => value.text(),
        })?;
        Some(match unit.as_str() {
            "MILLISECONDS" => format!("{time}ms"),
            "SECONDS" => format!("{time}s"),
            "MINUTES" => format!("{time}m"),
            "HOURS" => format!("{time}h"),
            "DAYS" => format!("{}h", time * 24),
            _ => return None,
        })
    }

    fn stage(&mut self, stmt: &Stmt) -> Option<Stage> {
        let Some(name) = stmt.positional().next().and_then(|a| a.value.text()) else {
            self.pending(stmt.at, "stage", "o estágio precisa de um nome em texto");
            return None;
        };
        let mut stage = Stage { name: name.clone(), ..Stage::default() };
        for part in stmt.body() {
            match part.name.as_str() {
                "steps" => stage.steps = self.steps(part.body(), &Around::default(), &name),
                "parallel" => {
                    for branch in part.body() {
                        if branch.name != "stage" {
                            self.unknown(branch, "parallel");
                            continue;
                        }
                        let Some(label) = branch.positional().next().and_then(|a| a.value.text()) else {
                            self.pending(branch.at, "stage", "o ramo precisa de um nome em texto");
                            continue;
                        };
                        let mut steps = Vec::new();
                        for inner in branch.body() {
                            match inner.name.as_str() {
                                "steps" => steps = self.steps(inner.body(), &Around::default(), &label),
                                _ => self.pending(
                                    inner.at,
                                    shown(inner),
                                    "um ramo de parallel só converte steps",
                                ),
                            }
                        }
                        stage.parallel.push((label, steps));
                    }
                }
                "when" => self.when(part, &mut stage),
                "options" => self.options(part, Some(&mut stage)),
                "agent" => {
                    for kind in part.body() {
                        match kind.name.as_str() {
                            "docker" => stage.container = self.docker(kind),
                            _ => self.pending(
                                kind.at,
                                shown(kind),
                                "por estágio, o Raven CI só troca a imagem (docker); o agente é do build todo",
                            ),
                        }
                    }
                    if let Some(arg) = part.positional().next()
                        && matches!(&arg.value, Value::Path(word) if word == "any")
                    {
                        self.note(
                            NoteKind::Ignored,
                            part.at,
                            "agent any",
                            "o estágio roda no agente do build",
                        );
                    }
                }
                "failFast" => {
                    self.pending(part.at, shown(part), "o Raven CI não para os outros ramos de parallel")
                }
                "environment" => self.pending(
                    part.at,
                    "environment { }",
                    "o ambiente por estágio não existe; ponha no do pipeline",
                ),
                "input" => self.pending(part.at, "input { }", "o Raven CI não para o build esperando alguém"),
                "post" => {
                    self.pending(part.at, "post { }", "o post por estágio não existe; use o do pipeline")
                }
                "stages" => self.pending(part.at, "stages { }", "estágios dentro de estágio não existem"),
                "tools" => {
                    self.pending(part.at, "tools { }", "o Raven CI não instala ferramentas; use um container")
                }
                _ => self.unknown(part, "o estágio"),
            }
        }
        if stage.steps.is_empty() && stage.parallel.is_empty() {
            self.pending(stmt.at, format!("stage('{name}')"), "o estágio ficou sem nenhum passo convertido");
            return None;
        }
        Some(stage)
    }

    fn when(&mut self, when: &Stmt, stage: &mut Stage) {
        for condition in when.body() {
            match condition.name.as_str() {
                "branch" => match condition.positional().next().and_then(|a| a.value.text()) {
                    Some(pattern) if condition.args.len() == 1 => stage.branches.push(pattern),
                    _ => self.pending(
                        condition.at,
                        shown(condition),
                        "branch só é convertido com o padrão em texto",
                    ),
                },
                "changeRequest" if condition.args.is_empty() => stage.pull_request = Some(true),
                "expression" => {
                    let source = condition.block.as_ref().and_then(|b| b.source.clone()).unwrap_or_default();
                    let text = source.strip_prefix("return").unwrap_or(&source).trim();
                    match text.strip_prefix("params.").filter(|name| is_name(name)) {
                        Some(name) => stage.parameter = Some(name.to_owned()),
                        None => self.pending(
                            condition.at,
                            format!("expression {{ {text} }}"),
                            "só é convertido expression { params.NOME }; o resto é código Groovy",
                        ),
                    }
                }
                "beforeAgent" | "beforeInput" | "beforeOptions" => {
                    self.note(
                        NoteKind::Ignored,
                        condition.at,
                        shown(condition),
                        "o when é decidido antes de tudo",
                    );
                }
                _ => self.pending(
                    condition.at,
                    shown(condition),
                    "o when converte branch, changeRequest() e expression { params.NOME }",
                ),
            }
        }
    }

    fn post(&mut self, stmt: &Stmt) {
        for condition in stmt.body() {
            let when = match condition.name.as_str() {
                "always" => "always",
                "success" => "success",
                "failure" => "failure",
                _ => {
                    self.pending(
                        condition.at,
                        format!("post {{ {} }}", condition.name),
                        "o post converte always, success e failure",
                    );
                    continue;
                }
            };
            let publishers = (self.out.artifacts.len(), self.out.junit.len());
            let steps = self.steps(condition.body(), &Around::default(), &format!("post {when}"));
            if when != "always" && publishers != (self.out.artifacts.len(), self.out.junit.len()) {
                self.note(
                    NoteKind::Warning,
                    condition.at,
                    format!("post {{ {when} {{ junit / archiveArtifacts }} }}"),
                    "os publicadores rodam em todo build, com qualquer resultado",
                );
            }
            if !steps.is_empty() {
                self.out.post.push((when, steps));
            }
        }
    }

    /// The steps of `body`, inside `around`.
    fn steps(&mut self, body: &[Stmt], around: &Around, place: &str) -> Vec<Step> {
        let mut steps = Vec::new();
        for stmt in body {
            match stmt.name.as_str() {
                "sh" => {
                    if let Some(step) = self.sh(stmt, around) {
                        steps.push(step);
                    }
                }
                "echo" => match stmt.positional().next().map(|a| self.shell_text(a)) {
                    Some(Some(text)) => steps.push(Step {
                        script: with_dir(around, &format!("echo {}", quoted(&text))),
                        name: None,
                        timeout: around.timeout.clone(),
                        retry: around.retry,
                    }),
                    Some(None) => {}
                    None => self.pending(stmt.at, "echo", "echo precisa de um texto"),
                },
                "dir" => match stmt.positional().next().and_then(|a| a.value.text()) {
                    Some(dir) => {
                        let mut inner = around.clone();
                        inner.dir.push(dir);
                        steps.extend(self.steps(stmt.body(), &inner, place));
                    }
                    None => self.pending(stmt.at, shown(stmt), "dir precisa da pasta em texto"),
                },
                "timeout" => match self.duration(stmt) {
                    Some(limit) => {
                        let mut inner = around.clone();
                        inner.timeout = Some(limit);
                        steps.extend(self.steps(stmt.body(), &inner, place));
                    }
                    None => self.pending(stmt.at, shown(stmt), "não entendi a duração"),
                },
                "retry" => {
                    match stmt
                        .positional()
                        .next()
                        .and_then(|a| a.value.text())
                        .and_then(|t| t.parse::<u64>().ok())
                    {
                        Some(times) => {
                            let inner_steps = stmt.body().len();
                            let mut inner = around.clone();
                            inner.retry = times.checked_sub(1).filter(|n| *n > 0);
                            if inner_steps > 1 {
                                self.note(
                                    NoteKind::Warning,
                                    stmt.at,
                                    shown(stmt),
                                    "cada passo é tentado de novo, não o bloco inteiro",
                                );
                            }
                            steps.extend(self.steps(stmt.body(), &inner, place));
                        }
                        None => self.pending(stmt.at, shown(stmt), "retry precisa de um número"),
                    }
                }
                "archiveArtifacts" => {
                    if let Some(paths) = self.paths(stmt, "artifacts", &["fingerprint"]) {
                        self.out.artifacts.extend(paths);
                    }
                }
                "junit" => {
                    if let Some(paths) = self.paths(stmt, "testResults", &["skipPublishingChecks"]) {
                        self.out.junit.extend(paths);
                    }
                }
                "checkout" => {
                    if matches!(stmt.positional().next().map(|a| &a.value), Some(Value::Path(p)) if p == "scm")
                    {
                        self.note(
                            NoteKind::Ignored,
                            stmt.at,
                            "checkout scm",
                            "o núcleo busca o código antes dos passos",
                        );
                    } else {
                        self.pending(
                            stmt.at,
                            shown(stmt),
                            "só checkout scm é convertido; outro repositório vai num sh com git",
                        );
                    }
                }
                "cleanWs" | "deleteDir" => self.note(
                    NoteKind::Ignored,
                    stmt.at,
                    shown(stmt),
                    "o plugin workspace-cleanup apaga a pasta do build no fim",
                ),
                "script" => self.pending(stmt.at, "script { }", "é código Groovy; reescreva como sh"),
                "withCredentials" => self.pending(
                    stmt.at,
                    "withCredentials",
                    "cadastre a credencial no cofre (raven-ci credencial); o log já esconde o valor",
                ),
                "bat" | "powershell" | "pwsh" => self.pending(
                    stmt.at,
                    stmt.name.clone(),
                    "escreva o comando como sh: no Windows, o passo sh roda no cmd",
                ),
                _ => self.pending(
                    stmt.at,
                    shown(stmt),
                    format!("o passo {} não tem equivalente no Raven CI ({place})", stmt.name),
                ),
            }
        }
        steps
    }

    fn sh(&mut self, stmt: &Stmt, around: &Around) -> Option<Step> {
        let script = stmt.named("script").or_else(|| stmt.positional().next());
        let Some(script) = script else {
            self.pending(stmt.at, "sh", "sh precisa do comando");
            return None;
        };
        for arg in &stmt.args {
            if let Some(other) = arg.name.as_deref()
                && !["script", "label"].contains(&other)
            {
                self.pending(
                    arg.at,
                    format!("sh {other}:"),
                    "o Raven CI não devolve a saída do sh para o pipeline",
                );
                return None;
            }
        }
        let text = self.shell_text(script)?;
        Some(Step {
            script: with_dir(around, &text),
            name: stmt.named("label").and_then(|a| a.value.text()),
            timeout: around.timeout.clone(),
            retry: around.retry,
        })
    }

    /// The text of a string for the shell: `${env.X}` and `${params.X}` become `${X}`, which the
    /// shell reads from the environment of the build, where each parameter is too. Any other
    /// interpolation is Groovy code, and is not converted.
    fn shell_text(&mut self, arg: &Arg) -> Option<String> {
        let Value::Str(text) = &arg.value else {
            self.pending(arg.at, "um valor que não é texto", "o comando tem que ser um texto");
            return None;
        };
        let mut out = String::new();
        for part in &text.parts {
            match part {
                Part::Text(piece) => out.push_str(piece),
                Part::Interpolation(expression) => {
                    let name = expression
                        .strip_prefix("env.")
                        .or_else(|| expression.strip_prefix("params."))
                        .unwrap_or(expression);
                    if is_name(name) {
                        let _ = write!(out, "${{{name}}}");
                    } else {
                        self.pending(
                            arg.at,
                            format!("${{{expression}}}"),
                            "a interpolação é código Groovy; use uma variável de ambiente",
                        );
                        return None;
                    }
                }
            }
        }
        Some(out)
    }

    /// The paths of `archiveArtifacts` or `junit`: positional or under `key`, split at commas.
    fn paths(&mut self, stmt: &Stmt, key: &str, ignored: &[&str]) -> Option<Vec<String>> {
        let mut paths = None;
        for arg in &stmt.args {
            match arg.name.as_deref() {
                None => paths = arg.value.text(),
                Some(name) if name == key => paths = arg.value.text(),
                Some(name) if ignored.contains(&name) => {
                    self.note(
                        NoteKind::Ignored,
                        arg.at,
                        format!("{}: {name}", stmt.name),
                        "o Raven CI não tem essa opção",
                    );
                }
                Some(name) => {
                    self.pending(arg.at, format!("{} {name}:", stmt.name), "o Raven CI não tem essa opção");
                    return None;
                }
            }
        }
        match paths {
            Some(paths) => {
                Some(paths.split(',').map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect())
            }
            None => {
                self.pending(stmt.at, stmt.name.clone(), "os caminhos têm que ser um texto");
                None
            }
        }
    }
}

fn lock_name(option: &Stmt) -> Option<String> {
    option.named("resource").or_else(|| option.positional().next()).and_then(|a| a.value.text())
}

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `cd 'a/b' && script`, for the folders of `dir` around a step.
fn with_dir(around: &Around, script: &str) -> String {
    if around.dir.is_empty() {
        return script.to_owned();
    }
    let path = around.dir.join("/");
    format!("cd {} && {script}", quoted(&path))
}

/// `text` for the shell: as it is when it is safe, between single quotes otherwise.
fn quoted(text: &str) -> String {
    let safe =
        !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if safe { text.to_owned() } else { format!("'{}'", text.replace('\'', "'\\''")) }
}

/// A statement as the person wrote it, short: `withCredentials(…)`, `cron('H 2 * * *')`.
fn shown(stmt: &Stmt) -> String {
    let first = stmt.args.first().map(|arg| {
        let value = match &arg.value {
            Value::Str(text) => {
                text.plain().map_or_else(|| "\"…\"".to_owned(), |t| format!("'{}'", short(&t)))
            }
            Value::Number(n) => n.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Path(p) => p.clone(),
            Value::Null => "null".to_owned(),
            _ => "…".to_owned(),
        };
        match &arg.name {
            Some(name) => format!("{name}: {value}"),
            None => value,
        }
    });
    let more = if stmt.args.len() > 1 { ", …" } else { "" };
    let block = if stmt.block.is_some() { " { }" } else { "" };
    match first {
        Some(first) => format!("{}({first}{more}){block}", stmt.name),
        None if stmt.block.is_some() => format!("{}{block}", stmt.name),
        None => format!("{}()", stmt.name),
    }
}

fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() > 40 || text.contains('\n') {
        format!("{}…", line.chars().take(40).collect::<String>())
    } else {
        line.to_owned()
    }
}

/// A TOML string: literal and many lines when the text has lines, basic and escaped otherwise.
fn toml_string(text: &str) -> String {
    let plain = !text.contains("'''") && text.chars().all(|c| c == '\n' || c == '\t' || !c.is_control());
    if text.contains('\n') && plain {
        return format!("'''\n{text}'''");
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn toml_list(items: &[String]) -> String {
    let items: Vec<String> = items.iter().map(|i| toml_string(i)).collect();
    format!("[{}]", items.join(", "))
}

/// A key of TOML: bare when it can be, quoted otherwise.
fn toml_key(key: &str) -> String {
    if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        key.to_owned()
    } else {
        toml_string(key)
    }
}

fn step_toml(step: &Step) -> String {
    if step.name.is_none() && step.timeout.is_none() && step.retry.is_none() {
        return toml_string(&step.script);
    }
    let mut fields = vec![format!("sh = {}", toml_string(&step.script))];
    if let Some(name) = &step.name {
        fields.push(format!("name = {}", toml_string(name)));
    }
    if let Some(timeout) = &step.timeout {
        fields.push(format!("timeout = {}", toml_string(timeout)));
    }
    if let Some(retry) = step.retry {
        fields.push(format!("retry = {retry}"));
    }
    format!("{{ {} }}", fields.join(", "))
}

fn steps_toml(steps: &[Step], indent: &str) -> String {
    let mut out = String::from("[\n");
    for step in steps {
        let _ = writeln!(out, "{indent}  {},", step_toml(step));
    }
    out.push_str(indent);
    out.push(']');
    out
}

fn container_toml(container: &Container) -> String {
    match &container.args {
        Some(args) => {
            format!("{{ image = {}, args = {} }}", toml_string(&container.image), toml_string(args))
        }
        None => format!("{{ image = {} }}", toml_string(&container.image)),
    }
}

impl Out {
    fn write(&self, notes: &[Note]) -> String {
        let mut out = String::from("# Convertido de um Jenkinsfile por raven-ci converter.\n");
        if !notes.is_empty() {
            out.push_str("#\n");
            for note in notes {
                let _ = writeln!(out, "# {note}");
            }
        }
        // Keys at the top come before any table, or TOML would put them inside it.
        let top: Vec<String> = [("artifacts", &self.artifacts), ("junit", &self.junit)]
            .into_iter()
            .filter(|(_, paths)| !paths.is_empty())
            .map(|(key, paths)| format!("{key} = {}", toml_list(paths)))
            .collect();
        if !top.is_empty() {
            out.push('\n');
            for line in top {
                out.push_str(&line);
                out.push('\n');
            }
        }
        if self.timeout.is_some() || self.keep.is_some() || self.label.is_some() {
            out.push_str("\n[pipeline]\n");
            if let Some(timeout) = &self.timeout {
                let _ = writeln!(out, "timeout = {}", toml_string(timeout));
            }
            if let Some(keep) = self.keep {
                let _ = writeln!(out, "keep = {keep}");
            }
            if let Some(label) = &self.label {
                let _ = writeln!(out, "agent = {{ label = {} }}", toml_string(label));
            }
        }
        if !self.environment.is_empty() {
            out.push_str("\n[environment]\n");
            for (name, value) in &self.environment {
                let _ = writeln!(out, "{} = {}", toml_key(name), toml_string(value));
            }
        }
        for parameter in &self.parameters {
            out.push_str("\n[[parameters]]\n");
            let _ = writeln!(out, "name = {}", toml_string(&parameter.name));
            let _ = writeln!(out, "type = \"{}\"", parameter.kind);
            match &parameter.default {
                Some(Literal::Bool(b)) => {
                    let _ = writeln!(out, "default = {b}");
                }
                Some(Literal::Text(text)) if parameter.kind == "bool" => {
                    let _ = writeln!(out, "default = {}", text == "true");
                }
                Some(Literal::Text(text)) => {
                    let _ = writeln!(out, "default = {}", toml_string(text));
                }
                None => {}
            }
            if !parameter.choices.is_empty() {
                let _ = writeln!(out, "choices = {}", toml_list(&parameter.choices));
            }
            if let Some(description) = &parameter.description {
                let _ = writeln!(out, "description = {}", toml_string(description));
            }
        }
        for stage in &self.stages {
            out.push_str("\n[[stages]]\n");
            let _ = writeln!(out, "name = {}", toml_string(&stage.name));
            let mut when = Vec::new();
            match stage.branches.as_slice() {
                [] => {}
                [one] => when.push(format!("branch = {}", toml_string(one))),
                many => when.push(format!("branch = {}", toml_list(many))),
            }
            if let Some(pr) = stage.pull_request {
                when.push(format!("pull_request = {pr}"));
            }
            if let Some(parameter) = &stage.parameter {
                when.push(format!("parameter = {}", toml_string(parameter)));
            }
            if !when.is_empty() {
                let _ = writeln!(out, "when = {{ {} }}", when.join(", "));
            }
            if let Some(timeout) = &stage.timeout {
                let _ = writeln!(out, "timeout = {}", toml_string(timeout));
            }
            if let Some(retry) = stage.retry {
                let _ = writeln!(out, "retry = {retry}");
            }
            if let Some(lock) = &stage.lock {
                let _ = writeln!(out, "lock = {}", toml_string(lock));
            }
            if let Some(container) = stage.container.as_ref().or(self.container.as_ref()) {
                let _ = writeln!(out, "container = {}", container_toml(container));
            }
            if stage.parallel.is_empty() {
                let _ = writeln!(out, "steps = {}", steps_toml(&stage.steps, ""));
            } else {
                out.push_str("parallel = [\n");
                for (name, steps) in &stage.parallel {
                    let _ = writeln!(
                        out,
                        "  {{ name = {}, steps = {} }},",
                        toml_string(name),
                        steps_toml(steps, "  ")
                    );
                }
                out.push_str("]\n");
            }
        }
        if !self.post.is_empty() {
            out.push_str("\n[post]\n");
            for (when, steps) in &self.post {
                let _ = writeln!(out, "{when} = {}", steps_toml(steps, ""));
            }
        }
        out
    }
}

/// Converts the declarative `Jenkinsfile` in `text`.
pub fn convert(text: &str) -> Conversion {
    let (stmts, problems) = groovy::parse(text);
    let mut converter = Converter { out: Out::default(), notes: Vec::new() };
    for problem in problems {
        converter.pending(problem.at, "um trecho", problem.message);
    }
    let mut seen = false;
    for stmt in &stmts {
        match stmt.name.as_str() {
            "pipeline" if !seen => {
                seen = true;
                converter.pipeline(stmt);
            }
            "node" => converter.pending(
                stmt.at,
                "node { }",
                "é um pipeline com script, não declarativo; o conversor só lê pipeline { }",
            ),
            _ => converter.unknown(stmt, "o arquivo"),
        }
    }
    if !seen && converter.notes.iter().all(|n| n.what != "node { }") {
        converter.pending(At { line: 1, column: 1 }, "o arquivo", "não achei o bloco pipeline { }");
    }
    converter.notes.sort_by_key(|note| (note.line, note.column));
    let toml = converter.out.write(&converter.notes);
    Conversion { toml, notes: converter.notes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_for_the_shell_is_quoted_only_when_it_needs() {
        assert_eq!(quoted("build/out"), "build/out");
        assert_eq!(quoted("a b"), "'a b'");
        assert_eq!(quoted("it's"), "'it'\\''s'");
    }

    #[test]
    fn toml_strings_are_literal_with_lines_and_escaped_otherwise() {
        assert_eq!(toml_string("a \"b\""), "\"a \\\"b\\\"\"");
        assert_eq!(toml_string("l1\nl2\n"), "'''\nl1\nl2\n'''");
        assert_eq!(toml_string("x'''\ny"), "\"x'''\\ny\"");
    }
}
