//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ComponentBook, Meta};

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    generated()
}

// rcb-scaffold: begin
mod attachment_strip;
mod audio_player_button;
mod chat;
#[cfg(feature = "code")]
mod code_editor;
mod floating_panel;
#[cfg(feature = "code")]
mod playground;
mod rich_text;
mod selection_bar;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
#[allow(
    unused_mut,
    clippy::vec_init_then_push,
    reason = "an entry may sit behind a feature, as its component does"
)]
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    all.push(<crate::AttachmentStrip as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::AudioPlayerButton as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Chat as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "code")]
    all.push(<crate::CodeEditor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FloatingPanel as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "code")]
    all.push(<crate::Playground as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::RichText as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::SelectionBar as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
