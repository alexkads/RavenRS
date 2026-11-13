//! `MovieEditor` and what it edits: a real film cut, dressed and read back.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, Color, Primitive, Quad, Rect, Size};
use ravenrs_movie::{
    CalloutStyle, Composition, EditorLabels, EditorState, FilmBuilder, Movie, MovieEditor, OverlayKind,
    Region,
};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn film() -> Movie {
    let mut film = FilmBuilder::new(Size::new(800.0, 500.0), Color::WHITE);
    for i in 0..=10u64 {
        let square = Quad::new(Rect::new(i as f32 * 10.0, 100.0, 40.0, 40.0), Color::BLACK);
        film.frame(i * 1000, &[Primitive::Quad(square)]);
    }
    film.pointer(0, ravenrs_core::Point::new(10.0, 10.0));
    film.finish(10_000).expect("film")
}

fn editor(state: &EditorState) -> Ui {
    let mut ui = Ui::new(MovieEditor::new(state.clone()), Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 900.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .or_else(|| tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name))))
        .unwrap_or_else(|| {
            let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
            panic!("no {name} in {names:?}")
        })
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

fn has(ui: &mut Ui, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
}

#[test]
fn cutting_out_a_stretch_with_the_buttons_and_taking_it_back() {
    let state = EditorState::new(film());
    let mut ui = editor(&state);
    state.seek(2_000);
    ui.update();
    click(&mut ui, "Cut here");
    state.seek(5_000);
    ui.update();
    click(&mut ui, "Cut here");
    assert_eq!(state.movie().project().clips.len(), 3);

    click(&mut ui, "Film 2");
    click(&mut ui, "Delete and close");
    assert_eq!(state.movie().duration_ms(), 7_000);

    click(&mut ui, "Undo");
    assert_eq!(state.movie().duration_ms(), 10_000);
    click(&mut ui, "Redo");
    assert_eq!(state.movie().duration_ms(), 7_000);
}

#[test]
fn an_effect_from_the_list_lands_on_a_track_and_its_properties_show() {
    let state = EditorState::new(film());
    let mut ui = editor(&state);
    assert!(has(&mut ui, "Select something"));
    click(&mut ui, "Callout");
    let project = state.movie();
    assert_eq!(project.project().overlays.len(), 1);
    assert!(has(&mut ui, "Text"), "its text can be changed");
    assert!(has(&mut ui, "Duration"));
    assert!(has(&mut ui, "Drag it on the canvas"));
    click(&mut ui, "Delete and close");
    assert!(state.movie().project().overlays.is_empty());
}

#[test]
fn every_effect_draws_at_every_moment_with_nothing_breaking() {
    let state = EditorState::new(film());
    let region = Region::new(100.0, 100.0, 200.0, 100.0);
    for kind in [
        OverlayKind::Callout { text: "Look".to_owned(), region, style: CalloutStyle::Label, soft: true },
        OverlayKind::Caption { text: "Words".to_owned() },
        OverlayKind::Spotlight { region },
        OverlayKind::Hide { region },
        OverlayKind::Zoom { region },
        OverlayKind::FadeIn,
        OverlayKind::FadeOut,
        OverlayKind::PointerHighlight,
        OverlayKind::Hotspot { region, title: "Spot".to_owned(), text: "Here".to_owned(), stop: true },
    ] {
        state.add_overlay(kind);
    }
    state.add_title("Part one");
    let mut ui = editor(&state);
    for ms in (0..=state.movie().duration_ms()).step_by(250) {
        state.seek(ms);
        ui.update();
    }
    let movie = state.movie();
    let bytes = movie.to_bytes();
    let back = Movie::from_bytes(&bytes).expect("read back");
    assert_eq!(back.project(), movie.project(), "the edits travel with the film");
}

#[test]
fn a_zoom_draws_the_film_bigger_without_any_pixels() {
    fn square_width(movie: Movie, at: u64) -> f32 {
        let mut ui = Ui::new(Composition::new(movie, at).width(800.0), Theme::raven_light());
        ui.set_viewport(Size::new(800.0, 500.0), 1.0);
        ui.update();
        let black = |p: &&Primitive| matches!(p, Primitive::Quad(q) if q.rect.width() < 790.0);
        match ui.scene().primitives().iter().find(black) {
            Some(Primitive::Quad(quad)) => quad.rect.width(),
            _ => 0.0,
        }
    }
    let plain = film();
    assert!((square_width(plain.clone(), 2_000) - 40.0).abs() < 0.5);
    let mut project = plain.project().clone();
    project.add_overlay(OverlayKind::Zoom { region: Region::new(0.0, 0.0, 200.0, 125.0) }, 1_000, 4_000);
    let zoomed = plain.with_project(project);
    assert!((square_width(zoomed.clone(), 3_000) - 160.0).abs() < 0.5, "four times nearer in the middle");
    assert!(square_width(zoomed.clone(), 1_100) < 160.0, "and on its way there");
    assert!((square_width(zoomed, 6_000) - 40.0).abs() < 0.5, "and back");
}

#[test]
fn the_words_can_be_portuguese() {
    let state = EditorState::new(film());
    let mut ui = Ui::new(MovieEditor::new(state).labels(EditorLabels::pt_br()), Theme::raven_light());
    ui.set_viewport(Size::new(1400.0, 900.0), 1.0);
    ui.update();
    assert!(has(&mut ui, "Cortar aqui"));
    click(&mut ui, "Tela");
    assert!(has(&mut ui, "Ocultar uma região"));
}

#[test]
fn the_tabs_change_the_library_and_trying_it_opens_the_player() {
    let state = EditorState::new(film());
    let mut ui = editor(&state);
    assert!(has(&mut ui, "Callout"));
    click(&mut ui, "Screen");
    assert!(has(&mut ui, "Zoom and pan"));
    assert!(!has(&mut ui, "Callout"));
    click(&mut ui, "Try it as a learner");
    assert!(has(&mut ui, "Stop at each step"), "the same player people will see");
    click(&mut ui, "Back to editing");
    assert!(!has(&mut ui, "Stop at each step"));
}

#[test]
fn a_chapter_added_at_the_playhead_shows_in_the_film() {
    let state = EditorState::new(film());
    let mut ui = editor(&state);
    state.seek(3_000);
    ui.update();
    click(&mut ui, "Chapter");
    assert_eq!(state.movie().chapters()[0].at_ms, 3_000);
    assert_eq!(state.movie().project().markers.len(), 1);
    click(&mut ui, "Undo");
    assert!(state.movie().chapters().is_empty());
}

#[test]
fn a_clip_can_be_sped_up_from_the_timeline_bar() {
    let state = EditorState::new(film());
    state.select_clip(0);
    state.set_speed(2.0);
    assert_eq!(state.movie().duration_ms(), 5_000);
}

#[test]
fn the_steps_recorded_with_the_film_show_on_their_track_and_go_to_their_moment() {
    let mut film_builder = FilmBuilder::new(Size::new(800.0, 500.0), Color::WHITE);
    film_builder.frame(0, &[Primitive::Quad(Quad::new(Rect::new(0.0, 0.0, 9.0, 9.0), Color::BLACK))]);
    film_builder.hotspot(ravenrs_movie::Hotspot {
        kind: ravenrs_movie::HotspotKind::Stop,
        x: 1.0,
        y: 1.0,
        width: 9.0,
        height: 9.0,
        title: "Click here".to_owned(),
        text: String::new(),
        from_ms: 4_000,
        until_ms: 4_000,
    });
    let state = EditorState::new(film_builder.finish(10_000).expect("film"));
    let mut ui = editor(&state);
    click(&mut ui, "Click here");
    assert_eq!(state.position_ms(), 4_000);
    assert!(state.movie().project().overlays.is_empty(), "looking does not edit");
}
