//! `DiagnosticsHost`: nothing while closed; opened, it shows the report and keeps
//! its numbers fresh without asking for frames of its own.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ravenrs_core::{AccessTree, DiagnosticReport, FrameSample, GpuReport, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn running(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 700.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn names(tree: &AccessTree) -> Vec<String> {
    tree.nodes.iter().filter_map(|n| n.name.clone()).collect()
}

#[test]
fn closed_it_adds_nothing_and_never_wakes() {
    let plain = running(Text::new("Page"));
    let hosted = running(DiagnosticsHost::new(Text::new("Page"), Diagnostics::default()));
    assert_eq!(hosted.element_count(), plain.element_count());
    assert_eq!(hosted.next_wake(), None);
    assert!(!hosted.needs_redraw());
}

#[test]
fn opening_shows_the_panel_and_refreshes_slowly() {
    let diagnostics = Diagnostics::default();
    let mut ui = running(DiagnosticsHost::new(Text::new("Page"), diagnostics.clone()));
    diagnostics.set_open(true);
    assert!(ui.needs_redraw());
    ui.update();
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::Dialog && n.name.as_deref() == Some("Diagnostics")));
    assert_eq!(ui.next_wake(), Some(0.5));

    diagnostics.set_open(false);
    ui.update();
    assert!(!names(&ui.access_tree()).iter().any(|n| n == "Diagnostics"));
}

#[test]
fn recording_a_frame_asks_for_no_other() {
    let diagnostics = Diagnostics::default();
    diagnostics.set_open(true);
    let mut ui = running(DiagnosticsHost::new(Text::new("Page"), diagnostics.clone()));
    ui.set_time(0.1);
    ui.update();
    diagnostics.record(FrameSample { update: Duration::from_millis(4), ..Default::default() });
    assert!(!ui.needs_redraw(), "the log is not reactive state");
}

#[test]
fn the_report_is_gathered_once_and_only_when_needed() {
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let diagnostics = Diagnostics::default();
    diagnostics.set_report_source(move || {
        counted.set(counted.get() + 1);
        DiagnosticReport { gpu: Ok(GpuReport::default()), ..Default::default() }
    });
    let mut ui = running(DiagnosticsHost::new(Text::new("Page"), diagnostics.clone()));
    assert_eq!(calls.get(), 0, "closed, the panel costs nothing");
    diagnostics.set_open(true);
    ui.update();
    ui.set_time(1.0);
    ui.update();
    assert_eq!(calls.get(), 1);
    diagnostics.set_report_source(DiagnosticReport::default);
    assert!(diagnostics.report().is_some());
}

#[test]
fn a_software_gpu_is_called_out() {
    let diagnostics = Diagnostics::default();
    diagnostics.set_report_source(|| {
        let mut gpu = GpuReport::default();
        gpu.active.software = true;
        DiagnosticReport { gpu: Ok(gpu), ..Default::default() }
    });
    diagnostics.set_open(true);
    let mut ui = running(Localized::new(
        Labels::pt_br(),
        DiagnosticsHost::new(Text::new("Page"), diagnostics.clone()),
    ));
    ui.update();
    let names = names(&ui.access_tree());
    assert!(names.iter().any(|n| n.contains("Desenhando na CPU")), "{names:?}");
}
