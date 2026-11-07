//! Telling screen readers what changed, without moving the keyboard.
//!
//! An [`Announcer`] keeps the latest announcement; its [`LiveRegion`] shows it to assistive
//! technology as a status, which waits for a pause, or as an alert, which interrupts. Nothing of
//! it shows on the screen. Each announcement is a new node, so the same words said twice are
//! heard twice, and the last few stay, so one said right after another does not take its place.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::Role;
use ravenrs_layout::LayoutStyle;
use ravenrs_reactive::State;
use ravenrs_widgets::{BuildCx, Keyed, NodeId, Paint, Semantics, View};

/// How many announcements the region keeps, newest last.
const KEPT: usize = 3;

/// How soon an announcement is heard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Urgency {
    /// When the screen reader is done with what it is saying: something that went as asked.
    #[default]
    Polite,
    /// At once: something that went wrong, or that cannot wait.
    Assertive,
}

/// One thing said to screen readers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Announcement {
    /// The words.
    pub text: String,
    /// How soon.
    pub urgency: Urgency,
    /// Which one it is, counting from the first.
    pub serial: u64,
}

/// Where an application says what changed. A handle: clones say it in the same place.
///
/// ```
/// use ravenrs_app_a11y::{Announcer, Urgency};
/// use ravenrs_core::Role;
/// use ravenrs_widgets::Ui;
///
/// let announcer = Announcer::new();
/// let mut ui = Ui::new(announcer.region(), ravenrs_style::Theme::raven_dark());
/// announcer.polite("Salvo: Conta corrente");
/// ui.update();
/// assert!(ui.access_tree().find(Role::Status, "Salvo: Conta corrente").is_some());
/// announcer.assertive("Não foi salvo");
/// ui.update();
/// assert!(ui.access_tree().find(Role::Alert, "Não foi salvo").is_some());
/// assert_eq!(announcer.latest().map(|a| a.urgency), Some(Urgency::Assertive));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Announcer {
    recent: State<Vec<Announcement>>,
    count: Rc<Cell<u64>>,
    quiet: Rc<Cell<bool>>,
}

impl Announcer {
    /// With nothing said yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Says `text` when the screen reader pauses.
    pub fn polite(&self, text: impl Into<String>) {
        self.say(text.into(), Urgency::Polite);
    }

    /// Says `text` at once.
    pub fn assertive(&self, text: impl Into<String>) {
        self.say(text.into(), Urgency::Assertive);
    }

    /// Says `text` this soon. Empty words say nothing.
    pub fn say(&self, text: String, urgency: Urgency) {
        if self.quiet.get() || text.trim().is_empty() {
            return;
        }
        let serial = self.count.get() + 1;
        self.count.set(serial);
        self.recent.update(|recent| {
            recent.push(Announcement { text, urgency, serial });
            let extra = recent.len().saturating_sub(KEPT);
            recent.drain(..extra);
        });
    }

    /// Stops saying anything, or starts again, for an application that announces in its own way.
    pub fn set_quiet(&self, quiet: bool) {
        self.quiet.set(quiet);
    }

    /// Whether it says nothing.
    pub fn is_quiet(&self) -> bool {
        self.quiet.get()
    }

    /// The latest thing said.
    pub fn latest(&self) -> Option<Announcement> {
        self.recent.peek().last().cloned()
    }

    /// The last few things said, newest last.
    pub fn recent(&self) -> Vec<Announcement> {
        self.recent.peek().clone()
    }

    /// Where screen readers hear it. It takes no room.
    pub fn region(&self) -> LiveRegion {
        LiveRegion { recent: self.recent.clone() }
    }
}

/// What an [`Announcer`] says, as a node of the accessibility tree and nothing on the screen.
#[derive(Clone, Debug)]
pub struct LiveRegion {
    recent: State<Vec<Announcement>>,
}

/// The node of one announcement.
struct Said(Announcement);

impl View for Said {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.container(
            LayoutStyle { width: Some(0.0), height: Some(0.0), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        );
        let role = match self.0.urgency {
            Urgency::Polite => Role::Status,
            Urgency::Assertive => Role::Alert,
        };
        cx.semantics(node, Semantics::new(role).name(self.0.text.clone()));
        node
    }
}

impl View for LiveRegion {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let said: Vec<NodeId> = self
            .recent
            .get()
            .into_iter()
            .map(|said| cx.child(&Keyed::new(said.serial, Said(said))))
            .collect();
        cx.container(
            LayoutStyle { width: Some(0.0), height: Some(0.0), ..LayoutStyle::leaf() },
            &said,
            Paint::None,
        )
    }
}
