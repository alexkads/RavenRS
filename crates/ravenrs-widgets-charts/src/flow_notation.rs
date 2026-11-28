//! `FlowNotation`: a notation for diagrams (its elements, how they look, how they join, what
//! makes a diagram right, the files it is kept in) handed to the diagram components at once.

use std::fmt;
use std::rc::Rc;

use crate::flow::{
    FlowConnector, FlowEdge, FlowGraph, FlowKind, FlowNext, FlowNode, FlowValidator, PortSide,
};
use crate::flow::{FlowEndpoint, FlowIssue};

/// Makes a diagram of a file's text.
type Reader = Rc<dyn Fn(&str) -> Result<FlowGraph, String>>;
/// Makes the text of a diagram, given its name.
type Writer = Rc<dyn Fn(&FlowGraph, &str) -> String>;

/// A file format a [`FlowLibrary`](crate::FlowLibrary) opens and saves diagrams in besides its own JSON, such as
/// a notation's XML: what it is called, its extension, and how a diagram is read from it and
/// written to it.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowFileFormat {
    pub(crate) name: String,
    pub(crate) extension: String,
    pub(crate) read: Reader,
    pub(crate) write: Writer,
}

impl fmt::Debug for FlowFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowFileFormat")
            .field("name", &self.name)
            .field("extension", &self.extension)
            .finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowFileFormat {
    /// A format called `name`, in files ending in `.extension`: `read` makes a diagram of a
    /// file's text, or says why it cannot; `write` makes the text of a diagram, given its name.
    pub fn new(
        name: impl Into<String>,
        extension: impl Into<String>,
        read: impl Fn(&str) -> Result<FlowGraph, String> + 'static,
        write: impl Fn(&FlowGraph, &str) -> String + 'static,
    ) -> Self {
        Self { name: name.into(), extension: extension.into(), read: Rc::new(read), write: Rc::new(write) }
    }
}

/// Says what is wrong with a diagram, for [`FlowCanvas::checks`](crate::FlowCanvas::checks).
pub type FlowChecks = Rc<dyn Fn(&FlowGraph) -> Vec<FlowIssue>>;

/// A notation for diagrams, such as a process notation or a flowchart's: the elements it has,
/// grouped for a palette, how each kind looks and is called, how connections between them are
/// drawn, which ones may be made, what comes next from the keys, what makes a diagram right and
/// the files it is kept in.
///
/// It is data: give it to [`FlowCanvas::notation`](crate::FlowCanvas::notation),
/// [`FlowToolbox::notation`](crate::FlowToolbox::notation),
/// [`FlowInspector::notation`](crate::FlowInspector::notation) and
/// [`FlowLibrary::notation`](crate::FlowLibrary::notation), and each takes the part it uses.
///
/// ```
/// use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowNode, FlowNotation, FlowShape, FlowToolbox};
/// use ravenrs_reactive::State;
///
/// let notation = FlowNotation::new()
///     .model("Steps", "Step", FlowNode::new("step", "step", "Step", 0.0, 0.0))
///     .model("Steps", "Choice", FlowNode::new("choice", "choice", "Choice?", 0.0, 0.0).shape(FlowShape::Diamond))
///     .kind("step", "Step")
///     .kind("choice", "Choice");
/// let graph = State::new(FlowGraph::new());
/// let canvas = FlowCanvas::new(graph.binding()).notation(&notation);
/// let palette = FlowToolbox::new().notation(&notation);
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowNotation {
    pub(crate) kinds: Vec<(String, String)>,
    pub(crate) looks: Vec<(String, FlowKind)>,
    pub(crate) models: Vec<(String, String, FlowNode)>,
    pub(crate) connector: Option<FlowConnector>,
    pub(crate) validate: Option<FlowValidator>,
    pub(crate) next: Option<FlowNext>,
    pub(crate) checks: Option<FlowChecks>,
    pub(crate) runs: Option<crate::flow::FlowRunRules>,
    pub(crate) formats: Vec<FlowFileFormat>,
    pub(crate) cables: Option<bool>,
}

impl fmt::Debug for FlowNotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowNotation").field("kinds", &self.kinds).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowNotation {
    /// A notation with nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// A kind of element, by its name in the diagram and the words shown for it.
    pub fn kind(mut self, kind: impl Into<String>, caption: impl Into<String>) -> Self {
        self.kinds.push((kind.into(), caption.into()));
        self
    }

    /// How the cards of a kind look: their icon, word and tone.
    pub fn look(mut self, kind: impl Into<String>, look: FlowKind) -> Self {
        self.looks.push((kind.into(), look));
        self
    }

    /// An element for the palette, in `group`, called `caption`, whose copies are `node`.
    pub fn model(mut self, group: impl Into<String>, caption: impl Into<String>, node: FlowNode) -> Self {
        self.models.push((group.into(), caption.into(), node));
        self
    }

    /// Shapes each connection made, as [`FlowCanvas::connection`](crate::FlowCanvas::connection).
    pub fn connection(mut self, shape: impl Fn(&FlowGraph, FlowEdge) -> FlowEdge + 'static) -> Self {
        self.connector = Some(Rc::new(shape));
        self
    }

    /// Which connections may be made, as [`FlowCanvas::validate`](crate::FlowCanvas::validate).
    pub fn validate(
        mut self,
        rule: impl Fn(&FlowGraph, &FlowEndpoint, &FlowEndpoint) -> Result<(), String> + 'static,
    ) -> Self {
        self.validate = Some(Rc::new(rule));
        self
    }

    /// What comes next from the keys and the arrows, as
    /// [`FlowCanvas::next_node`](crate::FlowCanvas::next_node).
    pub fn next_node(mut self, next: impl Fn(&FlowNode, PortSide) -> Option<FlowNode> + 'static) -> Self {
        self.next = Some(Rc::new(next));
        self
    }

    /// What makes a diagram right, as [`FlowCanvas::checks`](crate::FlowCanvas::checks).
    pub fn checks(mut self, checks: impl Fn(&FlowGraph) -> Vec<FlowIssue> + 'static) -> Self {
        self.checks = Some(Rc::new(checks));
        self
    }

    /// How a diagram of it runs, as [`FlowCanvas::runs`](crate::FlowCanvas::runs).
    pub fn runs(mut self, rules: crate::flow::FlowRunRules) -> Self {
        self.runs = Some(rules);
        self
    }

    /// Whether its connections are cables, as [`FlowCanvas::cables`](crate::FlowCanvas::cables).
    pub fn cables(mut self, cables: bool) -> Self {
        self.cables = Some(cables);
        self
    }

    /// A file format its diagrams are kept in, as [`FlowLibrary::format`](crate::FlowLibrary::format).
    pub fn format(mut self, format: FlowFileFormat) -> Self {
        self.formats.push(format);
        self
    }

    /// The words shown for a kind, if the notation has it.
    pub fn caption(&self, kind: &str) -> Option<&str> {
        self.kinds.iter().find(|(k, _)| k == kind).map(|(_, c)| c.as_str())
    }
}
