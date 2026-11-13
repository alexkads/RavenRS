//! What the system says about light and dark, kept by the [`Ui`](crate::Ui) for the views that
//! offer to follow it.

use ravenrs_reactive::State;
use ravenrs_style::Theme;

/// What the system says about light and dark, and whether the theme follows it. Platforms tell
/// the [`Ui`](crate::Ui) whenever the system changes; while it is followed, the theme becomes
/// Raven Dark or Raven Light to match. Choosing a theme by hand stops following.
#[derive(Clone)]
pub struct SystemAppearance {
    dark: State<Option<bool>>,
    follow: State<bool>,
}

impl Default for SystemAppearance {
    fn default() -> Self {
        Self { dark: State::new(None), follow: State::new(false) }
    }
}

impl std::fmt::Debug for SystemAppearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemAppearance")
            .field("dark", &*self.dark.peek())
            .field("follow", &*self.follow.peek())
            .finish()
    }
}

impl SystemAppearance {
    /// Whether the system is dark, when the platform knows.
    pub fn dark(&self) -> Option<bool> {
        self.dark.get()
    }

    /// Whether the theme follows the system.
    pub fn follows(&self) -> bool {
        self.follow.get()
    }

    /// The theme that matches the system: Raven Dark or Raven Light.
    pub fn theme_for(dark: bool) -> Theme {
        if dark { Theme::raven_dark() } else { Theme::raven_light() }
    }

    /// Starts or stops following the system. Starting puts the matching theme on at once,
    /// when the system is known.
    pub fn set_follow(&self, follow: bool, theme: &State<Theme>) {
        self.follow.set_if_changed(follow);
        if follow {
            self.apply(theme);
        }
    }

    /// What the platform says: the system is dark or not.
    pub(crate) fn report(&self, dark: bool, theme: &State<Theme>) {
        self.dark.set_if_changed(Some(dark));
        if *self.follow.peek() {
            self.apply(theme);
        }
    }

    fn apply(&self, theme: &State<Theme>) {
        let Some(dark) = *self.dark.peek() else { return };
        let wanted = Self::theme_for(dark);
        if theme.peek().name != wanted.name {
            theme.set(wanted);
        }
    }
}
