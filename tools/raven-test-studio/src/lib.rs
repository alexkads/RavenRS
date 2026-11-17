//! Raven Test Studio, first step (#634, part of #627).
//!
//! A scenario is a flow of blocks: open the app, type, click, check. The Studio runs it with
//! `ravenrs-e2e` against an app, marks each block as it goes (done, failed with the error, or
//! not run), and keeps a log of the run. Everything on screen is a RavenRS component:
//! `FlowLibrary` keeps the scenarios, `FlowToolbox` has the blocks, `FlowCanvas` shows the
//! scenario and `FlowInspector` edits the selected block.
//!
//! For now the app under test is a small form that comes with the Studio
//! ([`sample::expense_app`]) and runs in the same process. Opening the person's own app in
//! another process, scenario files and the live preview come in the next steps of #627.
//!
//! The Studio is tested by the tool it is made of: its tests drive it with `ravenrs-e2e`, as
//! `tests/suite/self_test.rs` shows.

pub mod apps;
pub mod atualizacao;
pub mod cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod delegate;
pub mod project;
pub mod report;
pub mod runner;
pub mod sample;
#[cfg(not(target_arch = "wasm32"))]
pub mod suggest;
pub mod tray;
pub mod versao;

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;
use ravenrs_app_update::{NoticeAction, Updater};

use crate::runner::{Outcome, Run};
pub use ravenrs_e2e_runner::run::RunOptions;

/// The Studio's version, the one its news speak of.
pub const VERSION: &str = versao::VERSION;

/// The Studio's icon, for the window and the icon beside the clock.
pub fn app_icon() -> Option<ImageData> {
    ravenrs::widgets::decode_image(include_bytes!("../assets/icon.png")).ok()
}

/// The kinds of block, with the word the canvas and the inspector show for each.
pub const KINDS: [(&str, &str); 12] = [
    ("open", "Início"),
    ("click", "Clicar"),
    ("type", "Digitar"),
    ("fill", "Preencher"),
    ("select", "Escolher"),
    ("expand", "Expandir"),
    ("collapse", "Recolher"),
    ("scroll", "Rolar até"),
    ("drag", "Arrastar"),
    ("wait", "Esperar"),
    ("check", "Conferir"),
    ("mention", "Conferir que menciona"),
];

fn icon(kind: &str) -> IconName {
    match kind {
        "open" => IconName::Play,
        "click" | "select" | "expand" | "collapse" | "scroll" | "drag" => IconName::MousePointer,
        "type" | "fill" => IconName::Keyboard,
        "wait" => IconName::Clock,
        _ => IconName::Check,
    }
}

/// Opens the app an Open block names: the sample form, another Studio with the sample
/// scenario, or another Studio whose scenario fails on purpose, compiled in; or an app opened
/// apart in a child process, such as Raven Finanças ([`apps`]). A Studio opened here runs its
/// own scenario the same way, so the Studio can test the Studio.
pub fn open(name: &str) -> Option<Result<runner::Session, String>> {
    let studio = |scenario: FlowGraph| {
        Ok(runner::Session::Here(Box::new(ravenrs_e2e::E2e::new(
            app_with(scenario, Rc::new(MemoryStorage::default())),
            Theme::raven_dark(),
        ))))
    };
    // An app of a project the Studio was pointed at comes first: pointed at its own folder, the
    // Studio tests another Studio in a process of its own, not the one compiled in.
    if apps::learned(name) {
        return apps::open(name);
    }
    match name {
        sample::STUDIO_APP => Some(studio(sample::scenario())),
        sample::BROKEN_STUDIO_APP => Some(studio(sample::broken_scenario())),
        other => runner::open_sample(other).or_else(|| apps::open(other)),
    }
}

/// The Studio's window: the run bar on top; the scenarios and the blocks on the left; the
/// scenario in the middle; the selected block and the log of the run on the right.
pub struct Studio {
    storage: Rc<dyn Storage>,
    scenario: FlowGraph,
    updater: Updater,
    /// How the Studio's own test went when the window opened; `None` for a Studio that did not
    /// test itself, as the one a scenario opens.
    health: Option<report::Report>,
    /// The menu beside the clock, for the installed window.
    tray: Option<tray::TrayLink>,
    /// The agent the window hands its scenario to, Codex by default.
    #[cfg(not(target_arch = "wasm32"))]
    agent: ravenrs_agent_cli::Cli,
}

impl Studio {
    /// The same window, handing its scenario to `agent` instead of Codex: Claude, or a fake one
    /// in tests.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn agent(mut self, agent: ravenrs_agent_cli::Cli) -> Self {
        self.agent = agent;
        self
    }

    /// The bar at the bottom: the version, how the Studio's own test went, and the updater's
    /// notice when there is one.
    fn status_bar(&self) -> StatusBar {
        let version = StatusBar::text(format!("Raven Test Studio {VERSION}"));
        let bar = match &self.health {
            Some(health) => {
                let (said, tone) = if health.passed() {
                    (
                        format!("Autoteste: {}", runner::summary(&health.outcomes, health.total)),
                        Semantic::Success,
                    )
                } else {
                    (
                        format!("Autoteste falhou: {}", runner::summary(&health.outcomes, health.total)),
                        Semantic::Danger,
                    )
                };
                StatusBar::new().left((version, StatusBar::text(said).foreground(tone).test_id("health")))
            }
            None => StatusBar::new().left((version,)),
        };
        let Some(notice) = self.updater.notice() else { return bar };
        let buttons: Vec<Button> = notice
            .actions
            .iter()
            .filter(|action| {
                !matches!(action, NoticeAction::News | NoticeAction::HowToUpdate | NoticeAction::Details)
            })
            .map(|&action| self.notice_button(action))
            .collect();
        let bar = bar.center((
            StatusBar::text(notice.text).foreground(Semantic::Ink).truncate().grow(1.0),
            HStack::new(buttons).spacing(4.0),
        ));
        if notice.important { bar.center_tone(Tone::Warning) } else { bar }
    }

    /// A button of the updater's notice, doing what Raven Finanças does with it.
    fn notice_button(&self, action: NoticeAction) -> Button {
        let updater = self.updater.clone();
        let button = StatusBar::action(action.label());
        let button = if action.is_primary() { button.style(ButtonStyle::Primary) } else { button };
        button.on_click(move || match action {
            NoticeAction::Update | NoticeAction::Retry => {
                if let Some(news) = updater.status().news().cloned() {
                    updater.install(news);
                }
            }
            NoticeAction::Reopen => updater.reopen(),
            NoticeAction::CheckAgain => updater.ask(),
            _ => updater.dismiss(),
        })
    }
}

impl std::fmt::Debug for Studio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Studio").finish_non_exhaustive()
    }
}

impl View for Studio {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The updater's threads wake the screen when they have something to say.
        let waker = cx.ui_waker();
        self.updater.connect(move || waker.wake());
        self.updater.poll();
        // What was chosen on the icon's menu, then the menu as the window now stands.
        if let Some(link) = &self.tray {
            for id in link.take() {
                if self.updater.tray_select(&id).is_some() {
                    continue;
                }
                #[cfg(not(target_arch = "wasm32"))]
                if id == tray::RUN_SELF_TEST {
                    let waker = cx.ui_waker();
                    link.run_self_test(move || waker.wake());
                }
            }
            link.menu().set(link.entries(self.health.as_ref(), self.updater.tray_entries()));
        }
        let start = self.scenario.clone();
        let graph = cx.use_state(move || start);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let view = cx.use_state(FlowViewport::fit);
        let run = cx.use_state(|| Rc::new(RefCell::new(Run::new(open))));
        let log = cx.use_state(Vec::<Outcome>::new);

        let mut canvas = FlowCanvas::new(graph.binding())
            .view(view.binding())
            .label("Cenário")
            .selection(selection.binding())
            .history(history.binding())
            .arrivals(arrivals.binding());
        let mut inspector =
            FlowInspector::new(graph.binding(), selection.binding()).history(history.binding());
        for (kind, caption) in KINDS {
            let tone = if kind == "check" { Semantic::Success } else { Semantic::Accent };
            canvas = canvas.kind(kind, FlowKind::new(icon(kind), caption).tone(tone));
            inspector = inspector.kind(kind, caption);
        }

        let memo = cx.use_state(|| None::<(String, Vec<Outcome>)>);
        let from_cache = cx.use_state(|| false);
        let runs = Runs {
            graph: graph.clone(),
            run: run.clone(),
            log: log.clone(),
            memo,
            from_cache: from_cache.clone(),
        };
        let finished = run.get().borrow().finished(&graph.get());
        let ran = !log.get().is_empty();
        #[cfg(not(target_arch = "wasm32"))]
        let handing = cx.use_state(Handing::shared);
        #[cfg(not(target_arch = "wasm32"))]
        let turn = cx.use_state(|| 0_u32);
        // What the agent said comes from another thread; the Studio's own report of the same
        // scenario marks the blocks, never the agent's words.
        #[cfg(not(target_arch = "wasm32"))]
        let delegation = {
            let shared = handing.get();
            let mut h = lock(&shared);
            let fresh = match &h.done {
                Some(Ok(done)) if !h.shown => Some(done.checked.outcomes.clone()),
                _ => None,
            };
            if let Some(outcomes) = fresh {
                h.shown = true;
                graph.set(runner::applied(&graph.get(), &outcomes));
                log.set(outcomes);
                from_cache.set(false);
            }
            DelegationView {
                running: h.running.clone(),
                card: h.done.as_ref().map(|r| match r {
                    Ok(d) => (d.card(), d.confirmed()),
                    Err(why) => (format!("A delegação não terminou: {why}"), false),
                }),
                hint: (!self.agent.installed()).then(|| {
                    format!(
                        "O {} não está instalado nesta máquina: instale o CLI e entre nele para delegar",
                        self.agent.agent().program()
                    )
                }),
            }
        };
        #[cfg(target_arch = "wasm32")]
        let delegation = DelegationView::default();
        let busy = delegation.running.is_some();
        let bar = HStack::new((
            Text::new("Raven Test Studio").style(TextStyle::Headline),
            Spacer::new(),
            run_button(&runs, true),
            run_button(&runs, false),
            next_button(&graph, &run, &log, finished),
            #[cfg(not(target_arch = "wasm32"))]
            delegate_button(&graph, &handing, &turn, &self.agent, busy, cx.ui_waker()),
            stop_button(&runs, ran, busy, {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    Some(handing.get())
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None::<()>
                }
            }),
        ))
        .spacing(8.0)
        .padding(8.0);

        cx.child(
            &VStack::new((
                bar,
                HStack::new((
                    VStack::new((
                        FlowLibrary::new(graph.binding())
                            .title("Cenários")
                            .key("scenarios")
                            .history(history.binding())
                            .selection(selection.binding())
                            .view(view.binding())
                            .storage(self.storage.clone()),
                        sample::toolbox().title("Blocos").arrivals(arrivals.binding()),
                    ))
                    .spacing(0.0)
                    .align(Align::Stretch),
                    canvas,
                    VStack::new((
                        inspector,
                        run_log(&log.get(), runner::path(&graph.get()).len(), from_cache.get(), &delegation),
                    ))
                    .spacing(0.0)
                    .align(Align::Stretch),
                ))
                .align(Align::Stretch)
                .grow(1.0),
                self.status_bar(),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

type RunState = State<Rc<RefCell<Run>>>;

/// The last full run of this window, by the key of the scenario it ran.
type Memo = State<Option<(String, Vec<Outcome>)>>;

/// What the run buttons share.
#[derive(Clone)]
struct Runs {
    graph: State<FlowGraph>,
    run: RunState,
    log: State<Vec<Outcome>>,
    memo: Memo,
    from_cache: State<bool>,
}

/// Runs the whole scenario from the start, stopping at the first block that fails. With
/// `use_cache`, a scenario that did not change since the last full run comes back from it.
fn run_button(runs: &Runs, use_cache: bool) -> impl View {
    let runs = runs.clone();
    let (label, id, style) = if use_cache {
        ("Rodar", "run", ButtonStyle::Primary)
    } else {
        ("Rodar sem cache", "run-fresh", ButtonStyle::Secondary)
    };
    Button::new(label)
        .icon(IconName::Play)
        .style(style)
        .on_click(move || {
            let graph = runs.graph.get();
            let key = cache::scenario_key(&graph);
            if use_cache && let Some((_, outcomes)) = runs.memo.get().filter(|(k, _)| *k == key) {
                runs.graph.set(runner::applied(&graph, &outcomes));
                runs.run.set(Rc::new(RefCell::new(Run::finished_with(open))));
                runs.log.set(outcomes);
                runs.from_cache.set(true);
                return;
            }
            let mut g = runner::cleared(&graph);
            let mut fresh = Run::new(open);
            let outcomes = fresh.to_end(&mut g);
            runs.run.set(Rc::new(RefCell::new(fresh)));
            runs.graph.set(g);
            runs.memo.set(Some((key, outcomes.clone())));
            runs.log.set(outcomes);
            runs.from_cache.set(false);
        })
        .test_id(id)
}

/// Runs one more block, starting a run when none is going.
fn next_button(
    graph: &State<FlowGraph>,
    run: &RunState,
    log: &State<Vec<Outcome>>,
    finished: bool,
) -> impl View {
    let (graph, run, log) = (graph.clone(), run.clone(), log.clone());
    Button::new("Próximo passo")
        .style(ButtonStyle::Secondary)
        .disabled(finished)
        .on_click(move || {
            let mut g = graph.get();
            let outcome = run.get().borrow_mut().step(&mut g);
            if let Some(outcome) = outcome {
                graph.set(g);
                let mut all = log.get();
                all.push(outcome);
                log.set(all);
            }
        })
        .test_id("next")
}

/// Puts every block back and forgets the run; the cache keeps the last full run. While an
/// agent has the scenario, it ends the agent's CLI instead.
#[cfg(not(target_arch = "wasm32"))]
fn stop_button(runs: &Runs, ran: bool, busy: bool, handing: Option<Shared>) -> impl View {
    let runs = runs.clone();
    Button::new("Parar")
        .style(ButtonStyle::Ghost)
        .disabled(!ran && !busy)
        .on_click(move || {
            if let Some(handing) = handing.as_ref().filter(|_| busy) {
                lock(handing).stop.store(true, std::sync::atomic::Ordering::SeqCst);
                return;
            }
            runs.graph.set(runner::cleared(&runs.graph.get()));
            runs.run.set(Rc::new(RefCell::new(Run::new(open))));
            runs.log.set(Vec::new());
            runs.from_cache.set(false);
        })
        .test_id("stop")
}

/// What the log shows of a delegation: that an agent has the scenario, its card once it
/// answered, and why the button is off when the agent is not installed.
#[derive(Clone, Debug, Default)]
struct DelegationView {
    running: Option<String>,
    card: Option<(String, bool)>,
    hint: Option<String>,
}

/// A delegation of this window, shared with the thread that runs it.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Handing {
    /// "Delegado ao Codex (modelo)…" while the agent works.
    running: Option<String>,
    /// Set by "Parar"; the agent's CLI is ended at its next event.
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// What came back.
    done: Option<Result<delegate::Delegation, String>>,
    /// Whether the window already marked its blocks with the report that came back.
    shown: bool,
}

#[cfg(not(target_arch = "wasm32"))]
type Shared = std::sync::Arc<std::sync::Mutex<Handing>>;

#[cfg(not(target_arch = "wasm32"))]
impl Handing {
    fn shared() -> Shared {
        Shared::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Handing> {
    shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Hands the scenario on the canvas to the agent, in a thread of its own, so the window keeps
/// answering while the agent works; the thread wakes the window when the answer is in.
#[cfg(not(target_arch = "wasm32"))]
fn delegate_button(
    graph: &State<FlowGraph>,
    handing: &State<Shared>,
    turn: &State<u32>,
    agent: &ravenrs_agent_cli::Cli,
    busy: bool,
    waker: UiWaker,
) -> impl View {
    let name = match agent.agent() {
        ravenrs_agent_cli::Agent::Codex => "Codex",
        ravenrs_agent_cli::Agent::Claude => "Claude",
    };
    let has_open = graph.get().nodes.iter().any(|n| n.kind == "open");
    let (graph, handing, turn, agent) = (graph.clone(), handing.get(), turn.clone(), agent.clone());
    let installed = agent.installed();
    Button::new(format!("Delegar ao {name}"))
        .icon(IconName::Sparkles)
        .style(ButtonStyle::Secondary)
        .disabled(busy || !installed || !has_open)
        .on_click(move || {
            let mut h = lock(&handing);
            h.shown = false;
            let written = sample::scenario_of(&graph.get(), "Cenário da janela").and_then(|scenario| {
                let file = std::env::temp_dir()
                    .join(format!("raven-test-studio-delegado-{}.raventest", std::process::id()));
                std::fs::write(&file, scenario.write()).map(|()| file).map_err(|e| e.to_string())
            });
            let file = match written {
                Ok(file) => file,
                Err(why) => {
                    h.done = Some(Err(why));
                    turn.update(|t| *t += 1);
                    return;
                }
            };
            let model = ravenrs_agent_cli::cheapest_model(agent.agent());
            h.running =
                Some(format!("Delegado ao {name} ({})…", model.as_deref().unwrap_or("modelo padrão")));
            h.done = None;
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            h.stop = stop.clone();
            drop(h);
            let (shared, agent, waker) = (handing.clone(), agent.clone(), waker.clone());
            std::thread::spawn(move || {
                let asked = file.to_string_lossy().into_owned();
                let result = delegate::delegate_until(&agent, &asked, model, &|| {
                    stop.load(std::sync::atomic::Ordering::SeqCst)
                });
                let _ = std::fs::remove_file(&file);
                let mut h = lock(&shared);
                h.running = None;
                h.done = Some(result);
                drop(h);
                waker.wake();
            });
            turn.update(|t| *t += 1);
        })
        .test_id("delegate")
}

/// The log of the run: a line that sums it up, whether it came from the cache, the agent's
/// card when the scenario was delegated, then one line per block that ran.
fn run_log(outcomes: &[Outcome], total: usize, from_cache: bool, delegation: &DelegationView) -> impl View {
    let note = if from_cache {
        "Do cache: o cenário não mudou desde a última vez que rodou nesta janela"
    } else {
        ""
    };
    let head =
        if outcomes.is_empty() { "Ainda não rodou".to_owned() } else { runner::summary(outcomes, total) };
    let lines: Vec<_> = outcomes
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let line = match &o.error {
                None => format!("{}. {}: passou", i + 1, o.title),
                Some(e) => format!("{}. {}: {e}", i + 1, o.title),
            };
            Text::new(line).style(TextStyle::Callout).test_id(format!("log-{}", i + 1))
        })
        .collect();
    VStack::new((
        Text::new("Registro").style(TextStyle::Caption),
        Text::new(head).style(TextStyle::Headline).test_id("summary"),
        Text::new(note).style(TextStyle::Caption).test_id("cache-note"),
        Text::new(delegation.hint.clone().unwrap_or_default())
            .style(TextStyle::Caption)
            .test_id("delegate-hint"),
        Text::new(match (&delegation.running, &delegation.card) {
            (Some(running), _) => running.clone(),
            (None, Some((card, _))) => card.clone(),
            (None, None) => String::new(),
        })
        .style(TextStyle::Callout)
        .foreground(match &delegation.card {
            Some((_, false)) if delegation.running.is_none() => Semantic::Danger,
            _ => Semantic::Ink,
        })
        .test_id("delegation"),
        VStack::new(lines).spacing(4.0),
    ))
    .spacing(8.0)
    .padding(16.0)
}

fn studio(
    storage: Rc<dyn Storage>,
    scenario: FlowGraph,
    updater: Updater,
    health: Option<report::Report>,
) -> Studio {
    Studio {
        storage,
        scenario,
        updater,
        health,
        tray: None,
        #[cfg(not(target_arch = "wasm32"))]
        agent: ravenrs_agent_cli::Cli::new(ravenrs_agent_cli::Agent::Codex),
    }
}

/// What `--run` takes by name. A Studio installed from scratch brings no test scenarios: only
/// its own health check, "O Studio testa o Studio", and `financas`, which runs the scenarios
/// Raven Finanças brings in its own `tests/e2e/`. Any other scenario is a `.raventest` file,
/// kept in the app or in a QA team's folder, and run by path ([`run_named`]).
pub const SCENARIOS: [(&str, &str); 2] =
    [("autoteste", "O Studio testa o Studio"), ("financas", "Os cenários que o Raven Finanças traz")];

/// Runs a scenario that comes with the Studio, with no window, and reports it; `None` when
/// there is no scenario by that name. With a `cache`, a scenario that did not change on a
/// build that did not change comes back from it instead of running again.
pub fn run_bundled(name: &str, cache: Option<&mut cache::RunCache>) -> Option<report::Report> {
    run_bundled_with(name, cache, &RunOptions::default())
}

/// [`run_bundled`] as `options` say: filming, and where to keep the films. A run that films
/// runs again, since the cache keeps no film.
pub fn run_bundled_with(
    name: &str,
    cache: Option<&mut cache::RunCache>,
    options: &RunOptions,
) -> Option<report::Report> {
    let cache = if options.film == runner::FilmMode::Never { cache } else { None };
    let (key, title) = SCENARIOS.iter().find(|(key, _)| *key == name)?;
    let graph = match *key {
        "autoteste" => sample::self_test(),
        "financas" => match apps::scenarios(apps::FINANCE_APP) {
            Some(Ok(scenarios)) => sample::from_scenarios(&scenarios),
            Some(Err(why)) => return Some(report::Report::refused(title, apps::FINANCE_APP, &why)),
            None => {
                return Some(report::Report::refused(
                    title,
                    apps::FINANCE_APP,
                    "o Studio não conhece o Raven Finanças",
                ));
            }
        },
        _ => return None,
    };
    // An app opened apart is built apart: the Studio's build says nothing about it, so its
    // scenario always runs.
    let kept = if sample::opens_apart(&graph) {
        None
    } else {
        cache::RunCache::key(&graph, cache::build_key().as_deref())
    };
    if let (Some(c), Some(k)) = (cache.as_deref(), kept.as_deref())
        && let Some(report) = c.get(k)
    {
        return Some(report);
    }
    let report = report::of(title, graph, Run::new(open).filming(options.film, options.folder_of(title)));
    if let (Some(c), Some(k)) = (cache, kept.as_deref()) {
        // A cache that cannot be written only means the next run runs again.
        let _ = c.put(k, &report);
    }
    Some(report)
}

/// Runs what `name` says: a name of [`SCENARIOS`], or the path of a `.raventest` file or of a
/// folder of them. `Err` says why nothing ran.
pub fn run_named(name: &str, cache: Option<&mut cache::RunCache>) -> Result<report::Report, String> {
    run_named_with(name, cache, &RunOptions::default())
}

/// [`run_named`] as `options` say.
pub fn run_named_with(
    name: &str,
    cache: Option<&mut cache::RunCache>,
    options: &RunOptions,
) -> Result<report::Report, String> {
    if let Some(report) = run_bundled_with(name, cache, options) {
        return Ok(report);
    }
    let path = std::path::Path::new(name);
    if path.exists() {
        return run_files_with(path, options);
    }
    let known: Vec<&str> = SCENARIOS.iter().map(|(key, _)| *key).collect();
    Err(format!(
        "não há cenário {name:?}; por nome: {}, ou o caminho de um arquivo .raventest ou de uma pasta deles",
        known.join(", ")
    ))
}

/// Runs `.raventest` files with no window and reports them: one file, or every file of a folder
/// (the QA team's own, or an app's `tests/e2e/`), one after the other.
pub fn run_files(path: &std::path::Path) -> Result<report::Report, String> {
    run_files_with(path, &RunOptions::default())
}

/// [`run_files`] as `options` say. The files are read as `cargo ravenrs e2e` reads them, and run
/// on the Studio's diagram, which opens the apps compiled into the Studio too.
pub fn run_files_with(path: &std::path::Path, options: &RunOptions) -> Result<report::Report, String> {
    use ravenrs_e2e_runner::run::{files_at, read, title_of};
    let scenarios = read(&files_at(path)?)?;
    let title = title_of(path, &scenarios);
    let run = Run::new(open).filming(options.film, options.folder_of(&title));
    Ok(report::of(&title, sample::from_scenarios(&scenarios), run))
}

/// The Studio opening with its own test, "O Studio testa o Studio", keeping its scenarios in
/// memory. It does not look for updates: that is the installed window's, [`installed_app`].
pub fn app() -> Localized<Studio> {
    app_with(sample::self_test(), Rc::new(MemoryStorage::default()))
}

/// The Studio opening `scenario`, keeping its scenarios in `storage`, not looking for updates.
pub fn app_with(scenario: FlowGraph, storage: Rc<dyn Storage>) -> Localized<Studio> {
    Localized::new(Labels::pt_br(), studio(storage, scenario, versao::idle(), None))
}

/// The Studio opening `scenario`, handing it to `agent` when "Delegar" is pressed: Claude
/// instead of Codex, or a fake agent in tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn app_delegating(scenario: FlowGraph, agent: ravenrs_agent_cli::Cli) -> Localized<Studio> {
    Localized::new(
        Labels::pt_br(),
        studio(Rc::new(MemoryStorage::default()), scenario, versao::idle(), None).agent(agent),
    )
}

/// The Studio after testing itself, with no window: it runs "O Studio testa o Studio" before it
/// opens (from `cache` when neither the scenario nor the build changed) and says how it went in
/// its status bar, so a Studio that no longer works says so before anyone trusts it.
pub fn self_checked_app(cache: Option<&mut cache::RunCache>) -> Localized<Studio> {
    checked(versao::idle(), cache)
}

fn checked(updater: Updater, cache: Option<&mut cache::RunCache>) -> Localized<Studio> {
    let health = run_bundled("autoteste", cache);
    let storage: Rc<dyn Storage> = Rc::new(MemoryStorage::default());
    // The window opens with no scenario: a Studio installed from scratch brings none. Its own
    // test ran above and shows in the status bar.
    Localized::new(Labels::pt_br(), studio(storage, FlowGraph::new(), updater, health))
}

/// The Studio another Studio drives over the test channel (`--e2e-serve`): the sample scenario
/// to run, step and stop, and its own health check in the status bar, as its window shows it.
/// It brings no scenario of its own and does not look for updates.
pub fn served_app() -> Localized<Studio> {
    let health = run_bundled("autoteste", None);
    let storage: Rc<dyn Storage> = Rc::new(MemoryStorage::default());
    Localized::new(Labels::pt_br(), studio(storage, sample::scenario(), versao::idle(), health))
}

/// The Studio as its window opens: it tests itself, then looks for a new version when it opens
/// and every hour after, as Raven Finanças does.
pub fn installed_app() -> Localized<Studio> {
    installed_app_in_tray(None)
}

/// [`installed_app`], keeping `link`'s menu beside the clock as the window's state changes and
/// acting on what is chosen there.
pub fn installed_app_in_tray(link: Option<tray::TrayLink>) -> Localized<Studio> {
    let updater = versao::idle();
    updater.watch();
    let mut kept = cache::default_file().map(|file| cache::RunCache::at(&file));
    let health = run_bundled("autoteste", kept.as_mut());
    let storage: Rc<dyn Storage> = Rc::new(MemoryStorage::default());
    let mut studio = studio(storage, FlowGraph::new(), updater, health);
    studio.tray = link;
    Localized::new(Labels::pt_br(), studio)
}

/// The Studio as [`installed_app_in_tray`] opens it, with an updater that does nothing until
/// told, for tests of the menu beside the clock.
pub fn app_in_tray(link: tray::TrayLink, updater: Updater) -> Localized<Studio> {
    let mut studio = studio(Rc::new(MemoryStorage::default()), sample::scenario(), updater, None);
    studio.tray = Some(link);
    Localized::new(Labels::pt_br(), studio)
}
