//! A panel any desktop app can open to see its GPU and what its frames cost.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{DiagnosticFinding, DiagnosticReport, FrameLog, FrameSample, Percentiles};
use ravenrs_reactive::State;

use crate::context::{BuildCx, NodeId, Placement};
use crate::floating::{SheetLayer, Side};
use crate::labels::Labels;
use crate::view::View;
use crate::{Alert, CodeBlock, Grid, ScrollView, Stat, Tone, VStack, ViewExt as _};

/// How often the open panel shows new numbers, in seconds. Its own rebuilds are
/// frames too, so this stays slow enough not to weigh on what it measures.
const REFRESH: f64 = 0.5;
/// The panel's width in logical pixels.
const PANEL_WIDTH: f32 = 520.0;

type ReportSource = Rc<dyn Fn() -> DiagnosticReport>;

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
}

impl fmt::Debug for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diagnostics").field("open", &self.open.get()).finish_non_exhaustive()
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self { open: State::new(false), frames: Rc::default(), source: Rc::default(), report: Rc::default() }
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
    Alert::new(title.clone(), message).tone(tone)
}

fn stage(name: &str, stage: Percentiles) -> Stat {
    Stat::new(format!("{name} · p95 {:.1?}", stage.p95), format!("{:.1?}", stage.median))
}

impl View for Panel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.wake_at(cx.now() + REFRESH);
        let labels = cx.labels();
        let spacing = cx.theme().spacing.s3;
        let report = self.diagnostics.report().unwrap_or_default();
        let summary = self.diagnostics.frames.borrow().summary();
        let alerts: Vec<_> = report.findings().iter().map(|f| finding_alert(&labels, f)).collect();
        cx.child(
            &ScrollView::new((
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
                CodeBlock::new(format!("{report}{summary}")),
            ))
            .grow(1.0),
        )
    }
}
