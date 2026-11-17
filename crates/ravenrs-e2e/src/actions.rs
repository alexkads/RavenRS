//! The accessible actions a person does with more than a click: fill a field, pick an option,
//! open or close an item, and scroll a part into sight. Each one is real input, as a person's.

use ravenrs_core::{AccessNode, Event, Key, Modifiers, Point, PointerButton, Rect, Role};

use crate::{E2e, E2eError, E2eResult, ErrorCode, Locator, Query, descendants};

/// Steps a number field may take to reach a value with the arrow keys.
const NUMBER_STEPS: usize = 1000;
/// Wheel turns `scroll_into_view` tries before giving up.
const SCROLL_TURNS: usize = 60;

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

    /// Turns the wheel over the part that scrolls around the node until its middle is in sight.
    pub fn scroll_into_view(self) -> E2eResult {
        let query = self.query;
        self.e2e.reveal(&query)
    }
}

impl E2e {
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
        // A tree item opens with the arrow keys once it has the focus; other items, such as a
        // group row of a grid, open with a click.
        if node.role == Role::TreeItem {
            if self.tree.focus != Some(node.id) {
                self.button(point, PointerButton::Primary);
            }
            if self.find(query)?.expanded != Some(open) {
                self.press(if open { Key::ArrowRight } else { Key::ArrowLeft });
            }
        } else {
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
            let window = Rect::from_origin_size(Point::ZERO, self.ui.viewport());
            let port = self.scroller_of(node.id).map_or(window, |s| s.bounds);
            let sight = port.intersect(&window);
            let middle = node.bounds.center();
            if sight.contains(middle) {
                return Ok(());
            }
            let Some(scroller) = self.scroller_of(node.id) else { break };
            let at = scroller.bounds.intersect(&window).center();
            // Positive delta scrolls up, toward the start: content moves down.
            let reach = |from: f32, to: f32, size: f32| -> f32 {
                if middle_outside(from, to, size) {
                    (to - from).signum() * -(size * 0.6).max(40.0)
                } else {
                    0.0
                }
            };
            let dy = reach(sight.center().y, middle.y, sight.height());
            let dx = reach(sight.center().x, middle.x, sight.width());
            let before = node.bounds;
            self.move_to(at);
            self.send(&Event::Scroll { position: at, delta: Point::new(dx, dy) });
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
                for _ in 0..SCROLL_TURNS * 4 {
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

    /// The nearest part that scrolls around the node.
    fn scroller_of(&self, id: u64) -> Option<&AccessNode> {
        let mut at = id;
        loop {
            let parent = self.tree.nodes.iter().find(|n| n.children.contains(&at))?;
            if SCROLLERS.contains(&parent.role) && parent.bounds.height() > 0.0 {
                return Some(parent);
            }
            at = parent.id;
        }
    }
}

/// Whether a point at `to` is outside a span of `size` around `from`.
fn middle_outside(from: f32, to: f32, size: f32) -> bool {
    (to - from).abs() > size / 2.0
}
