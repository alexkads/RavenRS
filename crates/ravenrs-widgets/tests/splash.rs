//! `SplashScreen`: covers the window, keeps the page out of reach, says what is
//! happening and leaves with a transition.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{
    AccessTree, Brush, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Role, Size,
};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

const FRAME: f64 = 1.0 / 60.0;

fn running(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn frames(ui: &mut Ui, t: &mut f64, n: u32) {
    for _ in 0..n {
        *t += FRAME;
        ui.set_time(*t);
        ui.update();
    }
}

fn settle(ui: &mut Ui, t: &mut f64) {
    for _ in 0..600 {
        ui.update();
        if !ui.is_animating() {
            return;
        }
        frames(ui, t, 1);
    }
    panic!("animations never finished");
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

/// The quad covering the whole window, last painted first.
fn cover(ui: &Ui) -> Option<Quad> {
    quads(ui).into_iter().rev().find(|q| q.rect.width() >= 800.0 && q.rect.height() >= 600.0)
}

fn click(ui: &mut Ui, at: Point) {
    let position = at;
    ui.handle_event(&Event::PointerMove { position });
    ui.handle_event(&Event::PointerDown { position, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position, button: PointerButton::Primary });
    ui.update();
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn status_text(tree: &AccessTree) -> (u64, String) {
    let status = tree.nodes.iter().find(|n| n.role == Role::Status).expect("a status node");
    let text = status
        .children
        .iter()
        .filter_map(|id| tree.get(*id).and_then(|n| n.name.clone()))
        .collect::<Vec<_>>()
        .join(" ");
    (status.id, text)
}

#[test]
fn it_covers_the_window_from_the_first_frame_with_an_opaque_canvas() {
    let visible = State::new(true);
    let ui = running(SplashScreen::new(visible.binding(), "Raven"));
    let cover = cover(&ui).expect("the splash covers the window");
    assert_eq!(cover.fill, Brush::Solid(Theme::raven_dark().color(Semantic::Canvas)), "no fade in");
    assert!(ui.next_wake().is_some(), "the spinner is live, so frames keep coming while it loads");
}

#[test]
fn it_opens_later_with_a_fade() {
    let visible = State::new(false);
    let mut ui = running(VStack::new((Button::new("Page"), SplashScreen::new(visible.binding(), "Raven"))));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    visible.set(true);
    ui.update();
    let Brush::Solid(fill) = cover(&ui).expect("the splash covers the window").fill else {
        panic!("a solid fill")
    };
    assert!(fill.a < Theme::raven_dark().color(Semantic::Canvas).a, "it fades in");
}

#[test]
fn the_page_behind_is_out_of_reach_until_it_leaves() {
    let visible = State::new(false);
    let clicks = State::new(0);
    let counter = clicks.clone();
    let mut ui = running(VStack::new((
        Button::new("Behind").on_click(move || counter.update(|c| *c += 1)),
        SplashScreen::new(visible.binding(), "Raven").status("Loading"),
    )));
    let button = quads(&ui)[0].rect;
    press(&mut ui, Key::Tab);
    assert!(ui.access_tree().focused().is_some(), "the button has focus");

    visible.set(true);
    ui.update();
    assert!(ui.access_tree().focused().is_none(), "focus leaves the page");
    click(&mut ui, button.center());
    press(&mut ui, Key::Enter);
    press(&mut ui, Key::Tab);
    press(&mut ui, Key::Enter);
    assert_eq!(clicks.get(), 0, "neither mouse nor keyboard reach the page");
    press(&mut ui, Key::Escape);
    assert!(visible.get(), "the app decides when it is done, not Escape");
    let tree = ui.access_tree();
    assert!(tree.find(Role::Button, "Behind").is_none(), "the page behind is inert");

    visible.set(false);
    ui.update();
    ui.update();
    assert_eq!(
        ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref(),
        Some("Behind"),
        "focus returns"
    );
    press(&mut ui, Key::Enter);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn screen_readers_get_the_name_and_each_new_status() {
    let visible = State::new(true);
    let status = State::new("Opening the database".to_owned());
    let message = status.clone();
    let mut ui = running(view_fn(move || {
        SplashScreen::new(visible.binding(), "Raven Finance").icon(IconName::Wallet).status(message.get())
    }));
    let tree = ui.access_tree();
    let dialog = tree.find(Role::Dialog, "Raven Finance").expect("named after the app");
    assert!(dialog.modal);
    let (first, text) = status_text(&tree);
    assert_eq!(text, "Opening the database");

    status.set("Loading your accounts".to_owned());
    ui.update();
    let (second, text) = status_text(&ui.access_tree());
    assert_eq!(text, "Loading your accounts");
    assert_ne!(first, second, "a new node, which the web bridge announces");
}

#[test]
fn it_shows_a_spinner_until_there_is_progress() {
    let visible = State::new(true);
    let progress = State::new(None::<f32>);
    let value = progress.clone();
    let mut ui = running(view_fn(move || {
        let splash = SplashScreen::new(visible.binding(), "Raven");
        match value.get() {
            Some(v) => splash.progress(v),
            None => splash,
        }
    }));
    let indicators = |tree: &AccessTree| {
        tree.nodes.iter().filter(|n| n.role == Role::ProgressIndicator).cloned().collect::<Vec<_>>()
    };
    let tree = ui.access_tree();
    let spinner = indicators(&tree);
    assert_eq!(spinner.len(), 1);
    assert_eq!(spinner[0].name.as_deref(), Some("Loading"));
    assert_eq!(spinner[0].value, None);

    progress.set(Some(0.4));
    ui.update();
    let mut t = 0.0;
    settle_except_live(&mut ui, &mut t);
    let tree = ui.access_tree();
    let bar = indicators(&tree);
    assert_eq!(bar.len(), 1, "the bar took the spinner's place");
    assert_eq!(bar[0].name.as_deref(), Some("Loading"));
    assert_eq!(bar[0].value.as_deref(), Some("40%"));
}

/// Runs long enough for the transitions to end, even with a spinner still live.
fn settle_except_live(ui: &mut Ui, t: &mut f64) {
    frames(ui, t, 120);
}

#[test]
fn it_leaves_with_a_fade_and_then_costs_nothing() {
    let visible = State::new(true);
    let clicks = State::new(0);
    let counter = clicks.clone();
    let mut ui = running(VStack::new((
        Button::new("Behind").on_click(move || counter.update(|c| *c += 1)),
        SplashScreen::new(visible.binding(), "Raven").progress(0.5),
    )));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    assert!(!ui.needs_redraw(), "a progress bar at rest asks for no frames");
    visible.set(false);
    ui.update();
    frames(&mut ui, &mut t, 2);
    assert!(ui.is_animating());
    assert!(cover(&ui).is_some(), "still fading out");
    assert!(ui.access_tree().find(Role::Dialog, "Raven").is_none(), "for the app it is already gone");
    settle(&mut ui, &mut t);
    assert!(cover(&ui).is_none());
    assert!(!ui.needs_redraw());
    let button = quads(&ui)[0].rect;
    click(&mut ui, button.center());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn reduced_motion_fades_without_scaling() {
    let visible = State::new(true);
    let mut ui = running(SplashScreen::new(visible.binding(), "Raven").progress(0.5));
    ui.set_reduced_motion(true);
    ui.update();
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    visible.set(false);
    ui.update();
    frames(&mut ui, &mut t, 3);
    let cover = cover(&ui).expect("still fading out");
    assert_eq!((cover.rect.width(), cover.rect.height()), (800.0, 600.0), "nothing scales");
    let Brush::Solid(fill) = cover.fill else { panic!("a solid fill") };
    assert!(fill.a < 1.0, "the fade still plays");
    settle(&mut ui, &mut t);
}

#[test]
fn a_long_name_or_status_stays_inside_a_small_window() {
    let visible = State::new(true);
    let long = "A name far too long to fit on a single line of such a small window";
    let mut ui =
        Ui::new(SplashScreen::new(visible.binding(), long).status(long).progress(0.5), Theme::raven_dark());
    ui.set_viewport(Size::new(240.0, 400.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    let tree = ui.access_tree();
    for node in tree.nodes.iter().filter(|n| n.role != Role::Window) {
        assert!(
            node.bounds.x() >= 0.0 && node.bounds.max_x() <= 240.0,
            "{:?} {:?} leaves the window: {:?}",
            node.role,
            node.name,
            node.bounds
        );
    }
}

#[test]
fn a_logo_takes_the_place_of_the_icon() {
    let visible = State::new(true);
    let mut ui = running(
        SplashScreen::new(visible.binding(), "Raven").icon(IconName::Wallet).logo(Text::new("Artwork")),
    );
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Artwork")));
}
