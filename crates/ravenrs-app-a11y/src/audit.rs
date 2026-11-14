//! An audit that walks an application the way a keyboard and a screen reader would, and says
//! what keeps someone out.
//!
//! [`Audit::app`] opens every item of the navigation, and on each screen the buttons it was told
//! to press, such as the one that starts a new record. [`Audit::items`] and [`Audit::item`] do the
//! same one item at a time, so a test can audit each screen in a test of its own and the screens
//! run in parallel. On each one it looks for:
//!
//! * a control screen readers cannot name, and an id that appears twice;
//! * keyboard focus on a node that is not there;
//! * focus held: Tab stays where it is, or goes round some of the controls and never reaches the
//!   others;
//! * two commands on the same keys.
//!
//! It also checks the contrast of text and of the focus ring on every theme it is given, and on
//! the one on screen. It runs with no window, so a test of any example can call it.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use ravenrs_core::{AccessAction, AccessTree, Color, Event, Key, Modifiers, Rect, Role};
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::Ui;

/// The least contrast for text, as WCAG asks of normal text (AA).
pub const TEXT_CONTRAST: f32 = 4.5;
/// The least contrast for what is not text but has to be seen, such as the focus ring (AA).
pub const UI_CONTRAST: f32 = 3.0;
/// The most Tab presses tried on one screen.
const MOST_TABS: usize = 400;

/// What keeps someone out.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Problem {
    /// A control with no name: a screen reader says only what it is.
    Unnamed {
        /// What it is.
        role: Role,
        /// Where it is.
        bounds: Rect,
    },
    /// Two nodes with the same id, which assistive technology takes for one.
    RepeatedId {
        /// The id.
        id: u64,
    },
    /// Keyboard focus is on a node the tree does not have.
    FocusOnNothing,
    /// Tab does not move the focus.
    FocusStuck {
        /// What it stays on.
        on: Option<String>,
    },
    /// Tab goes round some of the controls and never reaches the others.
    FocusTrapped {
        /// How many it reaches.
        reached: usize,
        /// How many there are.
        of: usize,
        /// The first one it never reaches.
        missed: Option<String>,
    },
    /// Two colors of a theme are too close to read one on the other.
    LowContrast {
        /// The theme.
        theme: String,
        /// The color in front and the one behind, by their tokens.
        pair: String,
        /// Their contrast.
        ratio: f32,
        /// The least it should be.
        least: f32,
    },
    /// A window over the screen that Escape does not close, so the keyboard cannot leave it.
    WindowHeld {
        /// What the window is called.
        name: Option<String>,
    },
    /// More than one command answers to the same keys.
    SharedShortcut {
        /// The keys, as they are written next to a command.
        keys: String,
        /// How many commands.
        times: usize,
    },
    /// The navigation has no item with this name to open, or it cannot be clicked.
    NoSuchItem {
        /// The name asked for.
        name: String,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unnamed { role, bounds } => write!(f, "a {role:?} has no name, at {bounds:?}"),
            Self::RepeatedId { id } => write!(f, "the id {id} appears more than once"),
            Self::FocusOnNothing => f.write_str("the focus is on a node the tree does not have"),
            Self::FocusStuck { on } => {
                write!(
                    f,
                    "Tab does not move the focus from {}",
                    on.as_deref().unwrap_or("an unnamed control")
                )
            }
            Self::FocusTrapped { reached, of, missed } => write!(
                f,
                "Tab goes round {reached} of {of} controls and never reaches {}",
                missed.as_deref().unwrap_or("the others")
            ),
            Self::LowContrast { theme, pair, ratio, least } => {
                write!(f, "{theme}: {pair} is {ratio:.2}:1, under {least}:1")
            }
            Self::WindowHeld { name } => {
                write!(f, "Escape does not close the window {}", name.as_deref().unwrap_or("with no name"))
            }
            Self::SharedShortcut { keys, times } => write!(f, "{times} commands answer to {keys}"),
            Self::NoSuchItem { name } => write!(f, "the navigation has no item {name} to open"),
        }
    }
}

/// A problem, and where it was found.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Finding {
    /// The screen, as the navigation calls it, or the theme.
    pub place: String,
    /// What is wrong.
    pub problem: Problem,
}

impl Finding {
    /// This problem, here.
    pub fn new(place: impl Into<String>, problem: Problem) -> Self {
        Self { place: place.into(), problem }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.place, self.problem)
    }
}

/// What an audit found, and where it looked.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Report {
    /// What is wrong.
    pub findings: Vec<Finding>,
    /// The screens it went through.
    pub places: Vec<String>,
}

impl Report {
    /// Nothing is wrong.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Fails, saying everything found, unless nothing was.
    ///
    /// # Panics
    ///
    /// When anything was found.
    #[track_caller]
    pub fn assert_clean(&self) {
        assert!(self.is_clean(), "{self}");
    }

    fn add(&mut self, found: Vec<Finding>) {
        for finding in found {
            if !self.findings.contains(&finding) {
                self.findings.push(finding);
            }
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "accessibility: {} problems in {} screens", self.findings.len(), self.places.len())?;
        for finding in &self.findings {
            writeln!(f, "- {finding}")?;
        }
        Ok(())
    }
}

/// Roles that are operated, so they must say what they are.
pub fn is_control(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::Slider
            | Role::SpinButton
            | Role::ComboBox
            | Role::Tab
            | Role::MenuItem
            | Role::ListBoxOption
            | Role::TreeItem
    )
}

/// What is wrong with the names and ids of a tree.
pub fn check_tree(tree: &AccessTree, place: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut ids = HashSet::new();
    for node in &tree.nodes {
        if !ids.insert(node.id) {
            found.push(Finding::new(place, Problem::RepeatedId { id: node.id }));
        }
        let named = node.name.as_deref().is_some_and(|name| !name.trim().is_empty());
        if is_control(node.role) && !named {
            found.push(Finding::new(place, Problem::Unnamed { role: node.role, bounds: node.bounds }));
        }
    }
    if tree.focus.is_some_and(|focus| tree.get(focus).is_none()) {
        found.push(Finding::new(place, Problem::FocusOnNothing));
    }
    found
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key: key.clone(), modifiers });
    ui.handle_event(&Event::KeyUp { key, modifiers });
    ui.update();
}

/// The windows over the screen, by id.
fn windows(tree: &AccessTree) -> Vec<u64> {
    tree.nodes.iter().filter(|n| n.modal || n.role == Role::Dialog).map(|n| n.id).collect()
}

/// Presses Tab across the screen and says whether the focus moves and reaches every control.
/// The focus is left where the last press put it.
///
/// What the application changes by itself on the way, such as a reminder that comes due, is not
/// held against the screen: a control counts as never reached only when it is there from the
/// first press to the last, and a window that opens on the way ends the walk.
pub fn check_focus(ui: &mut Ui, place: &str) -> Vec<Finding> {
    let tree = ui.access_tree();
    let names: BTreeMap<u64, Option<String>> =
        tree.nodes.iter().filter(|n| n.focusable).map(|n| (n.id, n.name.clone())).collect();
    if names.len() < 2 {
        return Vec::new();
    }
    let opened = windows(&tree);
    let mut seen: Vec<u64> = Vec::new();
    let mut closed = false;
    let mut before = tree.focus;
    let mut last = tree;
    for _ in 0..(names.len() + 2).min(MOST_TABS) {
        press(ui, Key::Tab, Modifiers::default());
        let now = ui.access_tree();
        if windows(&now).iter().any(|w| !opened.contains(w)) {
            return Vec::new();
        }
        let Some(focus) = now.focus else { break };
        if before == Some(focus) {
            let on = now.get(focus).and_then(|n| n.name.clone());
            return vec![Finding::new(place, Problem::FocusStuck { on })];
        }
        before = Some(focus);
        last = now;
        if seen.contains(&focus) {
            closed = true;
            break;
        }
        seen.push(focus);
    }
    let stayed: Vec<(&u64, &Option<String>)> =
        names.iter().filter(|(id, _)| last.get(**id).is_some_and(|n| n.focusable)).collect();
    let reached = stayed.iter().filter(|(id, _)| seen.contains(id)).count();
    if closed && reached < stayed.len() {
        let missed = stayed.iter().find(|(id, _)| !seen.contains(id)).and_then(|(_, name)| (*name).clone());
        return vec![Finding::new(place, Problem::FocusTrapped { reached, of: stayed.len(), missed })];
    }
    Vec::new()
}

/// Closes with Escape what stays over the screen, and says so when Escape does not.
fn close_windows(ui: &mut Ui, place: &str) -> Vec<Finding> {
    for _ in 0..4 {
        if windows(&ui.access_tree()).is_empty() {
            return Vec::new();
        }
        press(ui, Key::Escape, Modifiers::default());
        settle(ui);
    }
    let tree = ui.access_tree();
    let held = tree.nodes.iter().find(|n| n.modal || n.role == Role::Dialog);
    held.map(|n| vec![Finding::new(place, Problem::WindowHeld { name: n.name.clone() })]).unwrap_or_default()
}

/// Commands of the latest frame that answer to the same keys.
pub fn check_shortcuts(ui: &Ui, place: &str) -> Vec<Finding> {
    let mut counted: Vec<(ravenrs_widgets::Hotkey, usize)> = Vec::new();
    for keys in ui.shortcuts() {
        match counted.iter_mut().find(|(known, _)| *known == keys) {
            Some((_, times)) => *times += 1,
            None => counted.push((keys, 1)),
        }
    }
    counted
        .into_iter()
        .filter(|(_, times)| *times > 1)
        .map(|(keys, times)| Finding::new(place, Problem::SharedShortcut { keys: keys.label(), times }))
        .collect()
}

/// A color with some transparency as it shows over `ground`.
fn over(color: Color, ground: Color) -> Color {
    ground.lerp(color.with_alpha(1.0), color.a)
}

/// The pairs of tokens a theme has to keep apart: text on what it sits on, and the focus ring on
/// the window.
const PAIRS: &[(Semantic, Semantic, f32)] = &[
    (Semantic::Ink, Semantic::Canvas, TEXT_CONTRAST),
    (Semantic::Ink, Semantic::Surface, TEXT_CONTRAST),
    (Semantic::Ink, Semantic::SurfaceElevated, TEXT_CONTRAST),
    (Semantic::InkMuted, Semantic::Canvas, TEXT_CONTRAST),
    (Semantic::InkMuted, Semantic::Surface, TEXT_CONTRAST),
    (Semantic::OnAccent, Semantic::Accent, TEXT_CONTRAST),
    (Semantic::FocusRing, Semantic::Canvas, UI_CONTRAST),
    (Semantic::FocusRing, Semantic::Surface, UI_CONTRAST),
];

/// The pairs of colors of `theme` that are too close to read.
pub fn check_theme(theme: &Theme) -> Vec<Finding> {
    let canvas = theme.color(Semantic::Canvas).with_alpha(1.0);
    PAIRS
        .iter()
        .filter_map(|&(front, back, least)| {
            let ground = over(theme.color(back), canvas);
            let ratio = over(theme.color(front), ground).contrast_ratio(ground);
            (ratio < least).then(|| {
                Finding::new(
                    theme.name,
                    Problem::LowContrast {
                        theme: theme.name.to_owned(),
                        pair: format!("{front:?} on {back:?}"),
                        ratio,
                        least,
                    },
                )
            })
        })
        .collect()
}

/// The items of the navigation landmarks of a tree: what opens a screen.
fn navigation_items(tree: &AccessTree) -> Vec<(Role, String)> {
    let mut items = Vec::new();
    let mut stack: Vec<u64> =
        tree.nodes.iter().filter(|n| n.role == Role::Navigation).map(|n| n.id).collect();
    let mut visited = HashSet::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(node) = tree.get(id) else { continue };
        let opens = matches!(node.role, Role::Button | Role::Link | Role::TreeItem)
            && node.actions.contains(&AccessAction::Click)
            && node.expanded.is_none()
            && !node.disabled;
        if opens && let Some(name) = node.name.clone().filter(|n| !n.trim().is_empty()) {
            let item = (node.role, name);
            if !items.contains(&item) {
                items.push(item);
            }
        }
        stack.extend(node.children.iter().rev());
    }
    // In reading order: the stack took children last first.
    items.sort_by_key(|(role, name)| {
        tree.nodes.iter().position(|n| n.role == *role && n.name.as_deref() == Some(name.as_str()))
    });
    items
}

fn settle(ui: &mut Ui) {
    for _ in 0..3 {
        ui.update();
    }
}

/// How to walk an application, and what to look at.
///
/// ```
/// use ravenrs_app_a11y::Audit;
/// use ravenrs_widgets::{Button, Ui};
///
/// let mut ui = Ui::new(Button::new("Salvar"), ravenrs_style::Theme::raven_dark());
/// ui.update();
/// Audit::new().screen(&mut ui, "início").assert_clean();
/// ```
#[derive(Clone, Debug, Default)]
pub struct Audit {
    themes: Vec<Theme>,
    open: Vec<String>,
    skip: Vec<String>,
    focus: bool,
}

impl Audit {
    /// Looks at names, ids, focus, shortcuts and the theme on screen.
    pub fn new() -> Self {
        Self { focus: true, ..Self::default() }
    }

    /// Also checks the contrast of this theme, which the application offers.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.themes.push(theme);
        self
    }

    /// On every screen, also presses the button with this name when there is one, as `New`, and
    /// looks at what it opens.
    pub fn open(mut self, button: impl Into<String>) -> Self {
        self.open.push(button.into());
        self
    }

    /// Leaves out the item of the navigation with this name, as one that leaves the application.
    pub fn skip(mut self, item: impl Into<String>) -> Self {
        self.skip.push(item.into());
        self
    }

    /// Does not press Tab, for a screen that changes as the focus moves.
    pub fn without_focus(mut self) -> Self {
        self.focus = false;
        self
    }

    /// Looks at the screen as it is.
    pub fn screen(&self, ui: &mut Ui, place: &str) -> Report {
        let mut report = Report::default();
        self.look(ui, place, &mut report);
        report.add(self.contrast(ui));
        report
    }

    fn contrast(&self, ui: &Ui) -> Vec<Finding> {
        let shown = ui.theme();
        let mut found = check_theme(&shown);
        for theme in self.themes.iter().filter(|t| t.name != shown.name) {
            found.extend(check_theme(theme));
        }
        found
    }

    fn look(&self, ui: &mut Ui, place: &str, report: &mut Report) {
        settle(ui);
        report.places.push(place.to_owned());
        report.add(check_tree(&ui.access_tree(), place));
        report.add(check_shortcuts(ui, place));
        if self.focus {
            report.add(check_focus(ui, place));
        }
    }

    /// Walks the whole application: the screen it opens on, every item of its navigation, and on
    /// each, the buttons given to [`open`](Self::open).
    pub fn app(&self, ui: &mut Ui) -> Report {
        let mut report = Report::default();
        report.add(self.contrast(ui));
        settle(ui);
        let items = navigation_items(&ui.access_tree());
        self.look(ui, "start", &mut report);
        report.add(close_windows(ui, "start"));
        for (role, name) in items.iter().filter(|(_, name)| !self.skip.contains(name)) {
            if click(ui, *role, name) {
                self.visit(ui, name, &mut report);
            }
        }
        report
    }

    /// The items of the navigation on screen now, in reading order, without the ones given to
    /// [`skip`](Self::skip): what [`app`](Self::app) would open.
    pub fn items(&self, ui: &mut Ui) -> Vec<String> {
        settle(ui);
        navigation_items(&ui.access_tree())
            .into_iter()
            .map(|(_, name)| name)
            .filter(|name| !self.skip.contains(name))
            .collect()
    }

    /// Opens one item of the navigation, as [`app`](Self::app) does, and looks at it and at what
    /// the buttons given to [`open`](Self::open) open from it. An item the navigation does not have
    /// is a finding, so a test of a screen that went away fails with its name.
    pub fn item(&self, ui: &mut Ui, name: &str) -> Report {
        let mut report = Report::default();
        settle(ui);
        if click_item(ui, name) {
            self.visit(ui, name, &mut report);
        } else {
            report.add(vec![Finding::new(name, Problem::NoSuchItem { name: name.to_owned() })]);
        }
        report
    }

    /// The screen an item just opened, and what its buttons open.
    fn visit(&self, ui: &mut Ui, name: &str, report: &mut Report) {
        self.look(ui, name, report);
        // Whatever it opened over the screen goes away before the next item.
        report.add(close_windows(ui, name));
        for button in &self.open {
            if click(ui, Role::Button, button) {
                let place = format!("{name} > {button}");
                self.look(ui, &place, report);
                report.add(close_windows(ui, &place));
            }
        }
    }
}

/// Clicks the item of the navigation with this name, if the screen has it.
fn click_item(ui: &mut Ui, name: &str) -> bool {
    let items = navigation_items(&ui.access_tree());
    items.iter().find(|(_, n)| n == name).is_some_and(|(role, _)| click(ui, *role, name))
}

/// Clicks the enabled node with this role and name, if the screen has one.
fn click(ui: &mut Ui, role: Role, name: &str) -> bool {
    let tree = ui.access_tree();
    let Some(node) = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name) && !n.disabled)
        .filter(|n| n.actions.contains(&AccessAction::Click))
    else {
        return false;
    };
    ui.perform_action(node.id, AccessAction::Click);
    settle(ui);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravenrs_core::AccessNode;

    #[test]
    fn a_control_without_a_name_and_a_repeated_id_are_found() {
        let mut tree = AccessTree::default();
        tree.nodes.push(AccessNode::new(1, Role::Window, Rect::ZERO));
        tree.nodes.push(AccessNode::new(2, Role::Button, Rect::ZERO));
        let mut named = AccessNode::new(2, Role::Label, Rect::ZERO);
        named.name = Some("Nome".into());
        tree.nodes.push(named);
        tree.focus = Some(9);
        let found = check_tree(&tree, "teste");
        assert!(found.iter().any(|f| matches!(f.problem, Problem::Unnamed { role: Role::Button, .. })));
        assert!(found.iter().any(|f| f.problem == Problem::RepeatedId { id: 2 }));
        assert!(found.iter().any(|f| f.problem == Problem::FocusOnNothing));
    }

    #[test]
    fn the_themes_that_come_with_the_library_read_well() {
        for theme in [Theme::raven_dark(), Theme::raven_light(), Theme::raven_glass()] {
            let found = check_theme(&theme);
            assert!(found.is_empty(), "{found:?}");
        }
    }

    #[test]
    fn a_theme_with_faint_text_is_found() {
        let mut theme = Theme::raven_dark();
        theme.palette.ink_muted = theme.palette.bg_000;
        let found = check_theme(&theme);
        assert!(
            found.iter().any(
                |f| matches!(&f.problem, Problem::LowContrast { pair, .. } if pair.contains("InkMuted"))
            )
        );
    }
}
