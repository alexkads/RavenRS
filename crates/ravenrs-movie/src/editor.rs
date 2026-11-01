//! The editor, in the style of screen-recording editors: effects on the left, the
//! canvas in the middle, properties on the right and the timeline along the bottom.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Key, Point, Rect, Size};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Divider, Drag, DragPhase, HStack, IconButton,
    IconName, List, ListItem, NodeId, Paint, QuadPaint, ScrollView, Select, Semantics, Slider, Spacer,
    Stepper, Text, TextField, Timeline, TimelineEdit, TimelineItem, TimelineMarker, TimelineTrack, Toggle,
    Tone, ToolRail, VStack, View, ViewExt,
};

use crate::composition::{Composition, camera_at};
use crate::movie::Movie;
use crate::player::{MovieLabels, MoviePlayer};
use crate::project::{CalloutStyle, Clip, ClipKind, Marker, Overlay, OverlayKind, Project, Region};

const RAIL_WIDTH: f32 = 76.0;
const LIBRARY_WIDTH: f32 = 236.0;
const PROPERTIES_WIDTH: f32 = 288.0;
const TOP_BAR: f32 = 52.0;
const TRANSPORT: f32 = 48.0;
const TIMELINE_BAR: f32 = 40.0;
const TRACK_HEADS: f32 = 112.0;
const OVERLAY_ROW: f32 = 34.0;
const CLIP_ROW: f32 = 56.0;
const THUMBNAIL: f32 = 64.0;
const HANDLE: f32 = 8.0;
const FRAME: f64 = 1.0 / 30.0;
const SPEEDS: [f32; 5] = [0.5, 1.0, 1.5, 2.0, 4.0];
/// How long a new effect lasts, in milliseconds.
const NEW_EFFECT_MS: u64 = 3000;
/// How far one undo goes back at most.
const UNDO_LIMIT: usize = 200;

/// The words a [`MovieEditor`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct EditorLabels {
    /// Tries the film the way people will watch it.
    pub try_it: String,
    /// Goes back from trying it.
    pub back_to_editing: String,
    /// Adds a chapter at the playhead.
    pub chapter: String,
    /// A chapter's first name.
    pub new_chapter: String,
    /// The tool tab with callouts and captions.
    pub tab_annotations: String,
    /// The tool tab with what acts on the screen.
    pub tab_screen: String,
    /// The tool tab with fades.
    pub tab_transitions: String,
    /// The tool tab for the pointer.
    pub tab_pointer: String,
    /// The tool tab with clickable spots.
    pub tab_interaction: String,
    /// The tool bar's name.
    pub tools: String,
    /// The hint at the foot of the library.
    pub library_hint: String,
    /// The film's row on the timeline.
    pub track_film: String,
    /// How a callout looks.
    pub style: String,
    /// A balloon with a colored edge.
    pub style_balloon: String,
    /// A filled label.
    pub style_label: String,
    /// A callout comes and goes gradually.
    pub soft: String,
    /// What being soft means.
    pub soft_hint: String,
    /// When it starts.
    pub start: String,
    /// The film's size and length, under its name.
    pub timeline: String,
    /// Takes the last edit back.
    pub undo: String,
    /// Puts it back.
    pub redo: String,
    /// Cuts the clip at the playhead.
    pub split: String,
    /// Deletes what is selected.
    pub delete: String,
    /// Saves the film with its edits.
    pub save: String,
    /// Plays.
    pub play: String,
    /// Pauses.
    pub pause: String,
    /// The heading of the effects.
    pub effects: String,
    /// The heading of the properties.
    pub properties: String,
    /// The properties panel with nothing selected.
    pub nothing_selected: String,
    /// A clip of the film.
    pub film_clip: String,
    /// A title card.
    pub title_card: String,
    /// A balloon with text.
    pub callout: String,
    /// A line of subtitles.
    pub caption: String,
    /// Lights up one region.
    pub spotlight: String,
    /// Covers a region.
    pub hide: String,
    /// Zooms into a region.
    pub zoom: String,
    /// Comes in from black.
    pub fade_in: String,
    /// Goes out to black.
    pub fade_out: String,
    /// A circle around the pointer.
    pub pointer_highlight: String,
    /// A spot that talks to the watcher.
    pub hotspot: String,
    /// The text of what is selected.
    pub text: String,
    /// A title card's second line.
    pub subtitle: String,
    /// How long it lasts.
    pub duration: String,
    /// How fast a clip plays.
    pub speed: String,
    /// A hotspot waits for a click.
    pub wait_for_click: String,
    /// The zoom of the timeline itself.
    pub timeline_zoom: String,
    /// The position of the playhead.
    pub position: String,
    /// Goes to the chapter before.
    pub previous_chapter: String,
    /// Goes to the chapter after.
    pub next_chapter: String,
    /// How a region is placed.
    pub drag_hint: String,
    /// The first words of a new callout.
    pub new_text: String,
}

impl Default for EditorLabels {
    fn default() -> Self {
        Self {
            try_it: "Try it as a learner".into(),
            back_to_editing: "Back to editing".into(),
            chapter: "Chapter".into(),
            new_chapter: "New chapter".into(),
            tab_annotations: "Notes".into(),
            tab_screen: "Screen".into(),
            tab_transitions: "Fades".into(),
            tab_pointer: "Pointer".into(),
            tab_interaction: "Clicks".into(),
            tools: "Tools".into(),
            library_hint: "Click to place it at the playhead.".into(),
            track_film: "Film".into(),
            style: "Style".into(),
            style_balloon: "Balloon".into(),
            style_label: "Label".into(),
            soft: "Appear gradually".into(),
            soft_hint: "Comes and goes little by little.".into(),
            start: "Start".into(),
            timeline: "Timeline".into(),
            undo: "Undo".into(),
            redo: "Redo".into(),
            split: "Cut here".into(),
            delete: "Delete and close the gap".into(),
            save: "Save film".into(),
            play: "Play".into(),
            pause: "Pause".into(),
            effects: "Effects".into(),
            properties: "Properties".into(),
            nothing_selected: "Select something on the timeline to change it.".into(),
            film_clip: "Film".into(),
            title_card: "Title card".into(),
            callout: "Callout".into(),
            caption: "Caption".into(),
            spotlight: "Spotlight".into(),
            hide: "Hide a region".into(),
            zoom: "Zoom and pan".into(),
            fade_in: "Fade in".into(),
            fade_out: "Fade out".into(),
            pointer_highlight: "Pointer highlight".into(),
            hotspot: "Clickable spot".into(),
            text: "Text".into(),
            subtitle: "Second line".into(),
            duration: "Duration".into(),
            speed: "Clip speed".into(),
            wait_for_click: "Wait for a click".into(),
            timeline_zoom: "Timeline zoom".into(),
            position: "Position".into(),
            previous_chapter: "Previous chapter".into(),
            next_chapter: "Next chapter".into(),
            drag_hint: "Drag it on the canvas to place it; the corner resizes it.".into(),
            new_text: "Write here".into(),
        }
    }
}

impl EditorLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            try_it: "Testar como aluno".into(),
            back_to_editing: "Voltar a editar".into(),
            chapter: "Capítulo".into(),
            new_chapter: "Novo capítulo".into(),
            tab_annotations: "Anotações".into(),
            tab_screen: "Tela".into(),
            tab_transitions: "Transições".into(),
            tab_pointer: "Ponteiro".into(),
            tab_interaction: "Interação".into(),
            tools: "Ferramentas".into(),
            library_hint: "Clique para pôr na agulha.".into(),
            track_film: "Filme".into(),
            style: "Estilo".into(),
            style_balloon: "Balão".into(),
            style_label: "Etiqueta".into(),
            soft: "Aparecer suavemente".into(),
            soft_hint: "Surge e some aos poucos.".into(),
            start: "Início".into(),
            timeline: "Linha do tempo".into(),
            undo: "Desfazer".into(),
            redo: "Refazer".into(),
            split: "Cortar aqui".into(),
            delete: "Excluir e fechar o vão".into(),
            save: "Salvar filme".into(),
            play: "Reproduzir".into(),
            pause: "Pausar".into(),
            effects: "Efeitos".into(),
            properties: "Propriedades".into(),
            nothing_selected: "Selecione algo na linha do tempo para alterar.".into(),
            film_clip: "Filme".into(),
            title_card: "Cartão de título".into(),
            callout: "Chamada".into(),
            caption: "Legenda".into(),
            spotlight: "Destaque".into(),
            hide: "Ocultar uma região".into(),
            zoom: "Zoom e panorâmica".into(),
            fade_in: "Surgir do preto".into(),
            fade_out: "Sumir no preto".into(),
            pointer_highlight: "Destacar o ponteiro".into(),
            hotspot: "Ponto clicável".into(),
            text: "Texto".into(),
            subtitle: "Segunda linha".into(),
            duration: "Duração".into(),
            speed: "Velocidade do trecho".into(),
            wait_for_click: "Esperar um clique".into(),
            timeline_zoom: "Zoom da linha do tempo".into(),
            position: "Posição".into(),
            previous_chapter: "Capítulo anterior".into(),
            next_chapter: "Próximo capítulo".into(),
            drag_hint: "Arraste na tela para posicionar; o canto muda o tamanho.".into(),
            new_text: "Escreva aqui".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Annotations,
    Screen,
    Transitions,
    Pointer,
    Interaction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Selection {
    Nothing,
    Clip(usize),
    Overlay(u64),
}

#[derive(Debug)]
struct Inner {
    movie: RefCell<Movie>,
    position: Cell<u64>,
    running: Cell<Option<(f64, u64)>>,
    selection: Cell<Selection>,
    undo: RefCell<Vec<Project>>,
    redo: RefCell<Vec<Project>>,
    /// What the last edit was, so that typing a word is one undo and not five.
    last_edit: RefCell<Option<String>>,
    /// Pixels of the timeline per second.
    density: Cell<f32>,
    tab: Cell<Tab>,
    /// The canvas shows the player, the way people will watch.
    trying: Cell<bool>,
}

/// What is being edited, kept by the app so that it can be read and saved. All
/// the edits the editor can make are methods here too, for scripts and tests.
#[derive(Clone, Debug)]
pub struct EditorState {
    inner: Rc<Inner>,
    revision: State<u64>,
}

impl EditorState {
    /// Starts editing `movie`, with whatever edits it already has.
    pub fn new(movie: Movie) -> Self {
        Self {
            inner: Rc::new(Inner {
                movie: RefCell::new(movie),
                position: Cell::new(0),
                running: Cell::new(None),
                selection: Cell::new(Selection::Nothing),
                undo: RefCell::new(Vec::new()),
                redo: RefCell::new(Vec::new()),
                last_edit: RefCell::new(None),
                density: Cell::new(28.0),
                tab: Cell::new(Tab::Annotations),
                trying: Cell::new(false),
            }),
            revision: State::new(0),
        }
    }

    /// The film with the edits so far.
    pub fn movie(&self) -> Movie {
        self.inner.movie.borrow().clone()
    }

    fn touch(&self) {
        self.revision.update(|n| *n += 1);
    }

    /// Makes an edit. Edits with the same `key` in a row count as one undo.
    fn edit(&self, key: Option<&str>, work: impl FnOnce(&mut Project)) {
        let inner = &self.inner;
        let mut project = inner.movie.borrow().project().clone();
        let same = key.is_some() && inner.last_edit.borrow().as_deref() == key;
        if !same {
            let mut undo = inner.undo.borrow_mut();
            undo.push(project.clone());
            if undo.len() > UNDO_LIMIT {
                undo.remove(0);
            }
            inner.redo.borrow_mut().clear();
        }
        *inner.last_edit.borrow_mut() = key.map(str::to_owned);
        work(&mut project);
        let movie = inner.movie.borrow().with_project(project);
        *inner.movie.borrow_mut() = movie;
        self.keep_position_inside();
        self.touch();
    }

    fn keep_position_inside(&self) {
        let duration = self.movie().duration_ms();
        if self.inner.position.get() > duration {
            self.inner.position.set(duration);
        }
    }

    fn swap(&self, from: &RefCell<Vec<Project>>, to: &RefCell<Vec<Project>>) {
        let Some(project) = from.borrow_mut().pop() else {
            return;
        };
        to.borrow_mut().push(self.movie().project().clone());
        let movie = self.movie().with_project(project);
        *self.inner.movie.borrow_mut() = movie;
        *self.inner.last_edit.borrow_mut() = None;
        self.inner.selection.set(Selection::Nothing);
        self.keep_position_inside();
        self.touch();
    }

    /// Takes the last edit back.
    pub fn undo(&self) {
        self.swap(&self.inner.undo, &self.inner.redo);
    }

    /// Puts back what [`Self::undo`] took.
    pub fn redo(&self) {
        self.swap(&self.inner.redo, &self.inner.undo);
    }

    /// Where the playhead is, in milliseconds of the timeline.
    pub fn position_ms(&self) -> u64 {
        self.inner.position.get()
    }

    /// Moves the playhead.
    pub fn seek(&self, ms: u64) {
        self.inner.position.set(ms.min(self.movie().duration_ms()));
        if self.inner.running.get().is_some() {
            self.inner.running.set(Some((f64::NAN, self.inner.position.get())));
        }
        self.touch();
    }

    /// Plays from the playhead, or pauses.
    pub fn toggle_playing(&self) {
        if self.inner.running.get().is_some() {
            self.inner.running.set(None);
        } else {
            if self.position_ms() >= self.movie().duration_ms() {
                self.inner.position.set(0);
            }
            self.inner.running.set(Some((f64::NAN, self.position_ms())));
        }
        self.touch();
    }

    /// Cuts the clip at the playhead in two.
    pub fn split(&self) {
        let at = self.position_ms();
        self.edit(None, |project| {
            project.split(at);
        });
    }

    /// Deletes what is selected: a clip takes its time with it.
    pub fn delete_selected(&self) {
        match self.inner.selection.get() {
            Selection::Clip(index) => self.edit(None, |project| project.delete_clip(index)),
            Selection::Overlay(id) => self.edit(None, |project| project.remove_overlay(id)),
            Selection::Nothing => return,
        }
        self.inner.selection.set(Selection::Nothing);
        self.touch();
    }

    /// Selects the clip at this place on the main track.
    pub fn select_clip(&self, index: usize) {
        self.inner.selection.set(Selection::Clip(index));
        *self.inner.last_edit.borrow_mut() = None;
        self.touch();
    }

    /// Selects an overlay.
    pub fn select_overlay(&self, id: u64) {
        self.inner.selection.set(Selection::Overlay(id));
        *self.inner.last_edit.borrow_mut() = None;
        self.touch();
    }

    /// Adds an effect at the playhead and selects it.
    pub fn add_overlay(&self, kind: OverlayKind) -> u64 {
        let at = self.position_ms();
        let mut id = 0;
        self.edit(None, |project| {
            let length = NEW_EFFECT_MS.min(project.duration_ms().saturating_sub(at).max(500));
            id = project.add_overlay(kind, at, length);
        });
        self.inner.selection.set(Selection::Overlay(id));
        self.touch();
        id
    }

    /// Puts a title card at the playhead and selects it.
    pub fn add_title(&self, title: &str) {
        let at = self.position_ms();
        let mut index = 0;
        self.edit(None, |project| index = project.insert_title(at, title, "", NEW_EFFECT_MS));
        self.inner.selection.set(Selection::Clip(index));
        self.touch();
    }

    /// Starts a chapter at the playhead.
    pub fn add_chapter(&self, title: &str) {
        let at_ms = self.position_ms();
        self.edit(None, |project| project.markers.push(Marker { title: title.to_owned(), at_ms }));
    }

    /// Changes how fast the selected clip plays.
    pub fn set_speed(&self, speed: f32) {
        if let Selection::Clip(index) = self.inner.selection.get() {
            self.edit(None, |project| project.set_speed(index, speed));
        }
    }

    fn change_overlay(&self, id: u64, key: &str, work: impl FnOnce(&mut Overlay)) {
        self.edit(Some(&format!("{key}:{id}")), |project| {
            if let Some(overlay) = project.overlay_mut(id) {
                work(overlay);
            }
        });
    }

    fn selected_overlay(&self) -> Option<Overlay> {
        let Selection::Overlay(id) = self.inner.selection.get() else {
            return None;
        };
        self.movie().project().overlays.iter().find(|o| o.id == id).cloned()
    }

    fn selected_clip(&self) -> Option<(usize, Clip)> {
        let Selection::Clip(index) = self.inner.selection.get() else {
            return None;
        };
        self.movie().project().clips.get(index).cloned().map(|clip| (index, clip))
    }

    /// Runs the clock while playing. Called while building: only plain cells change.
    fn advance(&self, now: f64) {
        let Some((since, from)) = self.inner.running.get() else {
            return;
        };
        if since.is_nan() {
            self.inner.running.set(Some((now, from)));
            return;
        }
        let at = from + ((now - since).max(0.0) * 1000.0) as u64;
        let duration = self.movie().duration_ms();
        if at >= duration {
            self.inner.position.set(duration);
            self.inner.running.set(None);
        } else {
            self.inner.position.set(at);
        }
    }
}

fn region_of(kind: &OverlayKind) -> Option<Region> {
    match kind {
        OverlayKind::Callout { region, .. }
        | OverlayKind::Spotlight { region }
        | OverlayKind::Hide { region }
        | OverlayKind::Zoom { region }
        | OverlayKind::Hotspot { region, .. } => Some(*region),
        _ => None,
    }
}

fn set_region(kind: &mut OverlayKind, new: Region) {
    match kind {
        OverlayKind::Callout { region, .. }
        | OverlayKind::Spotlight { region }
        | OverlayKind::Hide { region }
        | OverlayKind::Zoom { region }
        | OverlayKind::Hotspot { region, .. } => *region = new,
        _ => {}
    }
}

fn label_of<'a>(kind: &OverlayKind, labels: &'a EditorLabels) -> &'a str {
    match kind {
        OverlayKind::Callout { .. } => &labels.callout,
        OverlayKind::Caption { .. } => &labels.caption,
        OverlayKind::Spotlight { .. } => &labels.spotlight,
        OverlayKind::Hide { .. } => &labels.hide,
        OverlayKind::Zoom { .. } => &labels.zoom,
        OverlayKind::FadeIn => &labels.fade_in,
        OverlayKind::FadeOut => &labels.fade_out,
        OverlayKind::PointerHighlight => &labels.pointer_highlight,
        OverlayKind::Hotspot { .. } => &labels.hotspot,
    }
}

/// What an overlay goes under on the timeline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Interaction,
    Annotations,
    Screen,
    Pointer,
}

fn row_of(kind: &OverlayKind) -> Row {
    match kind {
        OverlayKind::Hotspot { .. } => Row::Interaction,
        OverlayKind::Callout { .. } | OverlayKind::Caption { .. } => Row::Annotations,
        OverlayKind::PointerHighlight => Row::Pointer,
        _ => Row::Screen,
    }
}

/// The ids blocks have on the timeline: clips below, overlays above.
const OVERLAY_IDS: u64 = 1 << 32;
/// The steps that came with the film, which only say where they are.
const FILM_STEP_IDS: u64 = 2 << 32;
/// A step lasts no time, but it has to be seen and clicked.
const STEP_MS: u64 = 700;

fn on_timeline_edit(state: &EditorState, edit: TimelineEdit) {
    match edit {
        TimelineEdit::Began { .. } => *state.inner.last_edit.borrow_mut() = None,
        TimelineEdit::Moved { id, start_ms } if (OVERLAY_IDS..FILM_STEP_IDS).contains(&id) => {
            let id = id - OVERLAY_IDS;
            state.inner.selection.set(Selection::Overlay(id));
            state.change_overlay(id, "move", |overlay| overlay.start_ms = start_ms);
        }
        TimelineEdit::TrimmedStart { id, by_ms } if (OVERLAY_IDS..FILM_STEP_IDS).contains(&id) => {
            state.change_overlay(id - OVERLAY_IDS, "resize", |overlay| {
                let end = overlay.end_ms();
                let start = (overlay.start_ms as i64 + by_ms).clamp(0, end as i64 - 100) as u64;
                overlay.start_ms = start;
                overlay.duration_ms = end - start;
            });
        }
        TimelineEdit::TrimmedEnd { id, by_ms } if (OVERLAY_IDS..FILM_STEP_IDS).contains(&id) => {
            state.change_overlay(id - OVERLAY_IDS, "resize", |overlay| {
                overlay.duration_ms = (overlay.duration_ms as i64 + by_ms).max(100) as u64;
            });
        }
        TimelineEdit::TrimmedStart { id, by_ms } => {
            let index = id as usize;
            state.edit(Some(&format!("trim:{index}")), |project| project.trim_start(index, by_ms));
        }
        TimelineEdit::TrimmedEnd { id, by_ms } => {
            let index = id as usize;
            state.edit(Some(&format!("trim:{index}")), |project| project.trim_end(index, by_ms));
        }
        _ => {}
    }
}

fn timeline(state: &EditorState, labels: &EditorLabels) -> Timeline {
    let movie = state.movie();
    let project = movie.project();
    let selection = state.inner.selection.get();
    let overlays_in = |row: Row| -> Vec<TimelineItem> {
        project
            .overlays
            .iter()
            .filter(|overlay| row_of(&overlay.kind) == row)
            .map(|overlay| {
                let tone = match row {
                    Row::Interaction => Tone::Warning,
                    Row::Annotations => Tone::Accent,
                    Row::Screen => Tone::Success,
                    Row::Pointer => Tone::Neutral,
                };
                TimelineItem::new(
                    overlay.id + OVERLAY_IDS,
                    overlay.start_ms,
                    overlay.duration_ms,
                    label_of(&overlay.kind, labels),
                )
                .tone(tone)
                .selected(selection == Selection::Overlay(overlay.id))
                .movable()
                .trimmable()
            })
            .collect()
    };
    let clips: Vec<TimelineItem> = project
        .clips
        .iter()
        .enumerate()
        .map(|(index, clip)| {
            let start = project.clip_start_ms(index);
            let label = match &clip.kind {
                ClipKind::Film { .. } if (clip.speed - 1.0).abs() > 0.01 => {
                    format!("{} {} · {}×", labels.film_clip, index + 1, clip.speed)
                }
                ClipKind::Film { .. } => format!("{} {}", labels.film_clip, index + 1),
                ClipKind::Title { title, .. } => title.clone(),
            };
            let moment = if start == 0 { movie.poster_ms() } else { start + 1 };
            let item = TimelineItem::new(index as u64, start, clip.duration_ms(), label)
                .tone(if matches!(clip.kind, ClipKind::Title { .. }) { Tone::Success } else { Tone::Accent })
                .selected(selection == Selection::Clip(index));
            // What a stretch of film starts with, in vectors like the rest. A card
            // has its words on the block already, and they would not fit in a thumbnail.
            if matches!(clip.kind, ClipKind::Film { .. }) {
                item.leading(Composition::new(movie.clone(), moment).width(THUMBNAIL)).trimmable()
            } else {
                item
            }
        })
        .collect();
    // The steps recorded with the film, beside those added here.
    let added: Vec<&str> = project
        .overlays
        .iter()
        .filter_map(|overlay| match &overlay.kind {
            OverlayKind::Hotspot { title, .. } => Some(title.as_str()),
            _ => None,
        })
        .collect();
    let mut steps = overlays_in(Row::Interaction);
    for (index, spot) in movie.hotspots().iter().enumerate() {
        if !added.contains(&spot.title.as_str()) {
            let length = spot.until_ms.saturating_sub(spot.from_ms).max(STEP_MS);
            steps.push(
                TimelineItem::new(FILM_STEP_IDS + index as u64, spot.from_ms, length, spot.title.clone())
                    .tone(Tone::Warning),
            );
        }
    }
    let starts: Vec<u64> = movie.hotspots().iter().map(|spot| spot.from_ms).collect();
    let (select, seek, edit) = (state.clone(), state.clone(), state.clone());
    Timeline::new(vec![
        TimelineTrack::new(labels.tab_interaction.clone(), OVERLAY_ROW, steps),
        TimelineTrack::new(labels.tab_annotations.clone(), OVERLAY_ROW, overlays_in(Row::Annotations)),
        TimelineTrack::new(labels.tab_screen.clone(), OVERLAY_ROW, overlays_in(Row::Screen)),
        TimelineTrack::new(labels.tab_pointer.clone(), OVERLAY_ROW, overlays_in(Row::Pointer)),
        TimelineTrack::new(labels.track_film.clone(), CLIP_ROW, clips),
    ])
    .markers(
        movie.chapters().iter().map(|c| TimelineMarker { at_ms: c.at_ms, title: c.title.clone() }).collect(),
    )
    .position_ms(state.position_ms())
    .pixels_per_second(state.inner.density.get())
    .heads_width(TRACK_HEADS)
    .on_seek(move |ms| seek.seek(ms))
    .on_select(move |id| {
        if id >= FILM_STEP_IDS {
            if let Some(at) = usize::try_from(id - FILM_STEP_IDS).ok().and_then(|i| starts.get(i)) {
                select.seek(*at);
            }
        } else if id >= OVERLAY_IDS {
            select.select_overlay(id - OVERLAY_IDS);
        } else {
            select.select_clip(id as usize);
        }
    })
    .on_edit(move |change| on_timeline_edit(&edit, change))
}

/// The selected region on the canvas, to drag and resize.
struct RegionHandle {
    state: EditorState,
    id: u64,
    frame: Rect,
    scale: f32,
}

impl View for RegionHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = theme.color(Semantic::FocusRing);
        let (state, id, scale) = (self.state.clone(), self.id, self.scale);
        let size = HANDLE * 1.5;
        let corner = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.width() - size, self.frame.height() - size)),
                width: Some(size),
                height: Some(size),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            Paint::Quad(QuadPaint { corner_radius: 2.0, ..QuadPaint::new(color) }),
        );
        cx.semantics(corner, Semantics::hidden());
        let start = cx.use_state(|| Rc::new(Cell::new(None::<(Point, Region)>))).get();
        let resize = state.clone();
        cx.on_drag(
            corner,
            Rc::new(move |drag: Drag| {
                if let Some(mut region) = resize.selected_overlay().and_then(|o| region_of(&o.kind)) {
                    if drag.phase == DragPhase::Start {
                        *resize.inner.last_edit.borrow_mut() = None;
                    }
                    region.width = ((drag.position.x - drag.container.x()) / scale).max(16.0);
                    region.height = ((drag.position.y - drag.container.y()) / scale).max(16.0);
                    resize.change_overlay(id, "region", |overlay| set_region(&mut overlay.kind, region));
                }
            }),
        );
        cx.cursor(corner, CursorIcon::ColResize);
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[corner],
            Paint::Quad(QuadPaint {
                fill: Color::TRANSPARENT.into(),
                corner_radius: 0.0,
                border: Some(Border { width: 2.0, color }),
                shadow: None,
            }),
        );
        cx.on_drag(
            node,
            Rc::new(move |drag: Drag| {
                let Some(region) = state.selected_overlay().and_then(|o| region_of(&o.kind)) else {
                    return;
                };
                if drag.phase == DragPhase::Start {
                    start.set(Some((drag.position, region)));
                    *state.inner.last_edit.borrow_mut() = None;
                    return;
                }
                if let Some((from, was)) = start.get() {
                    let moved = Region {
                        x: was.x + (drag.position.x - from.x) / scale,
                        y: was.y + (drag.position.y - from.y) / scale,
                        ..was
                    };
                    state.change_overlay(id, "region", |overlay| set_region(&mut overlay.kind, moved));
                }
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        cx.semantics(node, Semantics::hidden());
        node
    }
}

/// The canvas: the moment under the playhead, and the region being placed.
struct Canvas {
    state: EditorState,
    width: f32,
}

impl View for Canvas {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let state = &self.state;
        let movie = state.movie();
        let logical = movie.logical_size();
        let height = self.width * logical.height / logical.width.max(1.0);
        let selected = state.selected_overlay();
        let placing_zoom = matches!(selected.as_ref().map(|o| &o.kind), Some(OverlayKind::Zoom { .. }));
        let at = state.position_ms();
        let mut film = Composition::new(movie.clone(), at).width(self.width);
        if placing_zoom {
            film = film.ignoring_zoom();
        }
        let mut children = vec![cx.child(&film)];
        if let Some(overlay) = &selected
            && let Some(region) = region_of(&overlay.kind)
            && overlay.covers(at)
        {
            let camera = camera_at(&movie, at, self.width, placing_zoom);
            children.push(cx.child(&RegionHandle {
                state: state.clone(),
                id: overlay.id,
                frame: camera.region(region),
                scale: camera.scale(),
            }));
        }
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(height),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        cx.clip(node);
        node
    }
}

/// What runs when the save button is clicked.
type SaveHandler = Rc<dyn Fn(&Movie)>;

/// An editor for a film, in the style of screen-recording editors.
///
/// Tools are on the left, with what each offers; the canvas in the middle; what
/// is selected on the right; and the tracks along the bottom. Clips are cut at
/// the playhead, trimmed at their edges and deleted, closing the gap. Effects sit
/// on tracks over them, where they slide and stretch, and regions are dragged on
/// the canvas. Nothing changes the film, every edit can be undone, and the film
/// can be tried at any time with the same player people will watch it in.
///
/// ```no_run
/// use ravenrs_movie::{EditorState, Movie, MovieEditor};
///
/// let film = Movie::from_bytes(&std::fs::read("manual.ravenmovie").unwrap_or_default())?;
/// let state = EditorState::new(film);
/// let editor = MovieEditor::new(state).title("Getting started").on_save(|film: &Movie| {
///     let _ = std::fs::write("manual.ravenmovie", film.to_bytes());
/// });
/// # Ok::<(), ravenrs_movie::MovieError>(())
/// ```
pub struct MovieEditor {
    state: EditorState,
    labels: EditorLabels,
    player_labels: MovieLabels,
    title: String,
    on_save: Option<SaveHandler>,
    area: Option<Size>,
}

impl std::fmt::Debug for MovieEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieEditor").field("state", &self.state).finish_non_exhaustive()
    }
}

/// `2:05`.
fn clock(ms: u64) -> String {
    format!("{}:{:02}", ms / 60_000, ms / 1000 % 60)
}

fn panel_title(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn muted(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Callout).foreground(Semantic::InkMuted)
}

impl MovieEditor {
    /// An editor over `state`.
    pub fn new(state: EditorState) -> Self {
        Self {
            state,
            labels: EditorLabels::default(),
            player_labels: MovieLabels::default(),
            title: String::new(),
            on_save: None,
            area: None,
        }
    }

    /// The words it shows.
    pub fn labels(mut self, labels: EditorLabels) -> Self {
        self.labels = labels;
        self
    }

    /// The words of the player it tries the film in.
    pub fn player_labels(mut self, labels: MovieLabels) -> Self {
        self.player_labels = labels;
        self
    }

    /// The film's name, in the top bar.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Shows a save button that runs `handler` with the film and its edits.
    pub fn on_save(mut self, handler: impl Fn(&Movie) + 'static) -> Self {
        self.on_save = Some(Rc::new(handler));
        self
    }

    /// The room it has, for when it is not the whole window.
    pub fn area(mut self, area: Size) -> Self {
        self.area = Some(area);
        self
    }

    fn top_bar(&self) -> impl View + use<> {
        let labels = &self.labels;
        let state = &self.state;
        let movie = state.movie();
        let (undo, redo, trying) = (state.clone(), state.clone(), state.clone());
        let is_trying = state.inner.trying.get();
        let can_undo = !state.inner.undo.borrow().is_empty();
        let can_redo = !state.inner.redo.borrow().is_empty();
        let kilobytes = movie.to_bytes().len() / 1024;
        let save = self.on_save.clone().map(|save| {
            let state = state.clone();
            Button::new(labels.save.clone())
                .icon(IconName::Save)
                .size(ButtonSize::Small)
                .on_click(move || save(&state.movie()))
        });
        HStack::new((
            VStack::new((
                (!self.title.is_empty()).then(|| Text::new(self.title.clone()).font_weight(Weight::SEMIBOLD)),
                muted(&format!("{kilobytes} KB · {}", clock(movie.duration_ms()))),
            ))
            .spacing(0.0),
            IconButton::new(IconName::Undo2)
                .label(labels.undo.clone())
                .disabled(!can_undo)
                .on_click(move || undo.undo()),
            IconButton::new(IconName::Redo2)
                .label(labels.redo.clone())
                .disabled(!can_redo)
                .on_click(move || redo.redo()),
            Spacer::new(),
            Button::new(if is_trying { labels.back_to_editing.clone() } else { labels.try_it.clone() })
                .icon(if is_trying { IconName::Pencil } else { IconName::Play })
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    trying.inner.trying.set(!trying.inner.trying.get());
                    trying.inner.running.set(None);
                    trying.touch();
                }),
            save,
        ))
        .spacing(8.0)
        .align(Align::Center)
        .padding((0.0, 12.0))
        .frame(None, Some(TOP_BAR))
        .background(Semantic::Surface)
    }

    fn rail(&self) -> impl View + use<> {
        let labels = &self.labels;
        const TABS: [Tab; 5] =
            [Tab::Annotations, Tab::Screen, Tab::Transitions, Tab::Pointer, Tab::Interaction];
        let (read, write) = (self.state.clone(), self.state.clone());
        let selected = Binding::new(
            move || TABS.iter().position(|tab| *tab == read.inner.tab.get()).unwrap_or(0),
            move |index: usize| {
                write.inner.tab.set(TABS.get(index).copied().unwrap_or(Tab::Annotations));
                write.touch();
            },
        );
        ToolRail::new(
            selected,
            [
                (IconName::MessageSquare, labels.tab_annotations.clone()),
                (IconName::Maximize2, labels.tab_screen.clone()),
                (IconName::Layers, labels.tab_transitions.clone()),
                (IconName::MousePointer, labels.tab_pointer.clone()),
                (IconName::Crosshair, labels.tab_interaction.clone()),
            ],
        )
        .width(RAIL_WIDTH)
        .background(Semantic::Surface)
        .frame(Some(RAIL_WIDTH), None)
    }

    fn library(&self) -> impl View + use<> {
        let labels = &self.labels;
        let logical = self.state.movie().logical_size();
        let middle = Region::new(
            logical.width * 0.3,
            logical.height * 0.3,
            logical.width * 0.4,
            logical.height * 0.25,
        );
        let note = Region::new(
            logical.width * 0.62,
            logical.height * 0.12,
            logical.width * 0.3,
            logical.height * 0.12,
        );
        let text = labels.new_text.clone();
        let tab = self.state.inner.tab.get();
        let kinds: Vec<(String, IconName, OverlayKind)> = match tab {
            Tab::Annotations => vec![
                (
                    labels.callout.clone(),
                    IconName::MessageSquare,
                    OverlayKind::Callout {
                        text: text.clone(),
                        region: note,
                        style: CalloutStyle::Balloon,
                        soft: true,
                    },
                ),
                (labels.caption.clone(), IconName::Type, OverlayKind::Caption { text: text.clone() }),
            ],
            Tab::Screen => vec![
                (labels.spotlight.clone(), IconName::Sparkles, OverlayKind::Spotlight { region: middle }),
                (labels.zoom.clone(), IconName::ZoomIn, OverlayKind::Zoom { region: middle }),
                (labels.hide.clone(), IconName::EyeOff, OverlayKind::Hide { region: middle }),
            ],
            Tab::Transitions => vec![
                (labels.fade_in.clone(), IconName::Eye, OverlayKind::FadeIn),
                (labels.fade_out.clone(), IconName::EyeOff, OverlayKind::FadeOut),
            ],
            Tab::Pointer => {
                vec![(
                    labels.pointer_highlight.clone(),
                    IconName::MousePointer,
                    OverlayKind::PointerHighlight,
                )]
            }
            Tab::Interaction => vec![(
                labels.hotspot.clone(),
                IconName::Crosshair,
                OverlayKind::Hotspot { region: note, title: labels.hotspot.clone(), text, stop: true },
            )],
        };
        let title = match tab {
            Tab::Annotations => &labels.tab_annotations,
            Tab::Screen => &labels.tab_screen,
            Tab::Transitions => &labels.tab_transitions,
            Tab::Pointer => &labels.tab_pointer,
            Tab::Interaction => &labels.tab_interaction,
        };
        let mut rows: Vec<ListItem> = Vec::with_capacity(kinds.len() + 1);
        if tab == Tab::Annotations {
            let (titles, first_words) = (self.state.clone(), labels.new_text.clone());
            rows.push(
                ListItem::new(labels.title_card.clone())
                    .icon(IconName::Layers)
                    .on_click(move || titles.add_title(&first_words)),
            );
        }
        for (name, icon, kind) in kinds {
            let state = self.state.clone();
            rows.push(ListItem::new(name).icon(icon).on_click(move || {
                state.add_overlay(kind.clone());
            }));
        }
        VStack::new((panel_title(title), ScrollView::new((List::new(rows),)), muted(&labels.library_hint)))
            .spacing(12.0)
            .align(Align::Stretch)
            .padding(12.0)
            .background(Semantic::Surface)
            .frame(Some(LIBRARY_WIDTH), None)
    }

    fn properties(&self) -> impl View + use<> {
        let labels = &self.labels;
        let state = &self.state;
        let mut items: Vec<AnyView> = Vec::new();
        if let Some(overlay) = state.selected_overlay() {
            let id = overlay.id;
            items.push(Box::new(panel_title(label_of(&overlay.kind, labels))));
            let has_text = matches!(
                overlay.kind,
                OverlayKind::Callout { .. } | OverlayKind::Caption { .. } | OverlayKind::Hotspot { .. }
            );
            if has_text {
                let (read, write) = (state.clone(), state.clone());
                let text = Binding::new(
                    move || match read.selected_overlay().map(|o| o.kind) {
                        Some(
                            OverlayKind::Callout { text, .. }
                            | OverlayKind::Caption { text }
                            | OverlayKind::Hotspot { text, .. },
                        ) => text,
                        _ => String::new(),
                    },
                    move |new: String| {
                        write.change_overlay(id, "text", |overlay| match &mut overlay.kind {
                            OverlayKind::Callout { text, .. }
                            | OverlayKind::Caption { text }
                            | OverlayKind::Hotspot { text, .. } => *text = new,
                            _ => {}
                        });
                    },
                );
                items.push(Box::new(muted(&labels.text)));
                items.push(Box::new(TextField::new(text)));
            }
            if let OverlayKind::Callout { style, .. } = &overlay.kind {
                let (balloon, tag) = (state.clone(), state.clone());
                let pick = |state: EditorState, chosen: CalloutStyle| {
                    move || {
                        state.change_overlay(id, "style", |overlay| {
                            if let OverlayKind::Callout { style, .. } = &mut overlay.kind {
                                *style = chosen;
                            }
                        });
                    }
                };
                let chosen = |this: CalloutStyle| {
                    if *style == this { ButtonStyle::Primary } else { ButtonStyle::Secondary }
                };
                items.push(Box::new(muted(&labels.style)));
                items.push(Box::new(
                    HStack::new((
                        Button::new(labels.style_balloon.clone())
                            .size(ButtonSize::Small)
                            .style(chosen(CalloutStyle::Balloon))
                            .on_click(pick(balloon, CalloutStyle::Balloon)),
                        Button::new(labels.style_label.clone())
                            .size(ButtonSize::Small)
                            .style(chosen(CalloutStyle::Label))
                            .on_click(pick(tag, CalloutStyle::Label)),
                    ))
                    .spacing(4.0),
                ));
                let (read, write) = (state.clone(), state.clone());
                let soft = Binding::new(
                    move || {
                        matches!(
                            read.selected_overlay().map(|o| o.kind),
                            Some(OverlayKind::Callout { soft: true, .. })
                        )
                    },
                    move |new: bool| {
                        write.change_overlay(id, "soft", |overlay| {
                            if let OverlayKind::Callout { soft, .. } = &mut overlay.kind {
                                *soft = new;
                            }
                        });
                    },
                );
                items.push(Box::new(Toggle::new(soft).label(labels.soft.clone())));
            }
            if matches!(overlay.kind, OverlayKind::Hotspot { .. }) {
                let (read, write) = (state.clone(), state.clone());
                let stop = Binding::new(
                    move || {
                        matches!(
                            read.selected_overlay().map(|o| o.kind),
                            Some(OverlayKind::Hotspot { stop: true, .. })
                        )
                    },
                    move |new: bool| {
                        write.change_overlay(id, "stop", |overlay| {
                            if let OverlayKind::Hotspot { stop, .. } = &mut overlay.kind {
                                *stop = new;
                            }
                        });
                    },
                );
                items.push(Box::new(Toggle::new(stop).label(labels.wait_for_click.clone())));
            }
            let total = state.movie().duration_ms().max(1) as f32 / 1000.0;
            let (read, write) = (state.clone(), state.clone());
            let start = Binding::new(
                move || read.selected_overlay().map_or(0.0, |o| o.start_ms as f32 / 1000.0),
                move |seconds: f32| {
                    write.change_overlay(id, "start", |o| o.start_ms = (seconds * 1000.0) as u64)
                },
            );
            items.push(Box::new(muted(&format!("{}: {}", labels.start, clock(overlay.start_ms)))));
            items.push(Box::new(Slider::new(start).range(0.0, total).step(0.1).label(labels.start.clone())));
            let (read, write) = (state.clone(), state.clone());
            let duration = Binding::new(
                move || read.selected_overlay().map_or(1.0, |o| o.duration_ms as f32 / 1000.0),
                move |seconds: f32| {
                    write.change_overlay(id, "duration", |o| o.duration_ms = (seconds * 1000.0) as u64)
                },
            );
            items.push(Box::new(muted(&format!(
                "{}: {:.1} s",
                labels.duration,
                overlay.duration_ms as f32 / 1000.0
            ))));
            items.push(Box::new(
                Slider::new(duration).range(0.5, total.max(1.0)).step(0.1).label(labels.duration.clone()),
            ));
            if region_of(&overlay.kind).is_some() {
                let axis = |state: &EditorState, vertical: bool, name: &str| {
                    let (read, write) = (state.clone(), state.clone());
                    let value = Binding::new(
                        move || {
                            let region = read.selected_overlay().and_then(|o| region_of(&o.kind));
                            region.map_or(0, |r| if vertical { r.y as i32 } else { r.x as i32 })
                        },
                        move |new: i32| {
                            write.change_overlay(id, "region", |overlay| {
                                if let Some(mut region) = region_of(&overlay.kind) {
                                    if vertical {
                                        region.y = new as f32;
                                    } else {
                                        region.x = new as f32;
                                    }
                                    set_region(&mut overlay.kind, region);
                                }
                            });
                        },
                    );
                    Stepper::new(value).range(0, 8000).label(name.to_owned())
                };
                items.push(Box::new(axis(state, false, "X")));
                items.push(Box::new(axis(state, true, "Y")));
                items.push(Box::new(muted(&labels.drag_hint)));
            }
        } else if let Some((index, clip)) = state.selected_clip() {
            match &clip.kind {
                ClipKind::Film { .. } => {
                    items.push(Box::new(panel_title(&labels.film_clip)));
                    items.push(Box::new(muted(&clock(clip.duration_ms()))));
                }
                ClipKind::Title { .. } => {
                    items.push(Box::new(panel_title(&labels.title_card)));
                    let words = |second: bool| {
                        let (read, write) = (state.clone(), state.clone());
                        Binding::new(
                            move || match read.selected_clip().map(|(_, clip)| clip.kind) {
                                Some(ClipKind::Title { title, subtitle, .. }) => {
                                    if second {
                                        subtitle
                                    } else {
                                        title
                                    }
                                }
                                _ => String::new(),
                            },
                            move |new: String| {
                                write.edit(Some(&format!("card:{index}:{second}")), |project| {
                                    if let Some(ClipKind::Title { title, subtitle, .. }) =
                                        project.clips.get_mut(index).map(|clip| &mut clip.kind)
                                    {
                                        *(if second { subtitle } else { title }) = new;
                                    }
                                });
                            },
                        )
                    };
                    items.push(Box::new(muted(&labels.text)));
                    items.push(Box::new(TextField::new(words(false))));
                    items.push(Box::new(muted(&labels.subtitle)));
                    items.push(Box::new(TextField::new(words(true))));
                }
            }
        } else {
            items.push(Box::new(panel_title(&labels.properties)));
            items.push(Box::new(muted(&labels.nothing_selected)));
        }
        ScrollView::new((VStack::new(items).spacing(10.0).align(Align::Stretch),))
            .padding(12.0)
            .background(Semantic::Surface)
            .frame(Some(PROPERTIES_WIDTH), None)
    }

    fn transport(&self) -> impl View + use<> {
        let labels = &self.labels;
        let state = &self.state;
        let running = state.inner.running.get().is_some();
        let movie = state.movie();
        let at = state.position_ms();
        let chapter_to = |target: Option<u64>| {
            let seek = state.clone();
            move || {
                if let Some(ms) = target {
                    seek.seek(ms);
                }
            }
        };
        let previous = movie.chapters().iter().rev().find(|c| c.at_ms < at).map(|c| c.at_ms);
        let next = movie.chapters().iter().find(|c| c.at_ms > at).map(|c| c.at_ms);
        let toggle = state.clone();
        HStack::new((
            IconButton::new(IconName::ChevronLeft)
                .label(labels.previous_chapter.clone())
                .disabled(previous.is_none())
                .on_click(chapter_to(previous)),
            Button::new(if running { labels.pause.clone() } else { labels.play.clone() })
                .icon(if running { IconName::Pause } else { IconName::Play })
                .size(ButtonSize::Small)
                .on_click(move || toggle.toggle_playing()),
            IconButton::new(IconName::ChevronRight)
                .label(labels.next_chapter.clone())
                .disabled(next.is_none())
                .on_click(chapter_to(next)),
            Text::new(format!("{} / {}", clock(at), clock(movie.duration_ms()))).style(TextStyle::Callout),
        ))
        .spacing(6.0)
        .align(Align::Center)
        .frame(None, Some(TRANSPORT))
    }

    fn timeline_bar(&self) -> impl View + use<> {
        let labels = &self.labels;
        let state = &self.state;
        let (split, delete, chapter) = (state.clone(), state.clone(), state.clone());
        let new_chapter = labels.new_chapter.clone();
        let selected = state.inner.selection.get() != Selection::Nothing;
        let speed_clip = state.selected_clip().filter(|(_, clip)| matches!(clip.kind, ClipKind::Film { .. }));
        let (read, write) = (state.clone(), state.clone());
        let speed = Binding::new(
            move || {
                let speed = read.selected_clip().map_or(1.0, |(_, clip)| clip.speed);
                SPEEDS.iter().position(|s| (*s - speed).abs() < 0.01).unwrap_or(1)
            },
            move |choice: usize| write.set_speed(SPEEDS.get(choice).copied().unwrap_or(1.0)),
        );
        let (read, write) = (state.clone(), state.clone());
        let density = Binding::new(
            move || read.inner.density.get(),
            move |value: f32| {
                write.inner.density.set(value);
                write.touch();
            },
        );
        let tool = |label: &str, icon: IconName| {
            Button::new(label.to_owned()).icon(icon).style(ButtonStyle::Ghost).size(ButtonSize::Small)
        };
        HStack::new((
            tool(&labels.split, IconName::Scissors).on_click(move || split.split()),
            tool(&labels.delete, IconName::Trash2)
                .disabled(!selected)
                .on_click(move || delete.delete_selected()),
            tool(&labels.chapter, IconName::Flag).on_click(move || chapter.add_chapter(&new_chapter)),
            speed_clip.map(|_| {
                HStack::new((
                    muted(&labels.speed),
                    Select::new(speed, SPEEDS.iter().map(|s| format!("{s}×"))).width(90.0),
                ))
                .spacing(6.0)
                .align(Align::Center)
            }),
            Spacer::new(),
            Slider::new(density).range(5.0, 200.0).step(1.0).label(labels.timeline_zoom.clone()).width(130.0),
        ))
        .spacing(4.0)
        .align(Align::Center)
        .padding((0.0, 12.0))
        .frame(None, Some(TIMELINE_BAR))
    }
}

impl View for MovieEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let _ = self.state.revision.get();
        let now = cx.now();
        self.state.advance(now);
        if self.state.inner.running.get().is_some() {
            cx.wake_at(now + FRAME);
        }
        let (undo, redo, delete) = (self.state.clone(), self.state.clone(), self.state.clone());
        cx.shortcut(Key::Character("z".into()), true, Rc::new(move || undo.undo()));
        cx.shortcut(Key::Character("y".into()), true, Rc::new(move || redo.redo()));
        cx.shortcut(Key::Delete, false, Rc::new(move || delete.delete_selected()));

        let viewport = self.area.unwrap_or_else(|| cx.viewport());
        let movie = self.state.movie();
        let logical = movie.logical_size();
        let timeline = timeline(&self.state, &self.labels);
        let timeline_height = timeline.total_height() + TIMELINE_BAR;
        let middle_height = (viewport.height - TOP_BAR - timeline_height).max(200.0);
        let by_width = (viewport.width - RAIL_WIDTH - LIBRARY_WIDTH - PROPERTIES_WIDTH - 48.0).max(240.0);
        let by_height =
            (middle_height - TRANSPORT - 16.0).max(120.0) * logical.width / logical.height.max(1.0);
        let canvas_width = by_width.min(by_height);

        let center: AnyView = if self.state.inner.trying.get() {
            Box::new(
                ScrollView::new((MoviePlayer::new(movie.clone())
                    .labels(self.player_labels.clone())
                    .width(by_width.max(480.0)),))
                .padding(8.0)
                .grow(1.0),
            )
        } else {
            Box::new(
                VStack::new((
                    Spacer::new(),
                    Canvas { state: self.state.clone(), width: canvas_width },
                    Spacer::new(),
                    self.transport(),
                ))
                .spacing(0.0)
                .align(Align::Center)
                .grow(1.0),
            )
        };
        let tree = VStack::new((
            self.top_bar(),
            Divider::horizontal(),
            HStack::new((self.rail(), self.library(), center, self.properties()))
                .spacing(0.0)
                .align(Align::Stretch)
                .grow(1.0),
            Divider::horizontal(),
            VStack::new((self.timeline_bar(), timeline))
                .spacing(0.0)
                .align(Align::Stretch)
                .background(Semantic::Surface),
        ))
        .spacing(0.0)
        .align(Align::Stretch)
        .grow(1.0);
        cx.child(&tree)
    }
}
