//! End-to-end tests for RavenRS apps.
//!
//! An [`E2e`] session runs a view with no window, the way the app runs on screen, and lets a
//! test find a part of the screen, act on it and check it:
//!
//! ```no_run
//! use ravenrs_core::Role;
//! use ravenrs_e2e::{E2e, E2eResult, Query};
//! use ravenrs_style::Theme;
//! # use ravenrs_widgets_base::{Button, View, ViewExt};
//! # fn screen() -> impl View { Button::new("Save").test_id("save") }
//!
//! fn saving_works() -> E2eResult {
//!     let mut e2e = E2e::new(screen(), Theme::raven_dark());
//!     e2e.by_test_id("save").click()?;
//!     e2e.locate(Query::role(Role::Status)).expect().to_have_name("Saved")?;
//!     Ok(())
//! }
//! ```
//!
//! The parts are found in the accessibility tree, the same one screen readers read: by the id
//! the app declared with `ViewExt::test_id`, by role and name, or by the text shown. Actions are
//! real input events at the middle of the part, so they go through the same path as a person's,
//! and a part that is disabled, hidden or covered by something else fails instead of being
//! clicked anyway. Every failure carries a stable [`ErrorCode`], what was on the screen, and up
//! to three nodes that came close.
//!
//! [`E2e::snapshot`] writes the screen as text, one line per node with its role, name, test id,
//! value and states and a `[ref=eN]` that stays the same for the whole session. It is how a
//! test, a report or a coding agent sees the screen without an image. Secret values never
//! appear in it.
//!
//! Time is the session's: it moves only with [`E2e::advance`] and while a step waits, so an
//! animation or a timer never makes a test pass on one machine and fail on another.
//! [`E2e::wait_until`] looks again until a check holds, moving the clock with real time so
//! timers fire and work done in the background shows, and fails with
//! [`ErrorCode::Timeout`] when it does not; [`E2e::patiently`] is the same for a step that
//! should simply not fail for being early.
//!
//! Besides clicks and keys, a [`Locator`] fills a field ([`Locator::fill`]), picks an option
//! ([`Locator::select`]), opens and closes an item ([`Locator::expand`],
//! [`Locator::collapse`]) and scrolls a part into sight ([`Locator::scroll_into_view`]), even a
//! row a long list has not built yet. Lists, grids and trees with keys give their rows the test
//! ids `row:<key>` and `item:<key>`, which stay with the data when it moves.

mod actions;
mod error;
mod observe;
mod patience;
mod query;
mod snapshot;

use std::collections::{HashMap, HashSet};

use ravenrs_core::{
    AccessNode, AccessTree, Event, Key, Modifiers, Point, PointerButton, Rect, Role, Size, Toggled,
};
use ravenrs_style::Theme;
use ravenrs_widgets_base::{Ui, View};

pub use error::{E2eError, E2eResult, ErrorCode};
pub use observe::FrameObserver;
pub use query::Query;

/// The window size a session starts with, a common laptop screen.
const DEFAULT_VIEWPORT: Size = Size::new(1280.0, 800.0);
/// Lines of the screen a failure keeps, so a large screen does not drown the message.
const SCREEN_LINES: usize = 80;
/// Pointer moves on the way of a drag, so drag handlers see it travel.
const DRAG_STEPS: u32 = 8;
/// How far the pointer moves, on each axis, before a drag looks for where to drop.
const DRAG_START: f32 = 6.0;
/// Frames past the first a step may take to settle, so a screen that keeps asking for one
/// does not hang the test.
/// Frames of the session's clock a busy node is given to get free, 5 s at 60 a second.
const BUSY_FRAMES: usize = 300;
/// Real time a busy node is given after that, for work away from the screen.
const BUSY_REAL: std::time::Duration = std::time::Duration::from_secs(5);
const SETTLE_FRAMES: u32 = 4;

/// What a session for a window holds until it borrows the window's interface.
struct Nothing;

impl View for Nothing {
    fn build(&self, cx: &mut ravenrs_widgets_base::BuildCx<'_>) -> ravenrs_widgets_base::NodeId {
        cx.empty()
    }
}

/// A view running with no window, driven by a test. See the [crate documentation](crate).
pub struct E2e {
    ui: Ui,
    tree: AccessTree,
    refs: HashMap<u64, u32>,
    next_ref: u32,
    pointer: Point,
    clock: f64,
    size: Size,
    observer: Option<Box<dyn FrameObserver>>,
}

impl std::fmt::Debug for E2e {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("E2e").field("nodes", &self.tree.nodes.len()).finish_non_exhaustive()
    }
}

impl E2e {
    /// Runs `view` in a 1280 × 800 window at scale 1.
    pub fn new(view: impl View + 'static, theme: Theme) -> Self {
        let mut ui = Ui::new(view, theme);
        ui.set_viewport(DEFAULT_VIEWPORT, 1.0);
        let mut session = Self {
            ui,
            tree: AccessTree::default(),
            refs: HashMap::new(),
            next_ref: 1,
            pointer: Point::ZERO,
            clock: 0.0,
            size: DEFAULT_VIEWPORT,
            observer: None,
        };
        session.frame();
        session
    }

    /// A session with no interface of its own, to drive the interface of a real window with
    /// [`Self::in_window`]: what an app opened with a test tool's `--headed` does, so the steps of
    /// a scenario act on what the person sees. Until then it holds an empty screen.
    pub fn for_window() -> Self {
        Self::new(Nothing, Theme::default())
    }

    /// Runs `steps` on `ui`, the interface a window shows, whose clock reads `now` seconds: the
    /// session borrows it for the steps and gives it back, so every click, key and check of
    /// [`Self::for_window`] lands on the window, and its clock, size and references to parts of
    /// the screen carry on from one call to the next. Waiting steps hold the window's thread
    /// while they wait, as they hold the test's in a session with no window.
    pub fn in_window<R>(&mut self, ui: &mut Ui, now: f64, steps: impl FnOnce(&mut Self) -> R) -> R {
        std::mem::swap(&mut self.ui, ui);
        self.size = self.ui.viewport();
        // The window's clock may be ahead of the session's; the session's only moves forward.
        self.clock = self.clock.max(now);
        self.ui.set_time(self.clock);
        self.frame();
        let result = steps(self);
        std::mem::swap(&mut self.ui, ui);
        result
    }

    /// Changes the window size and scale, such as `Size::new(375.0, 812.0)` for a phone.
    pub fn viewport(mut self, size: Size, scale: f32) -> Self {
        self.size = size;
        self.ui.set_viewport(size, scale);
        self.frame();
        self
    }

    /// The running interface, for what this session does not cover.
    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    /// The accessibility tree as of the latest frame.
    pub fn tree(&self) -> &AccessTree {
        &self.tree
    }

    /// The screen as text, one line per node; see the [crate documentation](crate).
    pub fn snapshot(&self) -> String {
        snapshot::render(&self.tree, &self.refs)
    }

    /// A handle on what `query` finds, to act on it or check it.
    pub fn locate(&mut self, query: Query) -> Locator<'_> {
        Locator { e2e: self, query }
    }

    /// Shorthand for `locate(Query::test_id(id))`.
    pub fn by_test_id(&mut self, id: impl Into<String>) -> Locator<'_> {
        self.locate(Query::test_id(id))
    }

    /// Shorthand for `locate(Query::role(role))`; narrow it with [`Locator::name`].
    pub fn by_role(&mut self, role: Role) -> Locator<'_> {
        self.locate(Query::role(role))
    }

    /// Shorthand for `locate(Query::text(text))`.
    pub fn by_text(&mut self, text: impl Into<String>) -> Locator<'_> {
        self.locate(Query::text(text))
    }

    /// Shorthand for `locate(Query::by_ref(reference))`, with a reference from [`Self::snapshot`].
    pub fn by_ref(&mut self, reference: impl Into<String>) -> Locator<'_> {
        self.locate(Query::by_ref(reference))
    }

    /// The one node `query` finds.
    pub fn find(&self, query: &Query) -> E2eResult<AccessNode> {
        self.resolve(query).cloned()
    }

    /// Where the target's test id was declared, or its instrumented view was constructed.
    /// Requires `e2e-source` in the app under test; targets without a recorded origin fail
    /// with `E2E_NO_SOURCE` rather than guessing. An explicit test id's origin takes priority.
    pub fn source(&self, query: &Query) -> E2eResult<&'static std::panic::Location<'static>> {
        let node = self.find(query)?;
        if let Some(source) = self.ui.test_source(node.id) {
            return Ok(source);
        }
        Err(E2eError::new(ErrorCode::NoSource, "the target has no declared source location")
            .hint(
                "enable e2e-source in the app under test; use an instrumented constructor or declare test_id",
            )
            .screen(self.snapshot()))
    }

    /// How many nodes `query` finds (ignoring [`Query::nth`]).
    pub fn count(&self, query: &Query) -> E2eResult<usize> {
        Ok(self.candidates(query)?.len())
    }

    /// Presses and releases `key` where the keyboard focus is.
    pub fn press(&mut self, key: Key) {
        self.press_with(key, Modifiers::default());
    }

    /// Presses and releases `key` with modifiers held, such as Command for a shortcut.
    pub fn press_with(&mut self, key: Key, modifiers: Modifiers) {
        self.observed_key();
        self.send(&Event::KeyDown { key: key.clone(), modifiers });
        self.send(&Event::KeyUp { key, modifiers });
    }

    /// Types `text` where the keyboard focus is, one key per character.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.press(Key::Character(c.to_string()));
        }
    }

    fn send(&mut self, event: &Event) {
        self.ui.handle_event(event);
        self.frame();
    }

    /// Builds the frames the latest input asks for, until the screen settles (some components,
    /// such as a diagram, need a second frame for their layout), and reads the tree again.
    fn frame(&mut self) {
        self.ui.update();
        for _ in 0..SETTLE_FRAMES {
            if !self.ui.needs_redraw() {
                break;
            }
            self.ui.update();
        }
        self.observed_frame();
        self.tree = self.ui.access_tree();
        for node in &self.tree.nodes {
            if node.id != self.tree.root && !self.refs.contains_key(&node.id) {
                self.refs.insert(node.id, self.next_ref);
                self.next_ref += 1;
            }
        }
    }

    /// A failure with `code`, `message` and the screen as it is now: what a driver of a
    /// component (such as a grid's) returns when a step of its own cannot be done.
    pub fn failure(&self, code: ErrorCode, message: impl Into<String>) -> E2eError {
        E2eError::new(code, message).screen(self.screen())
    }

    fn screen(&self) -> String {
        let text = self.snapshot();
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() <= SCREEN_LINES {
            return text;
        }
        let mut kept = lines[..SCREEN_LINES].join("\n");
        kept.push_str(&format!("\n… {} more lines", lines.len() - SCREEN_LINES));
        kept
    }

    fn line_of(&self, node: &AccessNode) -> String {
        snapshot::line(node, self.refs.get(&node.id).copied(), self.tree.focus == Some(node.id))
    }

    /// Every node `query` matches, in reading order.
    fn candidates(&self, query: &Query) -> E2eResult<Vec<&AccessNode>> {
        let scope = match query.parent() {
            Some(parent) => Some(descendants(&self.tree, self.resolve(parent)?.id)),
            None => None,
        };
        let reference = match query.reference() {
            Some(r) => {
                let number = r.trim_start_matches('e').parse::<u32>().ok();
                Some(self.refs.iter().find(|(_, n)| Some(**n) == number).map(|(id, _)| *id))
            }
            None => None,
        };
        Ok(self
            .tree
            .nodes
            .iter()
            .filter(|n| n.id != self.tree.root)
            .filter(|n| scope.as_ref().is_none_or(|s| s.contains(&n.id)))
            .filter(|n| reference.is_none_or(|r| r == Some(n.id)))
            .filter(|n| query.matches(n))
            .collect())
    }

    /// The one node `query` finds, or why there is not exactly one.
    fn resolve(&self, query: &Query) -> E2eResult<&AccessNode> {
        let found = self.candidates(query)?;
        if let Some(index) = query.index() {
            return found.get(index).copied().ok_or_else(|| {
                E2eError::new(
                    ErrorCode::NotFound,
                    format!("{query}: asked for match {index}, but there are {}", found.len()),
                )
                .screen(self.screen())
            });
        }
        match found.as_slice() {
            [one] => Ok(one),
            [] => Err(self.not_found(query)),
            many => Err(E2eError::new(
                ErrorCode::Ambiguous,
                format!("{query}: {} nodes match and the query does not say which", many.len()),
            )
            .hint("give the part a test id, narrow the query with `within`, or pick one with `nth`")
            .similar(many.iter().take(3).map(|n| self.line_of(n)).collect())
            .screen(self.screen())),
        }
    }

    fn not_found(&self, query: &Query) -> E2eError {
        let mut close: Vec<(u32, &AccessNode)> = self
            .tree
            .nodes
            .iter()
            .filter(|n| n.id != self.tree.root)
            .map(|n| (query.closeness(n), n))
            .filter(|(score, _)| *score > 0)
            .collect();
        close.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        let similar: Vec<String> = close.iter().take(3).map(|(_, n)| self.line_of(n)).collect();
        // While a modal is open only the modal shows: the rest of the screen is behind it. A
        // query already scoped `within` something on the screen is just not found.
        if query.parent().is_none()
            && let Some(modal) = self.tree.nodes.iter().find(|n| n.modal)
        {
            let name = modal.name.as_deref().unwrap_or("without a name");
            return E2eError::new(
                ErrorCode::Covered,
                format!(
                    "{query}: not in the open {} {name:?}, which covers the rest of the screen",
                    snapshot::role_name(modal.role)
                ),
            )
            .hint("close the dialog first, or look inside it with `within`")
            .similar(similar)
            .screen(self.screen());
        }
        E2eError::new(ErrorCode::NotFound, format!("{query}: nothing on the screen matches"))
            .hint("check the screen below; a part with no name can be tagged with `ViewExt::test_id`")
            .similar(similar)
            .screen(self.screen())
    }

    /// The node an action aims at, and the point it aims at, once it is known to take input.
    /// A node out of sight inside a part that scrolls is scrolled to first, as a person
    /// scrolls before clicking; out of the window with nothing to scroll, it is not visible.
    fn target(&mut self, query: &Query) -> E2eResult<(AccessNode, Point)> {
        let node = self.resolve(query)?.clone();
        if node.disabled {
            return Err(E2eError::new(ErrorCode::Disabled, format!("{query}: it is disabled"))
                .hint("check what enables it first, or expect it to be disabled with `to_be_disabled`")
                .screen(self.screen()));
        }
        let node = if node.busy { self.until_free(query)? } else { node };
        if node.bounds.width() > 0.0 && !self.sight(node.id).contains(node.bounds.center()) {
            self.reveal(query)?;
        }
        let node = self.resolve(query)?.clone();
        let sight = self.sight(node.id);
        let b = node.bounds;
        let point = b.center();
        if b.width() <= 0.0 || b.height() <= 0.0 || !sight.contains(point) {
            return Err(E2eError::new(
                ErrorCode::NotVisible,
                format!(
                    "{query}: it is outside the window or has no size ({:.0}×{:.0} at {:.0},{:.0})",
                    b.width(),
                    b.height(),
                    b.x(),
                    b.y()
                ),
            )
            .hint("scroll it into view or make the window larger with `viewport`")
            .screen(self.screen()));
        }
        match self.ui.access_node_at(point) {
            Some(hit) if self.related(hit, node.id) => {}
            Some(hit) => {
                let by = self.tree.get(hit).map(|n| self.line_of(n)).unwrap_or_default();
                return Err(E2eError::new(
                    ErrorCode::Covered,
                    format!("{query}: something else is on top of it: {by}"),
                )
                .hint("close or move what covers it first")
                .screen(self.screen()));
            }
            None if node.actions.contains(&ravenrs_core::AccessAction::Click) => {
                return Err(E2eError::new(
                    ErrorCode::Covered,
                    format!("{query}: nothing takes the pointer at its middle"),
                )
                .screen(self.screen()));
            }
            None => {}
        }
        Ok((node, point))
    }

    /// Waits for a busy node to be free: first on the session's clock, frame by frame, so work
    /// that ends on a timer passes with no real wait, then in real time, for work away from the
    /// screen (a thread, a server). Still busy after that, it fails with `E2E_TIMEOUT`.
    fn until_free(&mut self, query: &Query) -> E2eResult<AccessNode> {
        for _ in 0..BUSY_FRAMES {
            self.advance(1.0 / 60.0);
            let node = self.resolve(query)?;
            if !node.busy {
                return Ok(node.clone());
            }
        }
        let start = std::time::Instant::now();
        while start.elapsed() < BUSY_REAL {
            std::thread::sleep(std::time::Duration::from_millis(10));
            self.advance(0.01);
            let node = self.resolve(query)?;
            if !node.busy {
                return Ok(node.clone());
            }
        }
        Err(E2eError::new(
            ErrorCode::Timeout,
            format!(
                "{query}: it was busy and did not get free in {} s",
                BUSY_FRAMES / 60 + BUSY_REAL.as_secs() as usize
            ),
        )
        .hint("check the work it is busy with, or what keeps it marked busy")
        .screen(self.screen()))
    }

    /// Whether a click landing on `hit` reaches `target`: it is the target, inside it, or around it.
    fn related(&self, hit: u64, target: u64) -> bool {
        hit == target
            || descendants(&self.tree, target).contains(&hit)
            || descendants(&self.tree, hit).contains(&target)
    }

    fn move_to(&mut self, point: Point) {
        self.pointer = point;
        self.observed_pointer(point);
        self.send(&Event::PointerMove { position: point });
    }

    fn button(&mut self, point: Point, button: PointerButton) {
        self.move_to(point);
        if button == PointerButton::Primary {
            self.observed_click(point);
        }
        self.send(&Event::PointerDown { position: point, button });
        self.send(&Event::PointerUp { position: point, button });
    }
}

/// Every node inside `id`, not counting it.
fn descendants(tree: &AccessTree, id: u64) -> HashSet<u64> {
    let mut out = HashSet::new();
    let mut stack = tree.get(id).map(|n| n.children.clone()).unwrap_or_default();
    while let Some(child) = stack.pop() {
        if out.insert(child)
            && let Some(node) = tree.get(child)
        {
            stack.extend(node.children.iter().copied());
        }
    }
    out
}

/// What a [`Query`] finds in a session, to act on or check. Made by [`E2e::locate`] and its
/// shorthands; each action finds the node again, on the screen as it is at that moment.
#[derive(Debug)]
pub struct Locator<'a> {
    e2e: &'a mut E2e,
    query: Query,
}

impl<'a> Locator<'a> {
    /// Only nodes with exactly this name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.query = self.query.name(name);
        self
    }

    /// Only nodes inside the one `parent` finds.
    pub fn within(mut self, parent: Query) -> Self {
        self.query = self.query.within(parent);
        self
    }

    /// The match at this position when several match.
    pub fn nth(mut self, index: usize) -> Self {
        self.query = self.query.nth(index);
        self
    }

    /// The query this locator uses.
    pub fn query(&self) -> &Query {
        &self.query
    }

    /// The node as it is now.
    pub fn node(&self) -> E2eResult<AccessNode> {
        self.e2e.find(&self.query)
    }

    /// How many nodes match.
    pub fn count(&self) -> E2eResult<usize> {
        self.e2e.count(&self.query)
    }

    /// Clicks the middle of the node with the primary button.
    pub fn click(self) -> E2eResult {
        let (_, point) = self.e2e.target(&self.query)?;
        self.e2e.button(point, PointerButton::Primary);
        Ok(())
    }

    /// Clicks twice in quick succession.
    pub fn double_click(self) -> E2eResult {
        let (_, point) = self.e2e.target(&self.query)?;
        self.e2e.button(point, PointerButton::Primary);
        self.e2e.button(point, PointerButton::Primary);
        self.e2e.send(&Event::DoubleClick { position: point });
        Ok(())
    }

    /// Clicks with the secondary button, which opens a context menu.
    pub fn right_click(self) -> E2eResult {
        let (_, point) = self.e2e.target(&self.query)?;
        self.e2e.button(point, PointerButton::Secondary);
        Ok(())
    }

    /// Moves the pointer over the node, which shows tooltips and hover states.
    pub fn hover(self) -> E2eResult {
        let (_, point) = self.e2e.target(&self.query)?;
        self.e2e.move_to(point);
        Ok(())
    }

    /// Clicks the node unless it has the keyboard focus, then types `text` into it.
    pub fn type_text(self, text: &str) -> E2eResult {
        let (node, point) = self.e2e.target(&self.query)?;
        if self.e2e.tree.focus != Some(node.id) {
            self.e2e.button(point, PointerButton::Primary);
        }
        self.e2e.type_text(text);
        Ok(())
    }

    /// Clicks the node unless it has the keyboard focus, then presses `key`.
    pub fn press(self, key: Key) -> E2eResult {
        let (node, point) = self.e2e.target(&self.query)?;
        if self.e2e.tree.focus != Some(node.id) {
            self.e2e.button(point, PointerButton::Primary);
        }
        self.e2e.press(key);
        Ok(())
    }

    /// Drags the node to the middle of the one `to` finds, with the primary button held.
    ///
    /// `to` is looked for once the drag has started, as a person looks for where to drop: what
    /// shows only during a drag, such as the free ports of a diagram, can be the target. When it
    /// is not there, the button is let go where it was pressed and the step fails.
    pub fn drag_to(self, to: &Query) -> E2eResult {
        let (_, from) = self.e2e.target(&self.query)?;
        let button = PointerButton::Primary;
        self.e2e.move_to(from);
        self.e2e.send(&Event::PointerDown { position: from, button });
        // A few points away, so the component sees a drag and not a click.
        self.e2e.move_to(Point::new(from.x + DRAG_START, from.y + DRAG_START));
        let end = match self.e2e.resolve(to) {
            Ok(node) => node.bounds.center(),
            Err(e) => {
                self.e2e.move_to(from);
                self.e2e.send(&Event::PointerUp { position: from, button });
                return Err(e);
            }
        };
        for step in 1..=DRAG_STEPS {
            let t = step as f32 / DRAG_STEPS as f32;
            let p = Point::new(from.x + (end.x - from.x) * t, from.y + (end.y - from.y) * t);
            self.e2e.move_to(p);
        }
        self.e2e.send(&Event::PointerUp { position: end, button });
        Ok(())
    }

    /// Checks on the node.
    pub fn expect(self) -> Expect<'a> {
        Expect { e2e: self.e2e, query: self.query }
    }
}

/// Checks on what a [`Locator`] finds; each one fails with [`ErrorCode::Expectation`] and the
/// screen when it does not hold.
#[derive(Debug)]
pub struct Expect<'a> {
    e2e: &'a mut E2e,
    query: Query,
}

impl Expect<'_> {
    fn node(&self) -> E2eResult<AccessNode> {
        self.e2e.find(&self.query)
    }

    fn fail(&self, what: String) -> E2eError {
        E2eError::new(ErrorCode::Expectation, format!("{}: {what}", self.query)).screen(self.e2e.screen())
    }

    /// The node's value is exactly `value`. A secret value fails with
    /// [`ErrorCode::ProtectedValue`] instead of being compared.
    pub fn to_have_value(&self, value: &str) -> E2eResult {
        let node = self.node()?;
        if node.protected {
            return Err(E2eError::new(
                ErrorCode::ProtectedValue,
                format!("{}: its value is secret and is never given out", self.query),
            )
            .hint("check what the app does with the value instead, such as signing in")
            .screen(self.e2e.screen()));
        }
        match node.value.as_deref() {
            Some(v) if v == value => Ok(()),
            other => Err(self.fail(format!("expected value {value:?}, found {other:?}"))),
        }
    }

    /// The node's accessible name is exactly `name`.
    pub fn to_have_name(&self, name: &str) -> E2eResult {
        let node = self.node()?;
        match node.name.as_deref() {
            Some(n) if n == name => Ok(()),
            other => Err(self.fail(format!("expected name {name:?}, found {other:?}"))),
        }
    }

    /// The node's accessible description is exactly `description`.
    pub fn to_have_description(&self, description: &str) -> E2eResult {
        let node = self.node()?;
        match node.description.as_deref() {
            Some(d) if d == description => Ok(()),
            other => Err(self.fail(format!("expected description {description:?}, found {other:?}"))),
        }
    }

    /// The node's name or description contains `text`, such as the state a diagram node says
    /// ("done", an error message).
    pub fn to_mention(&self, text: &str) -> E2eResult {
        let node = self.node()?;
        let said = [node.name.as_deref(), node.description.as_deref()];
        if said.iter().flatten().any(|s| s.contains(text)) {
            return Ok(());
        }
        Err(self.fail(format!(
            "expected it to mention {text:?}; name {:?}, description {:?}",
            node.name, node.description
        )))
    }

    /// The node is checked (check box, switch, toggle).
    pub fn to_be_checked(&self) -> E2eResult {
        let node = self.node()?;
        if node.toggled == Some(Toggled::True) {
            Ok(())
        } else {
            Err(self.fail(format!("expected checked, found {:?}", node.toggled)))
        }
    }

    /// The node can be checked and is not (check box, switch, toggle).
    pub fn to_be_unchecked(&self) -> E2eResult {
        let node = self.node()?;
        if node.toggled == Some(Toggled::False) {
            Ok(())
        } else {
            Err(self.fail(format!("expected unchecked, found {:?}", node.toggled)))
        }
    }

    /// The node does not respond to input.
    pub fn to_be_disabled(&self) -> E2eResult {
        if self.node()?.disabled { Ok(()) } else { Err(self.fail("expected disabled, it is enabled".into())) }
    }

    /// The node responds to input.
    pub fn to_be_enabled(&self) -> E2eResult {
        if self.node()?.disabled { Err(self.fail("expected enabled, it is disabled".into())) } else { Ok(()) }
    }

    /// The node has the keyboard focus.
    pub fn to_be_focused(&self) -> E2eResult {
        let node = self.node()?;
        if self.e2e.tree.focus == Some(node.id) {
            return Ok(());
        }
        let focused = self.e2e.tree.focused().map(|n| self.e2e.line_of(n));
        Err(self.fail(format!("expected the focus, which is on {}", focused.as_deref().unwrap_or("nothing"))))
    }

    /// The node has a size and its middle is inside the window.
    pub fn to_be_visible(&self) -> E2eResult {
        let b = self.node()?.bounds;
        let window = Rect::from_origin_size(Point::ZERO, self.e2e.ui.viewport());
        if b.width() > 0.0 && b.height() > 0.0 && window.contains(b.center()) {
            Ok(())
        } else {
            Err(self.fail(format!(
                "expected it visible, it is {:.0}×{:.0} at {:.0},{:.0}",
                b.width(),
                b.height(),
                b.x(),
                b.y()
            )))
        }
    }

    /// Exactly `count` nodes match (ignoring [`Query::nth`]).
    pub fn to_have_count(&self, count: usize) -> E2eResult {
        let found = self.e2e.count(&self.query)?;
        if found == count {
            Ok(())
        } else {
            Err(self.fail(format!("expected {count} matches, found {found}")))
        }
    }
}
