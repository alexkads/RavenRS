//! A panel any desktop app can open to see its GPU and what its frames cost.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Bottleneck, DiagnosticFinding, DiagnosticReport, FrameLog, FrameSample, Percentiles, PerformanceResult,
    PerformanceVerdict, new_issue_url, redact_user_paths,
};
use ravenrs_reactive::State;

use crate::context::{BuildCx, NodeId, Placement};
use crate::floating::{SheetLayer, Side};
use crate::labels::Labels;
use crate::view::View;
use crate::{
    Alert, Button, ButtonStyle, CodeBlock, Grid, IconName, ProgressBar, ScrollView, Stat, Tone, VStack,
    ViewExt as _,
};

/// How often the open panel shows new numbers, in seconds. Its own rebuilds are
/// frames too, so this stays slow enough not to weigh on what it measures.
const REFRESH: f64 = 0.5;
/// How long a performance test redraws the window, in seconds.
const TEST_SECONDS: f64 = 5.0;
/// The panel's width in logical pixels.
const PANEL_WIDTH: f32 = 520.0;

type ReportSource = Rc<dyn Fn() -> DiagnosticReport>;
type OpenUrl = Rc<dyn Fn(&str) -> Result<(), String>>;

/// Where a bug report goes and how the platform gets it there.
#[derive(Clone)]
struct BugReporter {
    new_issue: String,
    open: OpenUrl,
}

/// Where a performance test stands.
#[derive(Clone, Copy, Debug, Default)]
enum TestPhase {
    #[default]
    Idle,
    /// Asked for; the clock starts at the next build.
    Starting,
    Running {
        started: f64,
    },
    Done(PerformanceResult),
}

/// What a platform shares with the panel of a [`DiagnosticsHost`]: it records
/// frames here, says how to describe the GPU, and opens the panel.
///
/// Frames are kept out of reactive state on purpose: recording one must never
/// ask for another.
#[derive(Clone)]
pub struct Diagnostics {
    open: State<bool>,
    frames: Rc<RefCell<FrameLog>>,
    source: Rc<RefCell<Option<ReportSource>>>,
    report: Rc<RefCell<Option<DiagnosticReport>>>,
    bug_reporter: Rc<RefCell<Option<BugReporter>>>,
    /// Changes without reactive state: builds move it along, and must not ask for
    /// another build by doing so. `test_runs` is what tells the UI about a new one.
    test: Rc<Cell<TestPhase>>,
    test_runs: State<u32>,
}

impl fmt::Debug for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diagnostics").field("open", &self.open.get()).finish_non_exhaustive()
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            open: State::new(false),
            frames: Rc::default(),
            source: Rc::default(),
            report: Rc::default(),
            bug_reporter: Rc::default(),
            test: Rc::default(),
            test_runs: State::new(0),
        }
    }
}

impl Diagnostics {
    /// Whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Opens or closes the panel.
    pub fn set_open(&self, open: bool) {
        self.open.set(open);
    }

    /// Opens the panel if it is closed and closes it if it is open.
    pub fn toggle(&self) {
        self.open.update(|open| *open = !*open);
    }

    /// Opens the panel and redraws the window nonstop for a few seconds, to tell
    /// the person in plain words how fast this machine draws this app.
    pub fn start_test(&self) {
        self.test.set(TestPhase::Starting);
        self.test_runs.update(|runs| *runs += 1);
        self.open.set(true);
    }

    /// Moves the test along to the time `now` and says where it stands.
    fn advance_test(&self, now: f64) -> TestPhase {
        let phase = match self.test.get() {
            TestPhase::Starting => {
                self.frames.borrow_mut().clear();
                TestPhase::Running { started: now }
            }
            TestPhase::Running { started } if now - started >= TEST_SECONDS => {
                TestPhase::Done(self.frames.borrow().measure(now - started))
            }
            phase => phase,
        };
        self.test.set(phase);
        phase
    }

    /// Adds a frame to the log.
    pub fn record(&self, sample: FrameSample) {
        self.frames.borrow_mut().record(sample);
    }

    /// Sets how to describe the GPU. Describing it can take a moment, so `source`
    /// only runs when the panel first needs it, and again after the next call here.
    pub fn set_report_source(&self, source: impl Fn() -> DiagnosticReport + 'static) {
        *self.source.borrow_mut() = Some(Rc::new(source));
        *self.report.borrow_mut() = None;
    }

    /// Adds a button to the panel that starts a bug report on the `new_issue` page.
    /// `open` shows a URL to the person, in a browser on the desktop, and says why
    /// when it cannot. Nothing is sent: the person reviews and submits on that page.
    pub fn set_bug_reporter(
        &self,
        new_issue: impl Into<String>,
        open: impl Fn(&str) -> Result<(), String> + 'static,
    ) {
        *self.bug_reporter.borrow_mut() =
            Some(BugReporter { new_issue: new_issue.into(), open: Rc::new(open) });
    }

    /// The report, gathering it if needed; `None` before a source is set.
    pub fn report(&self) -> Option<DiagnosticReport> {
        if self.report.borrow().is_none() {
            let source = self.source.borrow().clone()?;
            *self.report.borrow_mut() = Some(source());
        }
        self.report.borrow().clone()
    }
}

/// Shows `root` and, while [`Diagnostics`] says so, the diagnostics panel over
/// it. Closed, it adds nothing to the UI: no element and no wake-up.
pub struct DiagnosticsHost<V> {
    root: V,
    diagnostics: Diagnostics,
}

impl<V> fmt::Debug for DiagnosticsHost<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticsHost").field("diagnostics", &self.diagnostics).finish_non_exhaustive()
    }
}

impl<V: View> DiagnosticsHost<V> {
    /// Hosts `root` with the panel `diagnostics` controls.
    pub fn new(root: V, diagnostics: Diagnostics) -> Self {
        Self { root, diagnostics }
    }
}

impl<V: View> View for DiagnosticsHost<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let root = self.root.build(cx);
        if self.diagnostics.is_open() {
            self.diagnostics.test_runs.get();
            let open = self.diagnostics.open.binding();
            let close = open.clone();
            let labels = cx.labels();
            let panel = Panel { diagnostics: &self.diagnostics };
            let layer = SheetLayer {
                open: &open,
                title: &labels.diagnostics,
                description: None,
                content: Some(&panel),
                side: Side::Right,
                width: PANEL_WIDTH,
                bare: false,
            };
            cx.overlay(root, Placement::Fill, &layer, Some(Rc::new(move || close.set(false))));
        }
        root
    }
}

struct Panel<'a> {
    diagnostics: &'a Diagnostics,
}

/// What a bug report would share, and the button that starts one.
struct ReportBug {
    reporter: BugReporter,
    text: String,
}

impl View for ReportBug {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let spacing = cx.theme().spacing.s2;
        let failed = cx.use_state(|| false);
        let (reporter, text, failure) = (self.reporter.clone(), self.text.clone(), failed.clone());
        let on_click = {
            let labels = labels.clone();
            move || {
                let url = new_issue_url(
                    &reporter.new_issue,
                    &labels.diagnostics_report_questions,
                    &text,
                    &labels.diagnostics_report_truncated,
                );
                let opened = (reporter.open)(&url);
                if let Err(e) = &opened {
                    tracing::warn!(error = %e, "could not open the bug report");
                }
                failure.set(opened.is_err());
            }
        };
        let failure = failed.get().then(|| {
            Alert::new(
                labels.diagnostics_report_failed.clone(),
                labels.diagnostics_report_failed_detail.clone(),
            )
            .tone(Tone::Danger)
        });
        cx.child(
            &VStack::new((
                Alert::new(
                    labels.diagnostics_report_shares.clone(),
                    labels.diagnostics_report_shares_detail.clone(),
                ),
                failure,
                Button::new(labels.diagnostics_report_bug.clone())
                    .icon(IconName::ExternalLink)
                    .style(ButtonStyle::Secondary)
                    .on_click(on_click),
            ))
            .spacing(spacing),
        )
    }
}

fn finding_alert(labels: &Labels, finding: &DiagnosticFinding) -> Alert {
    let (title, message, tone) = match finding {
        DiagnosticFinding::SoftwareRendering => {
            (&labels.diagnostics_software, labels.diagnostics_software_detail.clone(), Tone::Danger)
        }
        DiagnosticFinding::UnoptimizedBuild => {
            (&labels.diagnostics_unoptimized, labels.diagnostics_unoptimized_detail.clone(), Tone::Warning)
        }
        DiagnosticFinding::HiddenAdapters(names) => {
            (&labels.diagnostics_hidden, names.join(", "), Tone::Warning)
        }
        DiagnosticFinding::NoGpu(reason) => (&labels.diagnostics_no_gpu, reason.clone(), Tone::Danger),
    };
    Alert::new(title.clone(), redact_user_paths(&message)).tone(tone)
}

fn result_alert(labels: &Labels, result: &PerformanceResult) -> Alert {
    let (title, tone) = match result.verdict {
        PerformanceVerdict::Good => (&labels.diagnostics_test_good, Tone::Success),
        PerformanceVerdict::Fair => (&labels.diagnostics_test_fair, Tone::Warning),
        PerformanceVerdict::Poor => (&labels.diagnostics_test_poor, Tone::Danger),
    };
    let cause = match result.bottleneck {
        None => &labels.diagnostics_test_cause_none,
        Some(Bottleneck::Gpu) => &labels.diagnostics_test_cause_gpu,
        Some(Bottleneck::Interface) => &labels.diagnostics_test_cause_interface,
    };
    Alert::new(title.clone(), format!("{:.0} {}. {cause}", result.fps, labels.diagnostics_test_fps))
        .tone(tone)
}

fn stage(name: &str, stage: Percentiles) -> Stat {
    Stat::new(format!("{name} · p95 {:.1?}", stage.p95), format!("{:.1?}", stage.median))
}

impl View for Panel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let now = cx.now();
        let phase = self.diagnostics.advance_test(now);
        // While testing, every frame asks for the next: that is the measurement.
        cx.wake_at(if matches!(phase, TestPhase::Running { .. }) { now } else { now + REFRESH });
        let labels = cx.labels();
        let spacing = cx.theme().spacing.s3;
        let report = self.diagnostics.report().unwrap_or_default();
        let summary = self.diagnostics.frames.borrow().summary();
        // The exact text a bug report would carry, so what is read here is what is shared.
        let measured = match phase {
            TestPhase::Done(result) => result.to_string(),
            _ => String::new(),
        };
        let text = redact_user_paths(&format!("{report}{measured}{summary}"));
        let reporter = self.diagnostics.bug_reporter.borrow().clone();
        let report_bug = reporter.map(|reporter| ReportBug { reporter, text: text.clone() });
        let alerts: Vec<_> = report.findings().iter().map(|f| finding_alert(&labels, f)).collect();
        let test: crate::AnyView = match phase {
            TestPhase::Running { started } => Box::new(
                VStack::new((
                    Alert::new(labels.diagnostics_testing.clone(), labels.diagnostics_testing_detail.clone()),
                    ProgressBar::new(((now - started) / TEST_SECONDS) as f32),
                ))
                .spacing(spacing),
            ),
            TestPhase::Starting | TestPhase::Idle | TestPhase::Done(_) => {
                let diagnostics = self.diagnostics.clone();
                let result = match phase {
                    TestPhase::Done(result) => Some(result_alert(&labels, &result)),
                    _ => None,
                };
                Box::new(
                    VStack::new((
                        result,
                        Button::new(labels.diagnostics_test.clone())
                            .icon(IconName::Activity)
                            .on_click(move || diagnostics.start_test()),
                    ))
                    .spacing(spacing),
                )
            }
        };
        cx.child(
            &ScrollView::new((
                test,
                VStack::new(alerts).spacing(spacing),
                Grid::new(
                    2,
                    (
                        stage(&labels.diagnostics_frame, summary.total),
                        stage(&labels.diagnostics_update, summary.update),
                        stage(&labels.diagnostics_acquire, summary.acquire),
                        stage(&labels.diagnostics_render, summary.render),
                    ),
                )
                .spacing(spacing),
                report_bug,
                CodeBlock::new(text),
            ))
            .grow(1.0),
        )
    }
}
