//! `DiagnosticsHost`: nothing while closed; opened, it shows the report and keeps
//! its numbers fresh without asking for frames of its own.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use ravenrs_core::{
    AccessTree, DiagnosticReport, Event, FrameSample, GpuReport, MAX_ISSUE_URL, Point, PointerButton, Role,
    Size, SystemReport, new_issue_url, redact_user_paths,
};
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

const NEW_ISSUE: &str = "https://github.com/alexkads/RavenRS/issues/new";

fn click_button(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let at: Point = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .map(|n| n.bounds.center())
        .expect("the button");
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

/// A panel opened on a report with a personal path in it, and the URLs it asked to open.
fn reporting(outcome: Result<(), String>) -> (Ui, Rc<RefCell<Vec<String>>>) {
    let opened = Rc::new(RefCell::new(Vec::new()));
    let seen = opened.clone();
    let diagnostics = Diagnostics::default();
    diagnostics.set_report_source(|| DiagnosticReport {
        gpu: Err("no driver at /home/ana/icd/radeon.json".into()),
        system: SystemReport { os: "Fedora Linux 44".into(), ..Default::default() },
    });
    diagnostics.set_bug_reporter(NEW_ISSUE, move |url| {
        seen.borrow_mut().push(url.to_owned());
        outcome.clone()
    });
    diagnostics.set_open(true);
    (running(DiagnosticsHost::new(Text::new("Page"), diagnostics)), opened)
}

/// The body of a URL `new_issue_url` built, failing on anything badly encoded.
fn body_of(url: &str) -> String {
    let (_, encoded) = url.split_once("body=").expect("a body");
    let mut bytes = Vec::new();
    let mut rest = encoded.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        rest = tail;
        if byte == b'%' {
            let (hex, tail) = rest.split_at(2);
            let hex = std::str::from_utf8(hex).expect("ascii");
            bytes.push(u8::from_str_radix(hex, 16).expect("two hex digits"));
            rest = tail;
        } else {
            assert!(byte.is_ascii_alphanumeric() || b"-._~".contains(&byte), "raw {byte}");
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).expect("whole characters")
}

#[test]
fn without_a_url_there_is_no_button() {
    let diagnostics = Diagnostics::default();
    diagnostics.set_open(true);
    let mut ui = running(DiagnosticsHost::new(Text::new("Page"), diagnostics));
    ui.update();
    let names = names(&ui.access_tree());
    assert!(!names.iter().any(|n| n == "Report bug"), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("What a bug report shares")), "{names:?}");
}

#[test]
fn the_panel_says_what_is_shared_and_opens_nothing_until_asked() {
    let (mut ui, opened) = reporting(Ok(()));
    ui.update();
    let names = names(&ui.access_tree());
    assert!(names.iter().any(|n| n.contains("What a bug report shares")), "{names:?}");
    assert!(names.iter().any(|n| n.contains("Nothing is sent from here")), "{names:?}");
    assert!(opened.borrow().is_empty());

    click_button(&mut ui, "Report bug");
    let opened = opened.borrow();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].starts_with(&format!("{NEW_ISSUE}?body=")), "{}", opened[0]);
    let body = body_of(&opened[0]);
    assert!(body.contains("**Steps to reproduce**"), "{body}");
    assert!(body.contains("System: Fedora Linux 44"), "{body}");
    assert!(body.contains("Frames: 0"), "{body}");
}

#[test]
fn a_personal_path_is_neither_shown_nor_shared() {
    let (mut ui, opened) = reporting(Ok(()));
    ui.update();
    let names = names(&ui.access_tree());
    assert!(!names.iter().any(|n| n.contains("ana")), "{names:?}");
    assert!(names.iter().any(|n| n.contains("<user folder>")), "{names:?}");

    click_button(&mut ui, "Report bug");
    let body = body_of(&opened.borrow()[0]);
    assert!(!body.contains("ana") && !body.contains("radeon"), "{body}");
}

#[test]
fn a_browser_that_will_not_open_is_called_out() {
    let (mut ui, opened) = reporting(Err("no browser".into()));
    click_button(&mut ui, "Report bug");
    assert_eq!(opened.borrow().len(), 1);
    let names = names(&ui.access_tree());
    assert!(names.iter().any(|n| n.contains("Could not open the browser")), "{names:?}");
}

#[test]
fn user_folders_are_hidden_on_every_system() {
    for path in
        ["/home/ana/a.json", "/var/home/ana/a.json", "/Users/ana/a b.json", r"C:\Users\Ana Silva\a.json"]
    {
        let redacted = redact_user_paths(&format!("before {path}\nafter"));
        assert!(!redacted.contains("ana") && !redacted.contains("Ana") && !redacted.contains("a.json"));
        assert!(redacted.contains("<user folder>") && redacted.ends_with("\nafter"), "{redacted}");
    }
    assert_eq!(redact_user_paths("/usr/share/vulkan/icd.d"), "/usr/share/vulkan/icd.d");
}

#[test]
fn a_query_already_in_the_url_is_kept() {
    let url = new_issue_url("https://example.com/new?template=bug.md", "Steps", "GPU", "");
    assert!(url.starts_with("https://example.com/new?template=bug.md&body=Steps"), "{url}");
}

#[test]
fn what_a_report_cannot_change_is_encoded() {
    let url = new_issue_url(NEW_ISSUE, "a&b=c #d", "é 100%\n", "");
    let (_, encoded) = url.split_once("body=").expect("a body");
    assert!(!encoded.contains(['&', '=', '#', ' ', '\n']), "{encoded}");
    assert_eq!(body_of(&url), "a&b=c #d\n\n```text\né 100%\n```\n");
}

#[test]
fn a_short_report_is_not_cut() {
    let url = new_issue_url(NEW_ISSUE, "Steps", "GPU: Intel\n", "CUT");
    assert!(!body_of(&url).contains("CUT"));
}

#[test]
fn a_long_report_fits_and_ends_cleanly() {
    let report = "Placa de vídeo não é usada ✓\n".repeat(2000);
    let url = new_issue_url(NEW_ISSUE, "Steps", &report, "CUT");
    assert!(url.len() <= MAX_ISSUE_URL, "{}", url.len());
    assert!(url.len() > MAX_ISSUE_URL / 2, "it should use the room it has: {}", url.len());
    let body = body_of(&url);
    assert!(body.ends_with("usada ✓\n```\nCUT"), "cut on a whole line, with the block closed");
}

#[test]
fn a_long_line_is_cut_between_characters() {
    let url = new_issue_url(NEW_ISSUE, "Steps", &"é✓😀".repeat(2000), "CUT");
    assert!(url.len() <= MAX_ISSUE_URL, "{}", url.len());
    assert!(body_of(&url).ends_with("\n```\nCUT"));
}
