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

pub mod cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod delegate;
pub mod report;
pub mod runner;
pub mod sample;
pub mod versao;

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;
use ravenrs_app_update::{NoticeAction, Updater};

use crate::runner::{Outcome, Run};

/// The Studio's version, the one its news speak of.
pub const VERSION: &str = versao::VERSION;

/// The Studio's icon, for the window and the icon beside the clock.
pub fn app_icon() -> Option<ImageData> {
    ravenrs::widgets::decode_image(include_bytes!("../assets/icon.png")).ok()
}

/// The kinds of block, with the word the canvas and the inspector show for each.
pub const KINDS: [(&str, &str); 5] = [
    ("open", "Início"),
    ("click", "Clicar"),
    ("type", "Digitar"),
    ("check", "Conferir"),
    ("mention", "Conferir que menciona"),
];

fn icon(kind: &str) -> IconName {
    match kind {
        "open" => IconName::Play,
        "click" => IconName::MousePointer,
        "type" => IconName::Keyboard,
        _ => IconName::Check,
    }
}

/// Opens the app an Open block names: the sample form, another Studio with the sample
/// scenario, or another Studio whose scenario fails on purpose. A Studio opened here runs its
/// own scenario the same way, so the Studio can test the Studio.
pub fn open(name: &str) -> Option<ravenrs_e2e::E2e> {
    let studio = |scenario: FlowGraph| {
        ravenrs_e2e::E2e::new(app_with(scenario, Rc::new(MemoryStorage::default())), Theme::raven_dark())
    };
    match name {
        sample::STUDIO_APP => Some(studio(sample::scenario())),
        sample::BROKEN_STUDIO_APP => Some(studio(sample::broken_scenario())),
        other => runner::open_sample(other),
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
}

impl Studio {
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
        let bar = HStack::new((
            Text::new("Raven Test Studio").style(TextStyle::Headline),
            Spacer::new(),
            run_button(&runs, true),
            run_button(&runs, false),
            next_button(&graph, &run, &log, finished),
            stop_button(&runs, ran),
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
                        run_log(&log.get(), runner::path(&graph.get()).len(), from_cache.get()),
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

/// Puts every block back and forgets the run; the cache keeps the last full run.
fn stop_button(runs: &Runs, ran: bool) -> impl View {
    let runs = runs.clone();
    Button::new("Parar")
        .style(ButtonStyle::Ghost)
        .disabled(!ran)
        .on_click(move || {
            runs.graph.set(runner::cleared(&runs.graph.get()));
            runs.run.set(Rc::new(RefCell::new(Run::new(open))));
            runs.log.set(Vec::new());
            runs.from_cache.set(false);
        })
        .test_id("stop")
}

/// The log of the run: a line that sums it up, whether it came from the cache, then one line
/// per block that ran.
fn run_log(outcomes: &[Outcome], total: usize, from_cache: bool) -> impl View {
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
        VStack::new(lines).spacing(4.0),
    ))
    .spacing(8.0)
    .padding(16.0)
}

/// The scenarios that come with the Studio, by the name `--run` takes: the Studio's own test,
/// the expense form, and a scenario that fails on purpose.
pub const SCENARIOS: [(&str, &str); 3] = [
    ("autoteste", "O Studio testa o Studio"),
    ("lancamento", "Lançar despesa"),
    ("quebrado", "Cenário quebrado"),
];

/// Runs a scenario that comes with the Studio, with no window, and reports it; `None` when
/// there is no scenario by that name. With a `cache`, a scenario that did not change on a
/// build that did not change comes back from it instead of running again.
pub fn run_bundled(name: &str, cache: Option<&mut cache::RunCache>) -> Option<report::Report> {
    let (key, title) = SCENARIOS.iter().find(|(key, _)| *key == name)?;
    let graph = match *key {
        "autoteste" => sample::self_test(),
        "lancamento" => sample::scenario(),
        _ => sample::broken_scenario(),
    };
    let kept = cache::RunCache::key(&graph, cache::build_key().as_deref());
    if let (Some(c), Some(k)) = (cache.as_deref(), kept.as_deref())
        && let Some(report) = c.get(k)
    {
        return Some(report);
    }
    let report = report::Report::of(title, graph, Run::new(open));
    if let (Some(c), Some(k)) = (cache, kept.as_deref()) {
        // A cache that cannot be written only means the next run runs again.
        let _ = c.put(k, &report);
    }
    Some(report)
}

/// The Studio opening with its own test, "O Studio testa o Studio", keeping its scenarios in
/// memory. It does not look for updates: that is the installed window's, [`installed_app`].
pub fn app() -> Localized<Studio> {
    app_with(sample::self_test(), Rc::new(MemoryStorage::default()))
}

/// The Studio opening `scenario`, keeping its scenarios in `storage`, not looking for updates.
pub fn app_with(scenario: FlowGraph, storage: Rc<dyn Storage>) -> Localized<Studio> {
    Localized::new(Labels::pt_br(), Studio { storage, scenario, updater: versao::idle(), health: None })
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
    Localized::new(Labels::pt_br(), Studio { storage, scenario: sample::self_test(), updater, health })
}

/// The Studio as its window opens: it tests itself, then looks for a new version when it opens
/// and every hour after, as Raven Finanças does.
pub fn installed_app() -> Localized<Studio> {
    let updater = versao::idle();
    updater.watch();
    let mut kept = cache::default_file().map(|file| cache::RunCache::at(&file));
    checked(updater, kept.as_mut())
}
