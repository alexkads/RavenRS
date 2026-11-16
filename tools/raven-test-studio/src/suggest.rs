//! Scenarios written by an AI for an app of a project the Studio was pointed at: when the app
//! has none, and to suggest others it does not have yet.
//!
//! The Studio opens the app on the test channel and reads its first screen as text, takes the
//! views and test ids [`crate::project`] found in its sources and the names of the scenarios it
//! already has, and asks an AI for `.raventest` files. The AI is local by default (a text model
//! of `ravenrs-ai` already downloaded on this computer); Codex or Claude, with their cheapest
//! model and reading only, when the person asks for them. The Studio does not trust what comes
//! back: it reads each scenario and runs it on the app, and says which passed.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ravenrs_e2e_wire::Scenario;
use serde_json::{Value, json};

use crate::project::Found;
use crate::report::Report;
use crate::runner::Run;

/// Who writes the scenarios.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    /// A text model on this computer, the default.
    Local,
    /// Codex or Claude, with this model or its cheapest.
    Agent(ravenrs_agent_cli::Agent, Option<String>),
}

/// One scenario the AI wrote, and how it went when the Studio ran it.
#[derive(Clone, Debug)]
pub struct Suggestion {
    /// The text as the AI wrote it.
    pub text: String,
    /// The scenario, when it reads.
    pub scenario: Option<Scenario>,
    /// Why it does not read, or does not open the app.
    pub problem: Option<String>,
    /// How it went on the app, when it read.
    pub report: Option<Report>,
    /// Where it was written, with `--write`.
    pub written: Option<PathBuf>,
}

impl Suggestion {
    /// Whether it read and passed on the app.
    pub fn passed(&self) -> bool {
        self.report.as_ref().is_some_and(Report::passed)
    }

    /// Its name: from the scenario, or from the `scenario:` line of a text that did not read.
    pub fn name(&self) -> String {
        self.scenario.as_ref().map_or_else(
            || {
                let first = self.text.lines().next().unwrap_or_default();
                first.split_once(':').map_or("(sem nome)", |(_, name)| name.trim()).to_owned()
            },
            |s| s.name.clone(),
        )
    }
}

/// What the AI suggested for one app.
#[derive(Clone, Debug)]
pub struct Suggestions {
    /// The app's name.
    pub app: String,
    /// Who wrote them: the local model's name, or the agent and its model.
    pub writer: String,
    /// The scenarios.
    pub list: Vec<Suggestion>,
    /// How long the AI took, in milliseconds.
    pub duration_ms: u64,
}

/// The `.raventest` format, for the AI.
const FORMAT: &str = "Format of a .raventest file, one block per line:\n\
scenario: <name of the scenario>\n\
open <app name> | <title>\n\
click <target> | <title>\n\
type <target> = <text> | <title>\n\
fill <target> = <text that replaces what the field has> | <title>\n\
select <target> = <option shown> | <title>\n\
expand <target> | <title>\n\
collapse <target> | <title>\n\
wait <target> = <text it comes to show> | <title>\n\
check <target> = <exact value or name> | <title>\n\
mention <target> = <text it contains> | <title>\n\
A target is #<test id> or the exact text shown on screen. The first block opens the app. \
type, fill, select, check and mention always have ` = ` and a text after the target; click, \
expand and collapse never have. Act only on the targets listed below; never invent a test id.";

/// A whole scenario, for the AI to copy the shape of when the app has none of its own.
const EXAMPLE: &str = "scenario: Salvar um contato\n\
open Agenda | Abrir a agenda\n\
click Novo contato | Novo contato\n\
type #field:name = Ana Souza | Digitar o nome\n\
click #form:save | Salvar\n\
check Contato salvo = Contato salvo | O aviso diz que salvou\n";

/// The question for the AI: the format, the screen, the views and test ids, and what exists.
pub fn prompt(app: &str, screen: &str, found: &Found, existing: &[String]) -> (String, String) {
    // The app's own first scenario shows the shape best; a generic one when it has none.
    let example = found
        .scenarios
        .iter()
        .find_map(|(file, read)| read.as_ref().ok().and_then(|_| std::fs::read_to_string(file).ok()))
        .unwrap_or_else(|| EXAMPLE.to_owned());
    let system = format!(
        "You write end-to-end test scenarios for a RavenRS app, in the .raventest format. {FORMAT}\n\
         Answer only with the scenarios, each starting with its scenario: line, blocks written in \
         English words, titles in Brazilian Portuguese. Write 3 short scenarios (4 to 8 blocks), \
         each testing one thing a person does on this app.\n\nA complete example:\n{example}"
    );
    let mut ids: Vec<&str> =
        found.screens.iter().flat_map(|s| s.test_ids.iter().map(String::as_str)).collect();
    ids.sort_unstable();
    ids.dedup();
    let views: Vec<String> = found
        .screens
        .iter()
        .take(40)
        .map(|s| format!("{} ({}:{})", s.name, s.file.display(), s.line))
        .collect();
    let mut user = format!(
        "App: {app}\n\nFirst screen, as text (one line per part: role, name, #test id, value):\n{screen}\n"
    );
    let on_screen = targets_on(screen);
    if !on_screen.is_empty() {
        user.push_str(&format!(
            "\nTargets on the first screen, the only ones to click or type into there: {}\n",
            on_screen.join(", ")
        ));
    }
    user.push_str(&format!("\nViews in its sources: {}\n", views.join(", ")));
    if !ids.is_empty() {
        user.push_str(&format!(
            "Test ids declared in its sources, for the screens a click opens: {}\n",
            ids.join(", ")
        ));
    }
    user.push_str(
        "Generated forms tag each editor #field:<field> and their buttons #form:save and #form:cancel.\n",
    );
    if existing.is_empty() {
        user.push_str("It has no scenario yet.\n");
    } else {
        user.push_str(&format!("It already has these scenarios; suggest others: {}\n", existing.join("; ")));
    }
    (system, user)
}

/// The targets a screen read over the test channel shows: `#id` for each test id and the name
/// of each part, as [`ravenrs_e2e::E2e::snapshot`] writes them, in the order they show.
pub fn targets_on(screen: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |target: String| {
        if !target.is_empty() && !out.contains(&target) {
            out.push(target);
        }
    };
    for line in screen.lines() {
        let line = line.trim_start().trim_start_matches("- ");
        if let Some(start) = line.find('"')
            && let Some(len) = line[start + 1..].find('"')
        {
            add(line[start + 1..start + 1 + len].to_owned());
        }
        for word in line.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                add(word.to_owned());
            }
        }
    }
    out
}

/// The first block of `scenario` that acts on a target the app does not show: not on its first
/// screen, not a test id its sources declare, not a field or button of a generated form. A check,
/// a mention or a wait may name what a later screen shows, which no one knows before running.
pub fn unknown_target(scenario: &Scenario, on_screen: &[String], declared: &[String]) -> Option<String> {
    use ravenrs_e2e_wire::Kind;
    scenario.blocks.iter().skip(1).find_map(|block| {
        if matches!(block.kind, Kind::Check | Kind::Mention | Kind::Wait) {
            return None;
        }
        let target = block.target.trim();
        let known = on_screen.iter().any(|t| t == target)
            || target.strip_prefix('#').is_some_and(|id| {
                declared.iter().any(|d| d == id)
                    || id.starts_with("field:")
                    || matches!(id, "form:save" | "form:cancel")
            });
        (!known).then(|| {
            format!("{} {target}: não está na primeira tela nem nos test ids do app", block.kind.word())
        })
    })
}

/// The scenarios in an AI's answer: each from its `scenario:` line to the next one.
pub fn scenarios_in(answer: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in answer.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            continue;
        }
        let head = trimmed.to_lowercase();
        if head.starts_with("scenario:") || head.starts_with("cenario:") || head.starts_with("cenário:") {
            out.push(format!("{trimmed}\n"));
        } else if let Some(current) = out.last_mut()
            && !trimmed.is_empty()
        {
            current.push_str(trimmed);
            current.push('\n');
        }
    }
    out
}

/// Asks the writer, and gives back its answer and who it was.
fn ask(writer: &Writer, system: &str, user: &str, project: &Path) -> Result<(String, String), String> {
    match writer {
        Writer::Local => ask_local(system, user),
        Writer::Agent(agent, model) => {
            let cli = ravenrs_agent_cli::Cli::new(*agent);
            let model = model.clone().or_else(|| ravenrs_agent_cli::cheapest_model(*agent));
            let policy = ravenrs_agent_cli::Policy {
                read_only: true,
                dir: Some(project.to_path_buf()),
                timeout: Duration::from_secs(600),
                model: model.clone(),
                ..ravenrs_agent_cli::Policy::default()
            };
            let answer = cli
                .run(&format!("{system}\n\n{user}"), &policy, None, &|| false)
                .map_err(|e| format!("o {} não rodou: {e}", agent.program()))?;
            let who =
                format!("{} ({})", agent.program(), model.unwrap_or_else(|| "modelo padrão".to_owned()));
            Ok((answer.final_message.unwrap_or_default(), who))
        }
    }
}

/// The folders where a text model may already be on this computer: the Studio's own, and the
/// one of Raven Finanças, whose assistant downloads them.
fn model_folders() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut folders = Vec::new();
    if let Some(cache) = crate::cache::default_file().and_then(|f| f.parent().map(|p| p.join("modelos"))) {
        folders.push(cache);
    }
    if let Some(home) = home {
        folders.push(home.join(".raven-financas").join("modelos"));
    }
    folders
}

/// The largest text model already downloaded, and its folder.
fn local_model() -> Option<(ravenrs_ai::ModelFolder, &'static ravenrs_ai::LocalModel)> {
    model_folders().into_iter().find_map(|root| {
        let folder = ravenrs_ai::ModelFolder::new(root);
        ravenrs_ai::CATALOG
            .iter()
            .rev()
            .find(|model| folder.state(model) == ravenrs_ai::ModelState::Installed)
            .map(|model| (folder, model))
    })
}

fn ask_local(system: &str, user: &str) -> Result<(String, String), String> {
    let (folder, model) = local_model().ok_or_else(|| {
        format!(
            "não há modelo de texto local neste computador ({}); baixe um pelo assistente do Raven Finanças, ou use --delegate codex",
            model_folders().iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join(", ")
        )
    })?;
    let mut engine = ravenrs_ai::LocalEngine::load(&folder, model).map_err(|e| e.to_string())?;
    let mut text = String::new();
    engine
        .generate(system, user, 1200, &mut |piece| {
            text.push_str(piece);
            true
        })
        .map_err(|e| e.to_string())?;
    Ok((text, format!("{} (local)", model.name)))
}

/// Suggests scenarios for `found`, runs each on the app, and writes them into
/// `tests/e2e/sugestoes/` of the app when `write` is set.
pub fn suggest(found: &Found, project: &Path, writer: &Writer, write: bool) -> Result<Suggestions, String> {
    if !found.is_app() || !found.channel {
        return Err(format!(
            "o {} não abre pelo canal de teste: sem ele o Studio não vê a tela para sugerir",
            found.name
        ));
    }
    let screen = match crate::apps::open(&found.name) {
        Some(Ok(mut session)) => session.screen().unwrap_or_default(),
        Some(Err(why)) => return Err(why),
        None => return Err(format!("o Studio não sabe abrir o {}", found.name)),
    };
    let existing: Vec<String> = found.scenarios.iter().filter_map(|(_, read)| read.clone().ok()).collect();
    let (system, user) = prompt(&found.name, &screen, found, &existing);
    let started = Instant::now();
    let (answer, who) = ask(writer, &system, &user, project)?;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let folder = found.folder.join(ravenrs_e2e_wire::FOLDER).join("sugestoes");
    // The blocks of the scenarios the app has: a suggestion with the same ones is a copy.
    let had: Vec<Vec<(ravenrs_e2e_wire::Kind, String, Option<String>)>> = found
        .scenarios
        .iter()
        .filter_map(|(file, _)| std::fs::read_to_string(file).ok())
        .filter_map(|text| Scenario::parse(&text).ok())
        .map(|s| steps_of(&s))
        .collect();
    let on_screen = targets_on(&screen);
    let declared: Vec<String> = found.screens.iter().flat_map(|s| s.test_ids.iter().cloned()).collect();
    let list = scenarios_in(&answer)
        .into_iter()
        .map(|text| {
            let mut suggestion =
                Suggestion { text: text.clone(), scenario: None, problem: None, report: None, written: None };
            let parsed = Scenario::parse(&text);
            let unknown =
                parsed.as_ref().ok().and_then(|scenario| unknown_target(scenario, &on_screen, &declared));
            match parsed {
                Err(e) => suggestion.problem = Some(e.to_string()),
                // Made up by the AI: discarded before it runs, with the reason.
                Ok(scenario) if unknown.is_some() => {
                    suggestion.problem = unknown;
                    suggestion.scenario = Some(scenario);
                }
                Ok(scenario) if had.contains(&steps_of(&scenario)) => {
                    suggestion.problem = Some("repete um cenário que o app já tem".to_owned());
                    suggestion.scenario = Some(scenario);
                }
                Ok(scenario) if scenario.app() != found.name => {
                    suggestion.problem = Some(format!("abre {:?}, não o {}", scenario.app(), found.name));
                    suggestion.scenario = Some(scenario);
                }
                Ok(scenario) => {
                    let graph = crate::sample::from_scenarios(std::slice::from_ref(&scenario));
                    suggestion.report = Some(Report::of(&scenario.name, graph, Run::new(crate::open)));
                    suggestion.scenario = Some(scenario);
                }
            }
            let copy = suggestion.problem.as_deref() == Some("repete um cenário que o app já tem");
            if write
                && !copy
                && let Some(scenario) = &suggestion.scenario
            {
                suggestion.written = keep(&folder, scenario, &suggestion, &who).ok();
            }
            suggestion
        })
        .collect();
    Ok(Suggestions { app: found.name.clone(), writer: who, list, duration_ms })
}

/// What a scenario does, without its name and titles.
fn steps_of(scenario: &Scenario) -> Vec<(ravenrs_e2e_wire::Kind, String, Option<String>)> {
    scenario.blocks.iter().map(|b| (b.kind, b.target.clone(), b.text.clone())).collect()
}

/// Writes a suggestion into `folder`, saying who wrote it and how it went: suggestions stay in a
/// folder of their own until a person moves them to `tests/e2e/`.
fn keep(folder: &Path, scenario: &Scenario, suggestion: &Suggestion, who: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let slug: String = scenario
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let file = folder.join(format!("{slug}.{}", ravenrs_e2e_wire::EXTENSION));
    let verdict = match (&suggestion.report, &suggestion.problem) {
        (Some(r), _) if r.passed() => "passou no app".to_owned(),
        (Some(r), _) => format!("falhou no app: {}", r.text().lines().nth(1).unwrap_or_default()),
        (None, Some(p)) => format!("não roda: {p}"),
        (None, None) => String::new(),
    };
    std::fs::write(
        &file,
        format!(
            "// Sugerido por {who}; {verdict}. Revise e mova para tests/e2e/ para manter.\n{}",
            scenario.write()
        ),
    )?;
    Ok(file)
}

impl Suggestions {
    /// For a tool or an AI.
    pub fn json(&self) -> Value {
        json!({
            "app": self.app,
            "writer": self.writer,
            "duration_ms": self.duration_ms,
            "passed": self.list.iter().filter(|s| s.passed()).count(),
            "suggestions": self.list.iter().map(|s| json!({
                "name": s.name(),
                "text": s.scenario.as_ref().map_or_else(|| s.text.clone(), Scenario::write),
                "passed": s.passed(),
                "problem": s.problem,
                "report": s.report.as_ref().map(Report::json),
                "written": s.written,
            })).collect::<Vec<_>>(),
        })
    }

    /// For a person.
    pub fn text(&self) -> String {
        let mut out = format!(
            "{}: {} cenários sugeridos por {} em {:.1} s, {} passaram no app\n",
            self.app,
            self.list.len(),
            self.writer,
            self.duration_ms as f64 / 1000.0,
            self.list.iter().filter(|s| s.passed()).count()
        );
        for s in &self.list {
            let name = s.name();
            let how = match (&s.report, &s.problem) {
                (Some(r), _) if r.passed() => "passou".to_owned(),
                (Some(r), _) => format!("falhou ({})", r.text().lines().nth(1).unwrap_or_default()),
                (None, Some(p)) => format!("não roda: {p}"),
                (None, None) => "não rodou".to_owned(),
            };
            out.push_str(&format!("\n— {name}: {how}\n"));
            if let Some(file) = &s.written {
                out.push_str(&format!("  gravado em {}\n", file.display()));
            }
            let shown = s.scenario.as_ref().map_or_else(|| s.text.clone(), Scenario::write);
            for line in shown.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out
    }
}
