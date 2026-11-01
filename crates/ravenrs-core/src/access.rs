//! A platform-neutral accessibility tree: what screen readers and other
//! assistive technology see. Widgets describe themselves with a [`Role`] and
//! state; platform layers translate the [`AccessTree`] to the operating
//! system's API (AccessKit on desktop, ARIA in the browser).

use crate::Rect;

/// What kind of control or content a node is, as assistive technology names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// The application window, the root of the tree.
    Window,
    /// A container with no meaning of its own.
    Group,
    /// Static text.
    Label,
    /// A heading; see [`AccessNode::level`].
    Heading,
    /// Activates an action.
    Button,
    /// Navigates somewhere.
    Link,
    /// A two- or three-state check box.
    CheckBox,
    /// An on/off switch.
    Switch,
    /// A set of radio buttons.
    RadioGroup,
    /// One choice of a radio group.
    RadioButton,
    /// Editable single-line text.
    TextInput,
    /// Editable multi-line text.
    MultilineTextInput,
    /// A value picked along a track.
    Slider,
    /// A number changed with increment and decrement buttons.
    SpinButton,
    /// A field that opens a list of choices.
    ComboBox,
    /// A list of selectable options.
    ListBox,
    /// One option of a list box.
    ListBoxOption,
    /// A popup menu.
    Menu,
    /// A horizontal bar of menus.
    MenuBar,
    /// One command in a menu.
    MenuItem,
    /// A row of tabs.
    TabList,
    /// One tab.
    Tab,
    /// The content shown for the selected tab.
    TabPanel,
    /// A window over the app; see [`AccessNode::modal`].
    Dialog,
    /// An urgent message.
    Alert,
    /// A status message (toasts).
    Status,
    /// A rich tooltip.
    Tooltip,
    /// A row of controls.
    Toolbar,
    /// A data grid.
    Grid,
    /// A static table.
    Table,
    /// A row of a grid or table.
    Row,
    /// A cell of a grid or table.
    Cell,
    /// A column header of a grid or table.
    ColumnHeader,
    /// A list of items.
    List,
    /// One item of a list.
    ListItem,
    /// An image; its name is the alternative text.
    Image,
    /// Progress of a task.
    ProgressIndicator,
    /// A visual divider.
    Separator,
    /// A region that scrolls.
    ScrollView,
    /// A date grid.
    Calendar,
    /// The band that ends a page: copyright, links, version (a landmark).
    ContentInfo,
    /// A set of links or buttons that move around the app (a landmark).
    Navigation,
}

impl Role {
    /// Roles whose accessible name comes from the text inside them when no
    /// explicit name is given, as in ARIA's "name from content".
    pub fn takes_name_from_content(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::Link
                | Self::CheckBox
                | Self::Switch
                | Self::RadioButton
                | Self::Heading
                | Self::ListBoxOption
                | Self::MenuItem
                | Self::Tab
                | Self::Cell
                | Self::ColumnHeader
                | Self::ListItem
                | Self::Tooltip
                | Self::Label
        )
    }
}

/// State of a check box, switch or toggle button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Toggled {
    /// Off or unchecked.
    False,
    /// On or checked.
    True,
    /// Partly checked (a parent of mixed children).
    Mixed,
}

impl From<bool> for Toggled {
    fn from(on: bool) -> Self {
        if on { Self::True } else { Self::False }
    }
}

/// A numeric value with its range (sliders, spin buttons, progress).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericValue {
    /// The current value.
    pub value: f64,
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// The amount one arrow key changes the value by, if any.
    pub step: Option<f64>,
}

/// What assistive technology can ask a node to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccessAction {
    /// Activate it, as a click or Enter would.
    Click,
    /// Move keyboard focus to it.
    Focus,
    /// Raise its value one step.
    Increment,
    /// Lower its value one step.
    Decrement,
}

/// One node of the accessibility tree.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct AccessNode {
    /// Identity, stable across frames for the same widget.
    pub id: u64,
    /// What the node is.
    pub role: Role,
    /// The accessible name: a label, or the text inside for roles that take
    /// their name from content.
    pub name: Option<String>,
    /// Extra description, read after the name.
    pub description: Option<String>,
    /// A text value (text fields, combo boxes, sliders with formatted values).
    pub value: Option<String>,
    /// A numeric value with its range.
    pub numeric: Option<NumericValue>,
    /// Checked state.
    pub toggled: Option<Toggled>,
    /// Selected state (tabs, options, rows, days).
    pub selected: Option<bool>,
    /// Expanded state of something that opens (menus, combo boxes, disclosures).
    pub expanded: Option<bool>,
    /// The control does not respond to input.
    pub disabled: bool,
    /// Keyboard focus can be on it.
    pub focusable: bool,
    /// While it is open, the rest of the app is inert (dialogs).
    pub modal: bool,
    /// Heading level, 1 to 6.
    pub level: Option<u32>,
    /// The option highlighted inside a focused text field that drives a list
    /// (combobox pattern): screen readers announce it as if it had focus.
    pub active_descendant: Option<u64>,
    /// Absolute frame in logical pixels.
    pub bounds: Rect,
    /// Child node ids in reading order.
    pub children: Vec<u64>,
    /// Actions assistive technology may request.
    pub actions: Vec<AccessAction>,
}

impl AccessNode {
    /// A node with no name, state, children or actions.
    pub fn new(id: u64, role: Role, bounds: Rect) -> Self {
        Self {
            id,
            role,
            name: None,
            description: None,
            value: None,
            numeric: None,
            toggled: None,
            selected: None,
            expanded: None,
            disabled: false,
            focusable: false,
            modal: false,
            level: None,
            active_descendant: None,
            bounds,
            children: Vec::new(),
            actions: Vec::new(),
        }
    }
}

/// The whole accessibility tree of one window, as of the latest frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessTree {
    /// Id of the root node (role [`Role::Window`]).
    pub root: u64,
    /// Every node, root first, parents before children.
    pub nodes: Vec<AccessNode>,
    /// The node with keyboard focus, if any.
    pub focus: Option<u64>,
}

impl AccessTree {
    /// The node with this id.
    pub fn get(&self, id: u64) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The first node with this role and name, for tests and tools.
    pub fn find(&self, role: Role, name: &str) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name))
    }

    /// The node with keyboard focus.
    pub fn focused(&self) -> Option<&AccessNode> {
        self.focus.and_then(|id| self.get(id))
    }
}
