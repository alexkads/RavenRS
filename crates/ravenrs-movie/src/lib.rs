#![doc = include_str!("../README.md")]

mod assistant;
mod chat;
mod composition;
mod editor;
mod errors;
mod flight;
mod movie;
mod narration;
mod player;
mod project;
mod record;
mod search;
mod social;
mod soundtrack;

pub use assistant::{AssistantLabels, MovieAssistant};
pub use chat::{MovieAnswers, MovieChat, MovieChatLabels, MovieWriter};
pub use composition::Composition;
pub use editor::{EditorLabels, EditorState, MovieEditor};
pub use errors::RecentErrors;
pub use flight::{BugLabels, BugReport, BugReporter, FlightRecorder};
pub use movie::{Chapter, FilmBuilder, Hotspot, HotspotKind, Movie, MovieError, PointerKey};
pub use narration::Narration;
pub use player::{MovieCue, MovieFrame, MovieLabels, MoviePlayer};
pub use project::{CalloutStyle, Clip, ClipKind, Marker, Overlay, OverlayKind, Place, Project, Region};
pub use record::{RecordError, Recorder};
pub use search::{Answer, Moment, MomentKind, MovieLibrary};
pub use social::SocialFrame;
pub use soundtrack::Soundtrack;
