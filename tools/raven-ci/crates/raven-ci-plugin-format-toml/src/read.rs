//! Reading `.raven-ci.toml` into a [`Pipeline`], every error with its line.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use raven_ci_plugin_api::{FormatError, PLUGIN_KEY, Pipeline, PostSteps, PostWhen, STAGE_KEY, Use};
use ravenrs_app::{EnumOption, FieldInfo, FieldKind};
use ravenrs_flows::{FlowDefinition, NodeDef, Retry};
use serde_json::{Map, Value, json};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

type Node<'i> = Spanned<DeValue<'i>>;

/// The keys at the top of the file.
const TOP: &[&str] = &["pipeline", "environment", "parameters", "stages", "post", "artifacts", "junit"];
const PIPELINE: &[&str] = &["timeout", "keep", "agent"];
const STAGE: &[&str] =
    &["name", "steps", "parallel", "when", "timeout", "retry", "lock", "nice", "container", "credentials"];
const WHEN: &[&str] = &["branch", "pull_request", "parameter"];
const STEP: &[&str] =
    &["sh", "cargo", "uses", "with", "name", "timeout", "retry", "nice", "credentials", "keep_daemons"];
const CREDENTIAL_FILE: &[&str] = &["file", "name"];
const PARAMETER: &[&str] = &["name", "type", "default", "description", "choices"];
const BRANCH: &[&str] = &["name", "steps"];
const CONTAINER: &[&str] = &["image", "args", "env"];
const POST: &[&str] = &["always", "success", "failure"];

/// What a stage gives each of its steps.
#[derive(Clone, Default)]
struct StageOptions {
    name: String,
    timeout_ms: Option<u64>,
    retry: Option<u32>,
    nice: Option<i64>,
    container: Option<Value>,
    /// The credentials of every `sh` of the stage, by variable (#922).
    credentials: Map<String, Value>,
    disabled: bool,
}

struct Reader<'t> {
    text: &'t str,
    errors: Vec<FormatError>,
}

impl Reader<'_> {
    fn line(&self, span: &Range<usize>) -> usize {
        let end = span.start.min(self.text.len());
        self.text[..end].bytes().filter(|b| *b == b'\n').count() + 1
    }

    fn error(&mut self, span: &Range<usize>, message: impl Into<String>) {
        let line = self.line(span);
        self.errors.push(FormatError::at(line, message));
    }

    /// The fields of `table` by name, saying each one not in `allowed`, in `place`.
    fn fields<'a, 'i>(
        &mut self,
        table: &'a DeTable<'i>,
        allowed: &[&str],
        place: &str,
    ) -> BTreeMap<String, &'a Node<'i>> {
        let mut fields = BTreeMap::new();
        for (key, value) in table.iter() {
            let name: &str = key.get_ref().as_ref();
            if !allowed.contains(&name) {
                let known = allowed.join(", ");
                self.error(&key.span(), format!("{place} não tem o campo {name}; os campos são: {known}"));
                continue;
            }
            fields.insert(name.to_owned(), value);
        }
        fields
    }

    fn text(&mut self, node: &Node<'_>, what: &str) -> Option<String> {
        match node.get_ref() {
            DeValue::String(text) => Some(text.to_string()),
            _ => {
                self.error(&node.span(), format!("{what} tem que ser um texto, entre aspas"));
                None
            }
        }
    }

    fn integer(&mut self, node: &Node<'_>, what: &str) -> Option<i64> {
        let parsed = node
            .get_ref()
            .as_integer()
            .and_then(|n| i64::from_str_radix(&n.as_str().replace('_', ""), n.radix()).ok());
        if parsed.is_none() {
            self.error(&node.span(), format!("{what} tem que ser um número inteiro"));
        }
        parsed
    }

    fn boolean(&mut self, node: &Node<'_>, what: &str) -> Option<bool> {
        let value = node.get_ref().as_bool();
        if value.is_none() {
            self.error(&node.span(), format!("{what} tem que ser true ou false"));
        }
        value
    }

    fn table<'a, 'i>(&mut self, node: &'a Node<'i>, what: &str) -> Option<&'a DeTable<'i>> {
        match node.get_ref() {
            DeValue::Table(table) => Some(table),
            _ => {
                self.error(&node.span(), format!("{what} tem que ser uma tabela, entre chaves"));
                None
            }
        }
    }

    fn array<'a, 'i>(&mut self, node: &'a Node<'i>, what: &str) -> Option<&'a [Node<'i>]> {
        match node.get_ref() {
            DeValue::Array(items) => Some(items.as_ref()),
            _ => {
                self.error(&node.span(), format!("{what} tem que ser uma lista, entre colchetes"));
                None
            }
        }
    }

    /// `credentials = { TOKEN = "nome", USUARIO = "nome:user", ARQUIVO = { file = "nome", name =
    /// ".npmrc" } }`: the variables of a step and the credentials of the vault they get.
    fn credentials(&mut self, node: &Node<'_>) -> Map<String, Value> {
        let mut out = Map::new();
        let Some(table) = self.table(node, "credentials") else { return out };
        for (key, value) in table.iter() {
            let name: &str = key.get_ref().as_ref();
            let fine = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !fine {
                self.error(
                    &key.span(),
                    format!("credentials: {name:?} não é nome de variável (letras, números e _)"),
                );
                continue;
            }
            match value.get_ref() {
                DeValue::String(credential) if !credential.trim().is_empty() => {
                    out.insert(name.to_owned(), json!(credential.to_string()));
                }
                DeValue::Table(file) => {
                    let fields = self.fields(file, CREDENTIAL_FILE, "um arquivo de credencial");
                    match fields.get("file").and_then(|f| self.text(f, "file")) {
                        Some(credential) => {
                            let mut entry = Map::new();
                            entry.insert("file".to_owned(), json!(credential));
                            if let Some(file_name) = fields.get("name").and_then(|n| self.text(n, "name")) {
                                entry.insert("name".to_owned(), json!(file_name));
                            }
                            out.insert(name.to_owned(), Value::Object(entry));
                        }
                        None => self.error(&value.span(), format!("credentials.{name}: diga file = \"nome\"")),
                    }
                }
                _ => self.error(
                    &value.span(),
                    format!("credentials.{name}: o nome da credencial (\"nome\" ou \"nome:campo\") ou {{ file = \"nome\" }}"),
                ),
            }
        }
        out
    }

    fn duration(&mut self, node: &Node<'_>, what: &str) -> Option<u64> {
        let text = self.text(node, what)?;
        let parsed = parse_duration(&text);
        if parsed.is_none() {
            self.error(
                &node.span(),
                format!("{what}: não entendi a duração {text:?}; use 30s, 10m, 1h30m ou 500ms"),
            );
        }
        parsed
    }

    /// A list of texts, or one text.
    fn texts(&mut self, node: &Node<'_>, what: &str) -> Vec<String> {
        match node.get_ref() {
            DeValue::String(text) => vec![text.to_string()],
            DeValue::Array(items) => items.iter().filter_map(|item| self.text(item, what)).collect(),
            _ => {
                self.error(&node.span(), format!("{what} tem que ser um texto ou uma lista de textos"));
                Vec::new()
            }
        }
    }
}

/// `30s`, `10m`, `1h30m`, `500ms` in milliseconds, read by `ravenrs-schedule` as every period of
/// the house is.
fn parse_duration(text: &str) -> Option<u64> {
    let period = ravenrs_schedule::parse_period(text).ok()?;
    u64::try_from(period.as_millis()).ok()
}

/// A TOML value as JSON, for the parameters of a step.
fn json_of(node: &Node<'_>) -> Value {
    match node.get_ref() {
        DeValue::String(text) => json!(text.to_string()),
        DeValue::Integer(n) => {
            i64::from_str_radix(&n.as_str().replace('_', ""), n.radix()).map_or(Value::Null, |n| json!(n))
        }
        DeValue::Float(f) => f.as_str().parse::<f64>().map_or(Value::Null, |f| json!(f)),
        DeValue::Boolean(b) => json!(b),
        DeValue::Datetime(d) => json!(d.to_string()),
        DeValue::Array(items) => Value::Array(items.iter().map(json_of).collect()),
        DeValue::Table(table) => {
            Value::Object(table.iter().map(|(k, v)| (k.get_ref().to_string(), json_of(v))).collect())
        }
    }
}

/// Whether `head` is one of `patterns`, where a pattern ending in `*` takes what starts with it.
fn matches_branch(head: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| match pattern.strip_suffix('*') {
        Some(prefix) => head.starts_with(prefix),
        None => head == pattern,
    })
}

/// The flow being made: its nodes, and where the next node connects from.
struct Builder {
    flow: FlowDefinition,
    /// The outputs the next node connects from, as `(node, port)`.
    cursor: Vec<(String, String)>,
}

impl Builder {
    fn new(job: &str) -> Self {
        let flow = FlowDefinition::new(job, job).node(NodeDef::new("start", "manual").title("Início"));
        Self { flow, cursor: vec![("start".to_owned(), "main".to_owned())] }
    }

    /// Adds `node` after the cursor, joining the cursor first when it holds more than one output,
    /// and makes it the cursor.
    fn push(&mut self, node: NodeDef) {
        self.join();
        let id = node.id.clone();
        let flow = std::mem::replace(&mut self.flow, FlowDefinition::new("", ""));
        let mut flow = flow.node(node);
        for (from, port) in &self.cursor {
            flow = flow.connect(from, port, &id, "main");
        }
        self.flow = flow;
        self.cursor = vec![(id, "main".to_owned())];
    }

    /// Joins the outputs of the cursor, two at a time, in `merge` nodes that keep one copy of
    /// what reached them.
    fn join(&mut self) {
        while self.cursor.len() > 1 {
            let b = self.cursor.pop().unwrap_or_default();
            let a = self.cursor.pop().unwrap_or_default();
            let id = format!("junta-{}", self.flow.nodes.len());
            let flow = std::mem::replace(&mut self.flow, FlowDefinition::new("", ""));
            self.flow = flow
                .node(NodeDef::new(&id, "merge").title("Junta").parameter("mode", json!("first")))
                .connect(&a.0, &a.1, &id, "a")
                .connect(&b.0, &b.1, &id, "b");
            self.cursor.push((id, "main".to_owned()));
        }
    }
}

struct Read<'t> {
    reader: Reader<'t>,
    head: String,
    stage_names: BTreeSet<String>,
}

impl Read<'_> {
    /// The node of the step `node`, `id`, in a stage with `options`.
    fn step(&mut self, node: &Node<'_>, id: &str, options: &StageOptions) -> Option<NodeDef> {
        let mut def = match node.get_ref() {
            DeValue::String(script) => self.sh(id, script.as_ref(), options),
            DeValue::Table(table) => {
                let fields = self.reader.fields(table, STEP, "um passo");
                let kinds: Vec<&str> =
                    ["sh", "cargo", "uses"].into_iter().filter(|k| fields.contains_key(*k)).collect();
                if kinds.len() != 1 {
                    self.reader.error(&node.span(), "um passo diz um, e só um, de sh, cargo ou uses");
                    return None;
                }
                let mut def = match kinds[0] {
                    "sh" => {
                        let script = self.reader.text(fields["sh"], "sh")?;
                        self.sh(id, &script, options)
                    }
                    "cargo" => {
                        let command = self.reader.text(fields["cargo"], "cargo")?;
                        NodeDef::new(id, "cargo")
                            .title(format!("cargo {command}"))
                            .parameter("command", json!(command))
                            .parameter(PLUGIN_KEY, json!("cargo"))
                    }
                    _ => {
                        let uses = self.reader.text(fields["uses"], "uses")?;
                        let Some((plugin, kind)) =
                            uses.split_once('/').filter(|(p, k)| !p.is_empty() && !k.is_empty())
                        else {
                            self.reader.error(
                                &fields["uses"].span(),
                                format!("uses = {uses:?}: escreva plugin/passo, como kubernetes/kubectl"),
                            );
                            return None;
                        };
                        let mut def =
                            NodeDef::new(id, kind).title(uses.clone()).parameter(PLUGIN_KEY, json!(plugin));
                        if let Some(with) = fields.get("with")
                            && let Some(table) = self.reader.table(with, "with")
                        {
                            for (key, value) in table.iter() {
                                def = def.parameter(key.get_ref().to_string(), json_of(value));
                            }
                        }
                        def
                    }
                };
                if fields.contains_key("with") && kinds[0] != "uses" {
                    self.reader.error(&fields["with"].span(), "with só vale junto de uses");
                }
                if let Some(name) = fields.get("name").and_then(|n| self.reader.text(n, "name")) {
                    def.title = Some(name);
                }
                if let Some(ms) = fields.get("timeout").and_then(|t| self.reader.duration(t, "timeout")) {
                    def.timeout_ms = Some(ms);
                }
                if let Some(n) = fields.get("retry").and_then(|r| self.reader.integer(r, "retry")) {
                    def.retry = retry(n);
                }
                if let Some(level) = fields.get("nice").and_then(|n| self.reader.integer(n, "nice")) {
                    def = def.parameter("nice", json!(level));
                }
                if let Some(keep) =
                    fields.get("keep_daemons").and_then(|k| self.reader.boolean(k, "keep_daemons"))
                {
                    if def.kind == "sh" {
                        def = def.parameter("keep_daemons", json!(keep));
                    } else {
                        self.reader
                            .error(&fields["keep_daemons"].span(), "keep_daemons só vale num passo sh");
                    }
                }
                if let Some(credentials) = fields.get("credentials") {
                    let own = self.reader.credentials(credentials);
                    if def.kind == "sh" {
                        let mut all = match def.parameters.get("credentials") {
                            Some(Value::Object(stage)) => stage.clone(),
                            _ => Map::new(),
                        };
                        all.extend(own);
                        def = def.parameter("credentials", Value::Object(all));
                    } else {
                        self.reader
                            .error(&credentials.span(), "credentials só vale num passo sh, por enquanto");
                    }
                }
                def
            }
            _ => {
                self.reader.error(
                    &node.span(),
                    "um passo é um texto (o comando) ou uma tabela, como { sh = \"...\" }",
                );
                return None;
            }
        };
        def = def.parameter(STAGE_KEY, json!(options.name));
        if def.timeout_ms.is_none() {
            def.timeout_ms = options.timeout_ms;
        }
        if def.retry.is_none() {
            def.retry = options.retry.and_then(|n| retry(i64::from(n)));
        }
        def.disabled = options.disabled;
        Some(def)
    }

    /// A `sh` (or, in a stage with a container, a step of the plugin docker) running `script`.
    fn sh(&self, id: &str, script: &str, options: &StageOptions) -> NodeDef {
        let title: String = script.lines().next().unwrap_or_default().chars().take(60).collect();
        let mut def = match &options.container {
            Some(container) => NodeDef::new(id, "container")
                .parameter(PLUGIN_KEY, json!("docker"))
                .parameter("container", container.clone()),
            None => NodeDef::new(id, "sh").parameter(PLUGIN_KEY, json!("shell")),
        }
        .title(title)
        .parameter("script", json!(script));
        if let Some(level) = options.nice {
            def = def.parameter("nice", json!(level));
        }
        if !options.credentials.is_empty() && options.container.is_none() {
            def = def.parameter("credentials", Value::Object(options.credentials.clone()));
        }
        def
    }

    /// The steps of `list`, chained in `builder`, ids starting with `prefix`.
    fn steps(&mut self, builder: &mut Builder, list: &[Node<'_>], prefix: &str, options: &StageOptions) {
        for (n, step) in list.iter().enumerate() {
            if let Some(def) = self.step(step, &format!("{prefix}-{}", n + 1), options) {
                builder.push(def);
            }
        }
    }

    /// The stage `node`, the `index`th, added to `builder`.
    fn stage(&mut self, builder: &mut Builder, node: &Node<'_>, index: usize) {
        let Some(table) = self.reader.table(node, "um estágio") else { return };
        let fields = self.reader.fields(table, STAGE, "um estágio");
        let name = match fields.get("name") {
            Some(name) => self.reader.text(name, "name").unwrap_or_default(),
            None => {
                self.reader.error(&node.span(), format!("o estágio {} não tem name", index + 1));
                format!("Estágio {}", index + 1)
            }
        };
        if !self.stage_names.insert(name.clone()) {
            self.reader.error(&node.span(), format!("já há um estágio chamado {name}"));
        }
        let mut options = StageOptions { name: name.clone(), ..StageOptions::default() };
        options.timeout_ms = fields.get("timeout").and_then(|t| self.reader.duration(t, "timeout"));
        options.retry = fields
            .get("retry")
            .and_then(|r| self.reader.integer(r, "retry"))
            .map(|n| u32::try_from(n.max(0)).unwrap_or(0));
        options.nice = fields.get("nice").and_then(|n| self.reader.integer(n, "nice"));
        if let Some(credentials) = fields.get("credentials") {
            options.credentials = self.reader.credentials(credentials);
            if fields.contains_key("container") {
                self.reader
                    .error(&credentials.span(), "credentials ainda não vale num estágio com container");
            }
        }
        if let Some(container) = fields.get("container")
            && let Some(table) = self.reader.table(container, "container")
        {
            let given = self.reader.fields(table, CONTAINER, "container");
            if !given.contains_key("image") {
                self.reader.error(&container.span(), "container precisa de image");
            }
            options.container = Some(json_of(container));
        }

        // When: decided now by the head, or as the build runs by a parameter.
        let mut by_parameter = None;
        if let Some(when) = fields.get("when")
            && let Some(table) = self.reader.table(when, "when")
        {
            let given = self.reader.fields(table, WHEN, "when");
            if let Some(branch) = given.get("branch") {
                let patterns = self.reader.texts(branch, "when.branch");
                if !matches_branch(&self.head, &patterns) {
                    options.disabled = true;
                }
            }
            if let Some(pr) =
                given.get("pull_request").and_then(|p| self.reader.boolean(p, "when.pull_request"))
                && pr != self.head.starts_with("PR-")
            {
                options.disabled = true;
            }
            if let Some(parameter) = given.get("parameter") {
                by_parameter = self.reader.text(parameter, "when.parameter");
            }
        }

        let mut skip = None;
        if let Some(parameter) = &by_parameter {
            let id = format!("quando-{}", index + 1);
            builder.push(
                NodeDef::new(&id, "if")
                    .title(format!("Quando {parameter}"))
                    .parameter("condition", json!(format!("[params.{parameter}]")))
                    .parameter(STAGE_KEY, json!(name)),
            );
            builder.cursor = vec![(id.clone(), "true".to_owned())];
            skip = Some((id, "false".to_owned()));
        }

        if let Some(lock) = fields.get("lock").and_then(|l| self.reader.text(l, "lock")) {
            let id = format!("trava-{}", index + 1);
            let mut def = NodeDef::new(&id, "lock")
                .title(format!("Trava {lock}"))
                .parameter("name", json!(lock))
                .parameter(STAGE_KEY, json!(name))
                .parameter(PLUGIN_KEY, json!("lock"));
            def.disabled = options.disabled;
            builder.push(def);
        }

        let prefix = format!("e{}", index + 1);
        match (fields.get("steps"), fields.get("parallel")) {
            (Some(_), Some(parallel)) => {
                self.reader
                    .error(&parallel.span(), format!("o estágio {name} diz steps e parallel; diga um só"));
            }
            (None, None) => {
                self.reader.error(&node.span(), format!("o estágio {name} não tem steps nem parallel"));
            }
            (Some(steps), None) => {
                if let Some(list) = self.reader.array(steps, "steps") {
                    if list.is_empty() {
                        self.reader
                            .error(&steps.span(), format!("o estágio {name} tem a lista de steps vazia"));
                    }
                    self.steps(builder, list, &prefix, &options);
                }
            }
            (None, Some(parallel)) => {
                if let Some(branches) = self.reader.array(parallel, "parallel") {
                    let from = builder.cursor.clone();
                    let mut ends = Vec::new();
                    for (b, branch) in branches.iter().enumerate() {
                        let Some(table) = self.reader.table(branch, "um ramo de parallel") else { continue };
                        let given = self.reader.fields(table, BRANCH, "um ramo de parallel");
                        let label = given
                            .get("name")
                            .and_then(|n| self.reader.text(n, "name"))
                            .unwrap_or_else(|| format!("{}", b + 1));
                        builder.cursor = from.clone();
                        match given.get("steps").and_then(|s| self.reader.array(s, "steps")) {
                            Some(list) => {
                                let branch_options =
                                    StageOptions { name: format!("{name} / {label}"), ..options.clone() };
                                self.steps(builder, list, &format!("{prefix}-{}", b + 1), &branch_options);
                            }
                            None => {
                                self.reader.error(&branch.span(), format!("o ramo {label} não tem steps"))
                            }
                        }
                        ends.extend(builder.cursor.clone());
                    }
                    builder.cursor = ends;
                }
            }
        }

        if fields.contains_key("lock") {
            let id = format!("solta-{}", index + 1);
            let mut def = NodeDef::new(&id, "unlock")
                .title("Solta a trava")
                .parameter("name", fields.get("lock").map(|l| json_of(l)).unwrap_or(Value::Null))
                .parameter(STAGE_KEY, json!(name))
                .parameter(PLUGIN_KEY, json!("lock"));
            def.disabled = options.disabled;
            builder.push(def);
        }

        if let Some(skip) = skip {
            builder.join();
            builder.cursor.push(skip);
            builder.join();
        }
    }

    fn parameter(&mut self, node: &Node<'_>) -> Option<FieldInfo> {
        let table = self.reader.table(node, "um parâmetro")?;
        let fields = self.reader.fields(table, PARAMETER, "um parâmetro");
        let Some(name) = fields.get("name").and_then(|n| self.reader.text(n, "name")) else {
            self.reader.error(&node.span(), "um parâmetro precisa de name");
            return None;
        };
        let kind_name =
            fields.get("type").and_then(|t| self.reader.text(t, "type")).unwrap_or_else(|| "text".to_owned());
        let kind = match kind_name.as_str() {
            "bool" => FieldKind::Boolean,
            "text" => FieldKind::Text,
            "choice" => {
                let choices =
                    fields.get("choices").map(|c| self.reader.texts(c, "choices")).unwrap_or_default();
                if choices.is_empty() {
                    self.reader
                        .error(&node.span(), format!("o parâmetro {name} é choice e precisa de choices"));
                }
                FieldKind::Enum(choices.iter().map(|c| EnumOption::new(c.clone(), c.clone())).collect())
            }
            other => {
                let at = fields.get("type").map_or_else(|| node.span(), |t| t.span());
                self.reader.error(&at, format!("o tipo {other} não existe; use bool, text ou choice"));
                FieldKind::Text
            }
        };
        let mut field = FieldInfo::new(&name, kind);
        if let Some(default) = fields.get("default") {
            field.default_value = ravenrs_app::Value::from_json(&json_of(default));
        }
        if let Some(description) = fields.get("description").and_then(|d| self.reader.text(d, "description"))
        {
            field.caption = description;
        }
        Some(field)
    }

    /// The paths of `artifacts` or `junit`: a list of texts, or a table with `paths`.
    fn publisher(&mut self, node: &Node<'_>, id: &str) -> Use {
        let settings = match node.get_ref() {
            DeValue::Table(_) => json_of(node),
            _ => json!({ "paths": self.reader.texts(node, id) }),
        };
        Use::new(id, id, settings)
    }
}

fn retry(again: i64) -> Option<Retry> {
    let again = u32::try_from(again).ok().filter(|n| *n > 0)?;
    Some(Retry { tries: again + 1, wait_ms: 0, backoff: 1.0, on_timeout: false })
}

/// Reads `text`, the pipeline of `job`.
pub(crate) fn read(job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
    let (root, errors) = DeTable::parse_recoverable(text);
    let mut read = Read {
        reader: Reader { text, errors: Vec::new() },
        head: job.rsplit('/').next().unwrap_or(job).to_owned(),
        stage_names: BTreeSet::new(),
    };
    for error in &errors {
        let span = error.span().unwrap_or(0..0);
        read.reader.error(&span, format!("o TOML não fecha: {}", error.message()));
    }
    if !errors.is_empty() {
        return Err(read.reader.errors);
    }
    let root = root.get_ref();
    let top = read.reader.fields(root, TOP, "o arquivo");
    let mut builder = Builder::new(job);
    let mut pipeline = Pipeline::new(FlowDefinition::new(job, job), "start");

    if let Some(options) = top.get("pipeline").and_then(|p| read.reader.table(p, "pipeline")) {
        let fields = read.reader.fields(options, PIPELINE, "pipeline");
        pipeline.timeout_ms = fields.get("timeout").and_then(|t| read.reader.duration(t, "timeout"));
        pipeline.keep_builds = fields
            .get("keep")
            .and_then(|k| read.reader.integer(k, "keep"))
            .and_then(|k| u32::try_from(k).ok());
        if let Some(agent) = fields.get("agent") {
            pipeline.agent = agent_of(&mut read.reader, agent);
        }
    }
    if let Some(environment) = top.get("environment").and_then(|e| read.reader.table(e, "environment")) {
        for (key, value) in environment.iter() {
            let name = key.get_ref().to_string();
            if let Some(value) = read.reader.text(value, &format!("environment.{name}")) {
                pipeline.environment.insert(name, value);
            }
        }
    }
    if let Some(parameters) = top.get("parameters").and_then(|p| read.reader.array(p, "parameters")) {
        let mut names = BTreeSet::new();
        for parameter in parameters {
            if let Some(field) = read.parameter(parameter) {
                if !names.insert(field.name.clone()) {
                    read.reader
                        .error(&parameter.span(), format!("já há um parâmetro chamado {}", field.name));
                }
                pipeline.parameters.push(field);
            }
        }
    }
    match top.get("stages") {
        Some(stages) => {
            if let Some(list) = read.reader.array(stages, "stages") {
                for (index, stage) in list.iter().enumerate() {
                    read.stage(&mut builder, stage, index);
                }
            }
        }
        None => read.reader.errors.push(FormatError::whole("o arquivo não tem nenhum [[stages]]")),
    }
    if let Some(post) = top.get("post").and_then(|p| read.reader.table(p, "post")) {
        let fields = read.reader.fields(post, POST, "post");
        for (when, name) in
            [(PostWhen::Always, "always"), (PostWhen::Success, "success"), (PostWhen::Failure, "failure")]
        {
            let Some(list) = fields.get(name).and_then(|l| read.reader.array(l, &format!("post.{name}")))
            else {
                continue;
            };
            let mut post_builder = Builder::new(job);
            let options = StageOptions { name: format!("post {name}"), ..StageOptions::default() };
            read.steps(&mut post_builder, list, &format!("post-{name}"), &options);
            pipeline.post.push(PostSteps { when, flow: post_builder.flow, start: "start".to_owned() });
        }
    }
    for id in ["artifacts", "junit"] {
        if let Some(node) = top.get(id) {
            pipeline.publishers.push(read.publisher(node, id));
        }
    }
    pipeline.flow = builder.flow;
    // A step cargo keeps its diagnostics; its plugin's publisher turns them into an attachment.
    let has_cargo = |flow: &FlowDefinition| flow.nodes.iter().any(|node| node.kind == "cargo");
    if (has_cargo(&pipeline.flow) || pipeline.post.iter().any(|post| has_cargo(&post.flow)))
        && !pipeline.publishers.iter().any(|publisher| publisher.id == "cargo")
    {
        pipeline.publishers.push(Use::new("cargo", "cargo", json!({})));
    }
    if read.reader.errors.is_empty() {
        Ok(pipeline)
    } else {
        read.reader.errors.sort_by_key(|e| e.line.unwrap_or(0));
        Err(read.reader.errors)
    }
}

fn agent_of(reader: &mut Reader<'_>, node: &Node<'_>) -> Option<Use> {
    match node.get_ref() {
        DeValue::String(text) if text.as_ref() == "any" => None,
        DeValue::Table(table) => {
            let fields = reader.fields(table, &["label", "docker", "kubernetes"], "agent");
            if fields.len() != 1 {
                reader.error(&node.span(), "agent diz um, e só um, de label, docker ou kubernetes");
                return None;
            }
            let (kind, value) = fields.into_iter().next()?;
            match kind.as_str() {
                "label" => {
                    let label = reader.text(value, "agent.label")?;
                    Some(Use { id: "label".to_owned(), plugin: None, settings: json!({ "label": label }) })
                }
                other => Some(Use::new(other, other, json_of(value))),
            }
        }
        _ => {
            reader.error(&node.span(), "agent é \"any\" ou uma tabela, como { label = \"linux\" }");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_read_in_hours_minutes_seconds_and_milliseconds() {
        assert_eq!(parse_duration("30s"), Some(30_000));
        assert_eq!(parse_duration("1h30m"), Some(5_400_000));
        assert_eq!(parse_duration("500ms"), Some(500));
        assert_eq!(parse_duration("10"), None);
        assert_eq!(parse_duration("dez minutos"), None);
    }

    #[test]
    fn a_pattern_ending_in_a_star_takes_what_starts_with_it() {
        let patterns = vec!["main".to_owned(), "release/*".to_owned()];
        assert!(matches_branch("main", &patterns));
        assert!(matches_branch("release/1.0", &patterns));
        assert!(!matches_branch("PR-3", &patterns));
    }
}
