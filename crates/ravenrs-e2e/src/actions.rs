//! The accessible actions a person does with more than a click: fill a field, pick an option,
//! open or close an item, and scroll a part into sight. Each one is real input, as a person's.

use ravenrs_core::{AccessAction, AccessNode, Event, Key, Modifiers, Point, PointerButton, Rect, Role};

use crate::{E2e, E2eError, E2eResult, ErrorCode, Locator, Query, descendants};

/// Steps a number field may take to reach a value with the arrow keys.
const NUMBER_STEPS: usize = 1000;
/// Wheel turns `scroll_into_view` tries before giving up.
const SCROLL_TURNS: usize = 60;
/// Wheel turns a hunt for a row not built yet takes in each direction at most. It stops at the
/// end of the list, when the screen no longer changes; this only keeps a list that never ends
/// from turning forever. At most of a page a turn, it reaches the end of a million rows.
const HUNT_TURNS: usize = 200_000;

/// The roles of the parts that scroll what they hold.
const SCROLLERS: [Role; 5] = [Role::ScrollView, Role::List, Role::ListBox, Role::Grid, Role::Tree];

/// The roles a list of choices shows its options in, closed or open.
const OPTIONS: [Role; 8] = [
    Role::ListBoxOption,
    Role::MenuItem,
    Role::ListItem,
    Role::TreeItem,
    Role::Row,
    Role::Tab,
    Role::RadioButton,
    Role::Cell,
];

impl Locator<'_> {
    /// Puts `text` in the field in place of what it has: a text field is cleared and typed
    /// into, a number (slider, spin button) is moved with the arrow keys until it shows the value.
    /// A cell of a grid is opened with a double click, as a person edits it, and the new text is
    /// confirmed with Enter; a cell marked read only fails with `E2E_DISABLED`.
    pub fn fill(self, text: &str) -> E2eResult {
        let query = self.query;
        let e2e = self.e2e;
        let (node, point) = e2e.target(&query)?;
        if node.read_only {
            return Err(E2eError::new(ErrorCode::Disabled, format!("{query}: it is read only"))
                .hint("check what makes it editable first")
                .screen(e2e.screen()));
        }
        if node.role == Role::Cell {
            return e2e.edit_cell(&query, point, text);
        }
        if e2e.tree.focus != Some(node.id) {
            e2e.button(point, PointerButton::Primary);
        }
        if node.numeric.is_some() {
            return e2e.step_number(&query, text);
        }
        let all = Modifiers { meta: true, ..Modifiers::default() };
        e2e.press_with(Key::Character("a".into()), all);
        e2e.press(Key::Backspace);
        e2e.type_text(text);
        Ok(())
    }

    /// Picks `option`: in a list, a tab list or a group shown in place, it clicks the option
    /// inside it; in a combo box or a button that opens a menu, it opens it first.
    pub fn select(self, option: &str) -> E2eResult {
        let query = self.query;
        let e2e = self.e2e;
        let (node, point) = e2e.target(&query)?;
        let in_place = matches!(
            node.role,
            Role::ListBox
                | Role::List
                | Role::Tree
                | Role::Grid
                | Role::Table
                | Role::TabList
                | Role::RadioGroup
                | Role::Menu
                | Role::MenuBar
        );
        if !in_place {
            e2e.button(point, PointerButton::Primary);
        }
        let choice = e2e.option(in_place.then_some(node.id), &query, option)?;
        // An option that declares how it is chosen is chosen that way; the rest with a click.
        if e2e.find(&choice)?.actions.contains(&AccessAction::Select) {
            return e2e.act(&choice, AccessAction::Select);
        }
        e2e.locate(choice).click()
    }

    /// Opens the item (a tree item, a group row, a section), if it is not open yet.
    pub fn expand(self) -> E2eResult {
        let query = self.query;
        self.e2e.set_open(&query, true)
    }

    /// Closes the item, if it is open.
    pub fn collapse(self) -> E2eResult {
        let query = self.query;
        self.e2e.set_open(&query, false)
    }

    /// Gives the node this text as its value through the action it declares
    /// ([`AccessAction::SetValue`]), as assistive technology does, with no typing. A node that does
    /// not declare it fails with `E2E_DISABLED`; [`Self::fill`] types instead.
    pub fn set_value(self, text: &str) -> E2eResult {
        let query = self.query;
        let e2e = self.e2e;
        let node = e2e.declaring(&query, AccessAction::SetValue)?;
        // A component that takes the action rebuilds; one that refuses the text does not.
        let taken = e2e.ui.set_access_value(node.id, text);
        e2e.frame();
        if taken {
            return Ok(());
        }
        Err(E2eError::new(ErrorCode::Expectation, format!("{query}: it did not take the value {text:?}"))
            .hint("check the values it takes, such as one of its options or a date as 2026-10-31")
            .screen(e2e.screen()))
    }

    /// Opens the node's menu through the action it declares ([`AccessAction::ShowMenu`]).
    pub fn show_menu(self) -> E2eResult {
        let query = self.query;
        self.e2e.act(&query, AccessAction::ShowMenu)
    }

    /// Closes the node (a dialog, a popover, a menu) through the action it declares
    /// ([`AccessAction::Dismiss`]).
    pub fn dismiss(self) -> E2eResult {
        let query = self.query;
        self.e2e.act(&query, AccessAction::Dismiss)
    }

    /// Asks the node for one of the actions it declares, as assistive technology does. A node
    /// that does not declare it fails with `E2E_DISABLED`, naming the action.
    pub fn perform(self, action: AccessAction) -> E2eResult {
        let query = self.query;
        self.e2e.act(&query, action)
    }

    /// Turns the wheel over the part that scrolls around the node until its middle is in sight.
    pub fn scroll_into_view(self) -> E2eResult {
        let query = self.query;
        self.e2e.reveal(&query)
    }
}

impl E2e {
    /// The node `query` finds, ready for input, once it is known to declare `action`.
    fn declaring(&mut self, query: &Query, action: AccessAction) -> E2eResult<AccessNode> {
        let (node, _) = self.target(query)?;
        if node.actions.contains(&action) {
            return Ok(node);
        }
        Err(E2eError::new(ErrorCode::Disabled, format!("{query}: it does not take the action {action:?}"))
            .hint("the component declares the actions it takes with `BuildCx::on_access`; use a click or the keys a person would")
            .screen(self.screen()))
    }

    /// Performs `action` on the node `query` finds, which must declare it.
    fn act(&mut self, query: &Query, action: AccessAction) -> E2eResult {
        let node = self.declaring(query, action)?;
        let taken = self.ui.perform_action(node.id, action);
        self.frame();
        if taken {
            return Ok(());
        }
        Err(E2eError::new(ErrorCode::Expectation, format!("{query}: it did not do {action:?}"))
            .screen(self.screen()))
    }

    /// Opens the cell at `point` with a double click, replaces the editor's text and confirms it.
    fn edit_cell(&mut self, query: &Query, point: Point, text: &str) -> E2eResult {
        self.button(point, PointerButton::Primary);
        self.button(point, PointerButton::Primary);
        self.send(&Event::DoubleClick { position: point });
        let editor = self.tree.focus.and_then(|id| self.tree.get(id)).map(|n| n.role);
        if !matches!(editor, Some(Role::TextInput | Role::SpinButton | Role::ComboBox)) {
            return Err(E2eError::new(
                ErrorCode::Disabled,
                format!("{query}: the cell did not open an editor"),
            )
            .hint("only cells of editable columns open one with a double click")
            .screen(self.screen()));
        }
        let all = Modifiers { meta: true, ..Modifiers::default() };
        self.press_with(Key::Character("a".into()), all);
        self.press(Key::Backspace);
        self.type_text(text);
        self.press(Key::Enter);
        Ok(())
    }

    fn step_number(&mut self, query: &Query, text: &str) -> E2eResult {
        let wanted: f64 = text.trim().replace(',', ".").parse().map_err(|_| {
            E2eError::new(ErrorCode::Expectation, format!("{query}: it takes a number, not {text:?}"))
                .screen(self.screen())
        })?;
        for _ in 0..NUMBER_STEPS {
            let Some(now) = self.find(query)?.numeric else { break };
            let half = now.step.unwrap_or(0.0) / 2.0;
            if (now.value - wanted).abs() <= half.max(f64::EPSILON) {
                return Ok(());
            }
            let before = now.value;
            self.press(if now.value < wanted { Key::ArrowUp } else { Key::ArrowDown });
            if self.find(query)?.numeric.is_none_or(|n| n.value == before) {
                break;
            }
        }
        let found = self.find(query)?.numeric.map(|n| n.value);
        Err(E2eError::new(
            ErrorCode::Expectation,
            format!("{query}: the arrow keys do not take it to {wanted}; it shows {found:?}"),
        )
        .hint("check its smallest and largest value and its step")
        .screen(self.screen()))
    }

    /// The option called `option` in the list `inside`, or, with no list, anywhere on the
    /// screen (where the list that just opened shows it).
    fn option(&self, inside: Option<u64>, asked: &Query, option: &str) -> E2eResult<Query> {
        let scope = inside.map(|id| descendants(&self.tree, id));
        let found: Vec<&AccessNode> = self
            .tree
            .nodes
            .iter()
            .filter(|n| OPTIONS.contains(&n.role) && n.name.as_deref() == Some(option))
            .filter(|n| scope.as_ref().is_none_or(|s| s.contains(&n.id)))
            .collect();
        match found.as_slice() {
            [one] => Ok(Query::by_ref(format!("e{}", self.refs.get(&one.id).copied().unwrap_or(0)))),
            [] => Err(E2eError::new(ErrorCode::NotFound, format!("{asked}: no option {option:?} to pick"))
                .hint("check the options it shows in the screen below")
                .similar(
                    self.tree
                        .nodes
                        .iter()
                        .filter(|n| OPTIONS.contains(&n.role))
                        .filter(|n| scope.as_ref().is_none_or(|s| s.contains(&n.id)))
                        .take(3)
                        .map(|n| self.line_of(n))
                        .collect(),
                )
                .screen(self.screen())),
            many => Err(E2eError::new(
                ErrorCode::Ambiguous,
                format!("{asked}: {} options are called {option:?}", many.len()),
            )
            .similar(many.iter().take(3).map(|n| self.line_of(n)).collect())
            .screen(self.screen())),
        }
    }

    fn set_open(&mut self, query: &Query, open: bool) -> E2eResult {
        let (node, point) = self.target(query)?;
        let Some(now) = node.expanded else {
            return Err(E2eError::new(
                ErrorCode::Expectation,
                format!("{query}: it does not open or close (it says nothing about being expanded)"),
            )
            .screen(self.screen()));
        };
        if now == open {
            return Ok(());
        }
        let wanted = if open { AccessAction::Expand } else { AccessAction::Collapse };
        if node.actions.contains(&wanted) {
            self.act(query, wanted)?;
        } else if node.role == Role::TreeItem {
            // A tree item opens with the arrow keys once it has the focus.
            if self.tree.focus != Some(node.id) {
                self.button(point, PointerButton::Primary);
            }
            if self.find(query)?.expanded != Some(open) {
                self.press(if open { Key::ArrowRight } else { Key::ArrowLeft });
            }
        } else {
            // Other items, such as a group row of a grid, open with a click.
            self.button(point, PointerButton::Primary);
        }
        let after = self.find(query)?.expanded;
        if after == Some(open) {
            return Ok(());
        }
        Err(E2eError::new(
            ErrorCode::Expectation,
            format!("{query}: expected it {}, it stayed {after:?}", if open { "open" } else { "closed" }),
        )
        .screen(self.screen()))
    }

    /// Scrolls the node into sight, if a part that scrolls holds it.
    pub(crate) fn reveal(&mut self, query: &Query) -> E2eResult {
        if let Err(missing) = self.find(query) {
            if missing.code() != ErrorCode::NotFound {
                return Err(missing);
            }
            self.hunt(query, missing)?;
        }
        for _ in 0..SCROLL_TURNS {
            let node = self.find(query)?;
            let middle = node.bounds.center();
            if self.sight(node.id).contains(middle) {
                return Ok(());
            }
            let Some((at, delta)) = self.turn_toward(node.id, middle) else { break };
            let before = node.bounds;
            self.move_to(at);
            self.send(&Event::Scroll { position: at, delta });
            if self.find(query)?.bounds == before {
                break;
            }
        }
        let b = self.find(query)?.bounds;
        Err(E2eError::new(
            ErrorCode::NotVisible,
            format!(
                "{query}: scrolling does not bring it into sight ({:.0}×{:.0} at {:.0},{:.0})",
                b.width(),
                b.height(),
                b.x(),
                b.y()
            ),
        )
        .hint("make the window larger with `viewport`, or check that a part that scrolls holds it")
        .screen(self.screen()))
    }

    /// The part of the window where the node can be seen: the window, cut by every part that
    /// scrolls around it. A node whose middle is outside it is hidden, even inside the window.
    pub(crate) fn sight(&self, id: u64) -> Rect {
        let window = Rect::from_origin_size(Point::ZERO, self.ui.viewport());
        self.scroll_ports(id).iter().fold(window, |sight, port| sight.intersect(port))
    }

    /// Where to turn the wheel, and how far, to bring `point` closer to sight. The innermost
    /// part that scrolls and leaves the point out is turned; when that part is itself out of
    /// sight (a list far down a page), the part around it is turned toward it first.
    fn turn_toward(&self, id: u64, point: Point) -> Option<(Point, Point)> {
        let window = Rect::from_origin_size(Point::ZERO, self.ui.viewport());
        let chain = self.scroll_ports(id);
        let mut aim = point;
        for (k, frame) in chain.iter().enumerate() {
            let port = if k + 1 == chain.len() { frame.intersect(&window) } else { *frame };
            if port.contains(aim) {
                continue;
            }
            // What shows of this part: its bounds, cut by the window and the parts around it.
            let shown = chain[k + 1..].iter().fold(frame.intersect(&window), |r, outer| r.intersect(outer));
            if shown.width() <= 0.0 || shown.height() <= 0.0 {
                aim = frame.center();
                continue;
            }
            // Positive delta scrolls up, toward the start: content moves down.
            let reach = |from: f32, to: f32, size: f32| -> f32 {
                if middle_outside(from, to, size) {
                    (to - from).signum() * -(size * 0.6).max(40.0)
                } else {
                    0.0
                }
            };
            let center = shown.center();
            let delta =
                Point::new(reach(center.x, aim.x, shown.width()), reach(center.y, aim.y, shown.height()));
            return Some((center, delta));
        }
        None
    }

    /// A long list builds only the rows in sight, so a row further down is not on the screen
    /// at all: turns the wheel over each part that scrolls, down to its end and then up to its
    /// start, until the node shows.
    fn hunt(&mut self, query: &Query, missing: E2eError) -> E2eResult {
        let window = Rect::from_origin_size(Point::ZERO, self.ui.viewport());
        let mut scrollers: Vec<(u64, Rect)> = self
            .tree
            .nodes
            .iter()
            .filter(|n| SCROLLERS.contains(&n.role) && n.bounds.intersect(&window).height() > 0.0)
            .map(|n| (n.id, n.bounds.intersect(&window)))
            .collect();
        // The innermost first: a list inside a page that scrolls is where rows live.
        scrollers.reverse();
        for (_, port) in scrollers {
            let at = port.center();
            for down in [true, false] {
                for _ in 0..HUNT_TURNS {
                    if self.find(query).is_ok() {
                        return Ok(());
                    }
                    let before = self.snapshot();
                    let step = (port.height() * 0.8).max(40.0);
                    self.move_to(at);
                    self.send(&Event::Scroll {
                        position: at,
                        delta: Point::new(0.0, if down { -step } else { step }),
                    });
                    if self.snapshot() == before {
                        break;
                    }
                }
            }
        }
        if self.find(query).is_ok() { Ok(()) } else { Err(missing) }
    }

    /// The frames of the parts that scroll around the node, the nearest first: what the app
    /// says scrolls (a `ScrollView` has no node of its own) and the lists, grids and trees.
    fn scroll_ports(&self, id: u64) -> Vec<Rect> {
        let mut ports = self.ui.scroll_frames(id);
        let mut at = id;
        while let Some(parent) = self.tree.nodes.iter().find(|n| n.children.contains(&at)) {
            let frame = parent.bounds;
            if SCROLLERS.contains(&parent.role) && frame.height() > 0.0 && !ports.contains(&frame) {
                // A part inside another is smaller: keep the nearest first.
                let place =
                    ports.iter().position(|p| p.width() * p.height() > frame.width() * frame.height());
                ports.insert(place.unwrap_or(ports.len()), frame);
            }
            at = parent.id;
        }
        ports
    }
}

/// Whether a point at `to` is outside a span of `size` around `from`.
fn middle_outside(from: f32, to: f32, size: f32) -> bool {
    (to - from).abs() > size / 2.0
}
