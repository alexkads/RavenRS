//! What the system says about light and dark, kept by the [`Ui`](crate::Ui) for the views that
//! offer to follow it.

use ravenrs_reactive::State;
use ravenrs_style::Theme;

/// What the system says about light and dark, and whether the theme follows it. Platforms tell
/// the [`Ui`](crate::Ui) whenever the system changes; while it is followed, the theme becomes
/// the dark or the light one of a pair to match: Raven Dark and Raven Light, or an application's
/// own, given with [`SystemAppearance::set_themes`]. Choosing a theme by hand stops following.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SystemAppearance {
    dark: State<Option<bool>>,
    follow: State<bool>,
    /// The light and the dark theme it follows with.
    pair: State<(Theme, Theme)>,
}

impl Default for SystemAppearance {
    fn default() -> Self {
        Self {
            dark: State::new(None),
            follow: State::new(false),
            pair: State::new((Theme::raven_light(), Theme::raven_dark())),
        }
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SystemAppearance {
    /// Whether the system is dark, when the platform knows.
    pub fn dark(&self) -> Option<bool> {
        self.dark.get()
    }

    /// Whether the theme follows the system.
    pub fn follows(&self) -> bool {
        self.follow.get()
    }

    /// The theme of RavenRS that matches the system: Raven Dark or Raven Light. What following
    /// puts on is [`SystemAppearance::theme_of`], which an application can change.
    pub fn theme_for(dark: bool) -> Theme {
        if dark { Theme::raven_dark() } else { Theme::raven_light() }
    }

    /// The theme following puts on when the system is dark, or light: the pair given to
    /// [`SystemAppearance::set_themes`], or Raven Dark and Raven Light.
    pub fn theme_of(&self, dark: bool) -> Theme {
        let (light, night) = *self.pair.peek();
        if dark { night } else { light }
    }

    /// Follows the system with these themes instead of Raven Light and Raven Dark, such as an
    /// application's own. While following, the one matching the system goes on at once.
    pub fn set_themes(&self, light: Theme, dark: Theme, theme: &State<Theme>) {
        self.pair.set((light, dark));
        if *self.follow.peek() {
            self.apply(theme);
        }
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
        let wanted = self.theme_of(dark);
        if theme.peek().name != wanted.name {
            theme.set(wanted);
        }
    }
}
