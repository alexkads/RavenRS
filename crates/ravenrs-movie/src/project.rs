//! The edits on a film: clips in order and overlays on tracks. Nothing here
//! touches the film, so any edit can be taken back.

use serde::{Deserialize, Serialize};

/// A clip is never cut shorter than this, in milliseconds.
const SHORTEST_MS: u64 = 100;

/// A rectangle on the film, in the logical pixels of the app.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Region {
    /// A region at this place and of this size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
}

/// What a clip shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClipKind {
    /// A stretch of the film.
    Film {
        /// Where it starts in the film, in milliseconds.
        from_ms: u64,
        /// Where it ends in the film, in milliseconds.
        to_ms: u64,
    },
    /// A card with a title, between stretches of the film.
    Title {
        /// The title.
        title: String,
        /// The line under it.
        subtitle: String,
        /// How long it stays, in milliseconds.
        duration_ms: u64,
    },
}

/// One piece of the main track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// What it shows.
    pub kind: ClipKind,
    /// 1 is real time, 2 is twice as fast. Cards ignore it.
    pub speed: f32,
}

impl Clip {
    /// How long it lasts on the timeline.
    pub fn duration_ms(&self) -> u64 {
        match &self.kind {
            ClipKind::Film { from_ms, to_ms } => {
                (to_ms.saturating_sub(*from_ms) as f64 / f64::from(self.speed.max(0.01))) as u64
            }
            ClipKind::Title { duration_ms, .. } => *duration_ms,
        }
    }
}

/// How a callout looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CalloutStyle {
    /// A light balloon with a colored edge.
    #[default]
    Balloon,
    /// A filled label in the accent color.
    Label,
}

/// A chapter added in the editor, at a moment of the timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    /// Its name.
    pub title: String,
    /// When it starts on the timeline, in milliseconds.
    pub at_ms: u64,
}

/// What an overlay does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OverlayKind {
    /// A balloon with text, placed on the film.
    Callout {
        /// What it says.
        text: String,
        /// Where it is.
        region: Region,
        /// How it looks.
        #[serde(default)]
        style: CalloutStyle,
        /// It comes and goes gradually.
        #[serde(default)]
        soft: bool,
    },
    /// A line of text at the bottom, as subtitles.
    Caption {
        /// What it says.
        text: String,
    },
    /// Darkens everything but this region.
    Spotlight {
        /// What stays lit.
        region: Region,
    },
    /// Covers this region, for what the film should not show.
    Hide {
        /// What gets covered.
        region: Region,
    },
    /// Zooms into this region and back out, sliding between them.
    Zoom {
        /// What fills the screen.
        region: Region,
    },
    /// Comes in from black.
    FadeIn,
    /// Goes out to black.
    FadeOut,
    /// A soft circle follows the pointer, and clicks show bigger.
    PointerHighlight,
    /// A spot that talks to whoever is watching.
    Hotspot {
        /// Where it is.
        region: Region,
        /// In a few words.
        title: String,
        /// What to say.
        text: String,
        /// The film waits here for a click on the spot.
        stop: bool,
    },
}

/// Something over the film for a while, on a track of the timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    /// Its identity, which edits do not change.
    pub id: u64,
    /// Its track, from 0.
    pub track: u8,
    /// When it starts on the timeline, in milliseconds.
    pub start_ms: u64,
    /// How long it lasts.
    pub duration_ms: u64,
    /// What it does.
    pub kind: OverlayKind,
}

impl Overlay {
    /// When it ends on the timeline.
    pub fn end_ms(&self) -> u64 {
        self.start_ms + self.duration_ms
    }

    /// It is on screen at `ms`.
    pub fn covers(&self, ms: u64) -> bool {
        self.start_ms <= ms && ms < self.end_ms()
    }
}

/// What the timeline shows at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// This moment of the film.
    Film(u64),
    /// The card that is this clip.
    Title(usize),
}

/// A film's edits.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Project {
    /// The main track, in order.
    pub clips: Vec<Clip>,
    /// What goes over it.
    pub overlays: Vec<Overlay>,
    /// The chapters added in the editor.
    #[serde(default)]
    pub markers: Vec<Marker>,
    next_id: u64,
}

impl Project {
    /// No edits: the whole film of this duration, as it is.
    pub fn whole(duration_ms: u64) -> Self {
        Self {
            clips: vec![Clip { kind: ClipKind::Film { from_ms: 0, to_ms: duration_ms }, speed: 1.0 }],
            overlays: Vec::new(),
            markers: Vec::new(),
            next_id: 1,
        }
    }

    /// How long it all lasts.
    pub fn duration_ms(&self) -> u64 {
        self.clips.iter().map(Clip::duration_ms).sum()
    }

    /// When a clip starts on the timeline.
    pub fn clip_start_ms(&self, clip: usize) -> u64 {
        self.clips.iter().take(clip).map(Clip::duration_ms).sum()
    }

    /// The clip on the timeline at `ms`. The very end belongs to the last clip.
    pub fn clip_at(&self, ms: u64) -> Option<usize> {
        let mut start = 0;
        for (i, clip) in self.clips.iter().enumerate() {
            start += clip.duration_ms();
            if ms < start {
                return Some(i);
            }
        }
        self.clips.len().checked_sub(1)
    }

    /// What the timeline shows at `ms`.
    pub fn place_at(&self, ms: u64) -> Option<Place> {
        let index = self.clip_at(ms)?;
        let clip = &self.clips[index];
        let inside = ms.saturating_sub(self.clip_start_ms(index)).min(clip.duration_ms());
        Some(match &clip.kind {
            ClipKind::Film { from_ms, to_ms } => {
                Place::Film((*from_ms + (inside as f64 * f64::from(clip.speed)) as u64).min(*to_ms))
            }
            ClipKind::Title { .. } => Place::Title(index),
        })
    }

    /// Where a moment of the film ended up on the timeline; `None` if it was cut.
    pub fn film_to_timeline(&self, film_ms: u64) -> Option<u64> {
        self.clips.iter().enumerate().find_map(|(i, clip)| match clip.kind {
            ClipKind::Film { from_ms, to_ms } if from_ms <= film_ms && film_ms < to_ms.max(from_ms + 1) => {
                let inside = (film_ms - from_ms) as f64 / f64::from(clip.speed.max(0.01));
                Some(self.clip_start_ms(i) + inside as u64)
            }
            _ => None,
        })
    }

    /// Cuts the clip under `ms` in two there. Overlays stay where they are.
    /// Returns false when `ms` is too near a clip's edge to leave two pieces.
    pub fn split(&mut self, ms: u64) -> bool {
        let Some(index) = self.clip_at(ms) else {
            return false;
        };
        let start = self.clip_start_ms(index);
        let inside = ms.saturating_sub(start);
        let clip = self.clips[index].clone();
        if inside < SHORTEST_MS || clip.duration_ms().saturating_sub(inside) < SHORTEST_MS {
            return false;
        }
        let ClipKind::Film { from_ms, to_ms } = clip.kind else {
            return false;
        };
        let middle = from_ms + (inside as f64 * f64::from(clip.speed)) as u64;
        self.clips[index].kind = ClipKind::Film { from_ms, to_ms: middle };
        self.clips
            .insert(index + 1, Clip { kind: ClipKind::Film { from_ms: middle, to_ms }, speed: clip.speed });
        true
    }

    /// Takes a clip out and closes the gap: overlays after it come along, those
    /// over it get shorter, and those only over it go.
    pub fn delete_clip(&mut self, index: usize) {
        if index >= self.clips.len() || self.clips.len() == 1 {
            return;
        }
        let start = self.clip_start_ms(index);
        let length = self.clips[index].duration_ms();
        self.clips.remove(index);
        self.close_gap(start, length);
    }

    fn close_gap(&mut self, start: u64, length: u64) {
        let end = start + length;
        self.overlays.retain_mut(|overlay| {
            let (from, to) = (overlay.start_ms, overlay.end_ms());
            if from >= start && to <= end {
                return false;
            }
            let moved = |at: u64| if at <= start { at } else { at.saturating_sub(length).max(start) };
            overlay.start_ms = moved(from);
            overlay.duration_ms = moved(to) - overlay.start_ms;
            overlay.duration_ms >= SHORTEST_MS
        });
    }

    fn open_gap(&mut self, at: u64, length: u64) {
        for overlay in &mut self.overlays {
            if overlay.start_ms >= at {
                overlay.start_ms += length;
            }
        }
    }

    /// Changes how fast a clip plays, which moves what comes after it.
    pub fn set_speed(&mut self, index: usize, speed: f32) {
        let Some(old) = self.clips.get(index).map(Clip::duration_ms) else {
            return;
        };
        self.clips[index].speed = speed.clamp(0.25, 8.0);
        let new = self.clips[index].duration_ms();
        let end = self.clip_start_ms(index) + old;
        for overlay in &mut self.overlays {
            if overlay.start_ms >= end {
                overlay.start_ms = (overlay.start_ms + new).saturating_sub(old);
            }
        }
    }

    /// Moves the start of a film clip by `ms` of the timeline. What comes after follows.
    pub fn trim_start(&mut self, index: usize, ms: i64) {
        self.trim(index, ms, true);
    }

    /// Moves the end of a film clip by `ms` of the timeline. What comes after follows.
    pub fn trim_end(&mut self, index: usize, ms: i64) {
        self.trim(index, ms, false);
    }

    fn trim(&mut self, index: usize, ms: i64, at_start: bool) {
        let Some(clip) = self.clips.get(index).cloned() else {
            return;
        };
        let ClipKind::Film { from_ms, to_ms } = clip.kind else {
            return;
        };
        let old = clip.duration_ms();
        let film = (ms as f64 * f64::from(clip.speed)) as i64;
        let shortest = SHORTEST_MS as i64;
        let (new_from, new_to) = if at_start {
            ((from_ms as i64 + film).clamp(0, to_ms as i64 - shortest) as u64, to_ms)
        } else {
            (from_ms, (to_ms as i64 + film).max(from_ms as i64 + shortest) as u64)
        };
        self.clips[index].kind = ClipKind::Film { from_ms: new_from, to_ms: new_to };
        let new = self.clips[index].duration_ms();
        let end = self.clip_start_ms(index) + old;
        for overlay in &mut self.overlays {
            if overlay.start_ms >= end {
                overlay.start_ms = (overlay.start_ms + new).saturating_sub(old);
            }
        }
    }

    /// Puts a card in at `ms`, cutting the clip there if needed. What is after it moves on.
    pub fn insert_title(&mut self, ms: u64, title: &str, subtitle: &str, duration_ms: u64) -> usize {
        self.split(ms);
        let mut index = self.clips.len();
        let mut start = 0;
        for (i, clip) in self.clips.iter().enumerate() {
            if start >= ms {
                index = i;
                break;
            }
            start += clip.duration_ms();
        }
        let at = self.clip_start_ms(index);
        self.open_gap(at, duration_ms);
        let kind = ClipKind::Title { title: title.to_owned(), subtitle: subtitle.to_owned(), duration_ms };
        self.clips.insert(index, Clip { kind, speed: 1.0 });
        index
    }

    /// Adds an overlay on the first track free for its time, and says which it is.
    pub fn add_overlay(&mut self, kind: OverlayKind, start_ms: u64, duration_ms: u64) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        let overlaps = |o: &Overlay| start_ms < o.end_ms() && o.start_ms < start_ms + duration_ms;
        let track = (0..u8::MAX)
            .find(|track| !self.overlays.iter().any(|o| o.track == *track && overlaps(o)))
            .unwrap_or(0);
        self.overlays.push(Overlay { id, track, start_ms, duration_ms, kind });
        id
    }

    /// An overlay by its identity.
    pub fn overlay_mut(&mut self, id: u64) -> Option<&mut Overlay> {
        self.overlays.iter_mut().find(|overlay| overlay.id == id)
    }

    /// Takes an overlay out.
    pub fn remove_overlay(&mut self, id: u64) {
        self.overlays.retain(|overlay| overlay.id != id);
    }

    /// How many tracks of overlays there are.
    pub fn track_count(&self) -> usize {
        self.overlays.iter().map(|o| usize::from(o.track) + 1).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caption() -> OverlayKind {
        OverlayKind::Caption { text: "x".to_owned() }
    }

    #[test]
    fn untouched_the_timeline_is_the_film() {
        let project = Project::whole(10_000);
        assert_eq!(project.duration_ms(), 10_000);
        assert_eq!(project.place_at(4_000), Some(Place::Film(4_000)));
        assert_eq!(project.place_at(10_000), Some(Place::Film(10_000)));
        assert_eq!(project.film_to_timeline(4_000), Some(4_000));
    }

    #[test]
    fn cutting_out_the_middle_joins_the_rest() {
        let mut project = Project::whole(10_000);
        assert!(project.split(2_000));
        assert!(project.split(5_000));
        assert_eq!(project.clips.len(), 3);
        project.delete_clip(1);
        assert_eq!(project.duration_ms(), 7_000);
        assert_eq!(project.place_at(1_999), Some(Place::Film(1_999)));
        assert_eq!(project.place_at(2_000), Some(Place::Film(5_000)));
        assert_eq!(project.film_to_timeline(3_000), None, "it was cut");
        assert_eq!(project.film_to_timeline(6_000), Some(3_000));
    }

    #[test]
    fn a_cut_too_near_an_edge_does_nothing_and_the_last_clip_stays() {
        let mut project = Project::whole(1_000);
        assert!(!project.split(10));
        assert!(!project.split(995));
        project.delete_clip(0);
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.duration_ms(), 1_000);
    }

    #[test]
    fn deleting_carries_the_overlays_along() {
        let mut project = Project::whole(10_000);
        let before = project.add_overlay(caption(), 500, 1_000);
        let across = project.add_overlay(caption(), 1_500, 2_000);
        let over = project.add_overlay(caption(), 2_500, 500);
        let after = project.add_overlay(caption(), 6_000, 1_000);
        project.split(2_000);
        project.split(5_000);
        project.delete_clip(1);
        let start = |project: &mut Project, id| project.overlay_mut(id).map(|o| (o.start_ms, o.duration_ms));
        assert_eq!(start(&mut project, before), Some((500, 1_000)));
        assert_eq!(start(&mut project, across), Some((1_500, 500)), "only what was left of it");
        assert_eq!(start(&mut project, over), None, "it was only over what went");
        assert_eq!(start(&mut project, after), Some((3_000, 1_000)));
    }

    #[test]
    fn speed_changes_the_length_and_what_is_shown() {
        let mut project = Project::whole(10_000);
        project.split(4_000);
        let later = project.add_overlay(caption(), 5_000, 500);
        project.set_speed(0, 2.0);
        assert_eq!(project.duration_ms(), 8_000);
        assert_eq!(project.place_at(1_000), Some(Place::Film(2_000)));
        assert_eq!(project.place_at(2_500), Some(Place::Film(4_500)));
        assert_eq!(project.film_to_timeline(2_000), Some(1_000));
        assert_eq!(project.overlay_mut(later).map(|o| o.start_ms), Some(3_000));
    }

    #[test]
    fn trimming_moves_what_follows_and_stops_at_the_film() {
        let mut project = Project::whole(10_000);
        project.split(5_000);
        project.trim_end(0, -1_000);
        assert_eq!(project.duration_ms(), 9_000);
        assert_eq!(project.place_at(4_000), Some(Place::Film(5_000)));
        project.trim_start(0, 1_000);
        assert_eq!(project.place_at(0), Some(Place::Film(1_000)));
        project.trim_start(0, -99_000);
        assert_eq!(project.place_at(0), Some(Place::Film(0)), "not before the film starts");
        project.trim_end(0, -99_000);
        assert_eq!(project.clips[0].duration_ms(), SHORTEST_MS, "nor shorter than a sliver");
    }

    #[test]
    fn a_card_opens_a_gap_in_the_timeline() {
        let mut project = Project::whole(10_000);
        let later = project.add_overlay(caption(), 6_000, 500);
        let index = project.insert_title(4_000, "Part two", "", 3_000);
        assert_eq!(index, 1);
        assert_eq!(project.duration_ms(), 13_000);
        assert_eq!(project.place_at(5_000), Some(Place::Title(1)));
        assert_eq!(project.place_at(7_000), Some(Place::Film(4_000)));
        assert_eq!(project.overlay_mut(later).map(|o| o.start_ms), Some(9_000));

        let first = project.insert_title(0, "Welcome", "", 1_000);
        assert_eq!(first, 0, "at the very start");
    }

    #[test]
    fn overlays_find_a_free_track_and_a_project_survives_a_file() {
        let mut project = Project::whole(10_000);
        project.add_overlay(caption(), 0, 2_000);
        project.add_overlay(OverlayKind::FadeIn, 1_000, 2_000);
        project.add_overlay(OverlayKind::PointerHighlight, 2_000, 1_000);
        assert_eq!(project.overlays.iter().map(|o| o.track).collect::<Vec<_>>(), [0, 1, 0]);
        assert_eq!(project.track_count(), 2);
        let text = serde_json::to_string(&project).expect("written");
        assert_eq!(serde_json::from_str::<Project>(&text).expect("read"), project);
    }
}
