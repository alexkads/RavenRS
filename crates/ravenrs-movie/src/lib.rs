#![doc = include_str!("../README.md")]

mod assistant;
mod composition;
mod editor;
mod flight;
mod movie;
mod player;
mod project;
mod record;
mod search;

pub use assistant::{AssistantLabels, MovieAssistant};
pub use composition::Composition;
pub use editor::{EditorLabels, EditorState, MovieEditor};
pub use flight::{BugLabels, BugReport, BugReporter, FlightRecorder};
pub use movie::{Chapter, FilmBuilder, Hotspot, HotspotKind, Movie, MovieError, PointerKey};
pub use player::{MovieCue, MovieLabels, MoviePlayer};
pub use project::{CalloutStyle, Clip, ClipKind, Marker, Overlay, OverlayKind, Place, Project, Region};
pub use record::{RecordError, Recorder};
pub use search::{Answer, Moment, MomentKind, MovieLibrary};
