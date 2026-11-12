//! The accessibility of a RavenRS business application, so that every application made with
//! `ravenrs-app` reaches screen readers and the keyboard with nothing written for them.
//!
//! * [`AccessibleText`] says what a field or an action is called and what is said after it, from
//!   the model: the caption, unless the declaration or the model says otherwise.
//! * [`Announcer`] says what changed, such as a record kept or deleted, through a [`LiveRegion`].
//! * [`AccessWords`] are the words of those announcements, by key, in each language.
//! * [`Audit`] walks an application with no window and says what keeps someone out: a control
//!   with no name, focus that is held, contrast too low on its themes, two commands on the same
//!   keys. A test of any example calls it.
//!
//! ```
//! use ravenrs_app_a11y::{AccessibleText, Audit};
//! use ravenrs_app::{FieldInfo, FieldKind};
//!
//! let until = FieldInfo::new("until", FieldKind::Date).caption("Até").accessible_name("Fim do período");
//! assert_eq!(AccessibleText::of_field(&until, None, "Até").name, "Fim do período");
//!
//! let mut ui = ravenrs_widgets::Ui::new(ravenrs_widgets::Button::new("Salvar"), ravenrs_style::Theme::raven_light());
//! Audit::new().screen(&mut ui, "início").assert_clean();
//! ```

mod announce;
mod audit;
mod names;
pub mod words;

pub use announce::{Announcement, Announcer, LiveRegion, Urgency};
pub use audit::{
    Audit, Finding, Problem, Report, TEXT_CONTRAST, UI_CONTRAST, check_focus, check_shortcuts, check_theme,
    check_tree, is_control,
};
pub use names::AccessibleText;
pub use words::AccessWords;
