//! [`FlowCanvasDriver`]: what a test does to a `FlowCanvas` as a person does it, by the keys
//! of nodes and ports, never by pixels or by reading the diagram's state.
//!
//! It only uses `ravenrs-e2e`'s public locators and actions on the ids the canvas gives a test:
//! `node:<node>` on each node and `port:<node>:<port>` on each port.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_widgets_charts::testing::FlowCanvasDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! FlowCanvasDriver::new(e2e).connect("pedido.out", "aprovar.in")?;
//! # Ok(()) }
//! ```

use ravenrs_e2e::{E2e, E2eResult, ErrorCode, Query};

/// Joins and moves the nodes of a `FlowCanvas` in an [`E2e`] session.
pub struct FlowCanvasDriver<'a> {
    e2e: &'a mut E2e,
}

impl std::fmt::Debug for FlowCanvasDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlowCanvasDriver").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<'a> FlowCanvasDriver<'a> {
    /// Drives the canvas on the screen. Nodes and ports have keys unique to their diagram, so
    /// two canvases on one screen need different keys.
    pub fn new(e2e: &'a mut E2e) -> Self {
        Self { e2e }
    }

    /// Joins the port `from` to the port `to`, each written `node.port`, by dragging one to the
    /// other as a person does. The pointer passes over the first node first, so a canvas with
    /// quiet ports shows them; the other ports show while the drag lasts.
    pub fn connect(&mut self, from: &str, to: &str) -> E2eResult {
        let (from_node, from_port) = self.split(from)?;
        let (to_node, to_port) = self.split(to)?;
        self.e2e.by_test_id(format!("node:{from_node}")).hover()?;
        self.e2e
            .by_test_id(format!("port:{from_node}:{from_port}"))
            .drag_to(&Query::test_id(format!("port:{to_node}:{to_port}")))
    }

    /// Drags the node with key `node` onto the part `to` finds.
    pub fn move_node(&mut self, node: &str, to: &Query) -> E2eResult {
        self.e2e.by_test_id(format!("node:{node}")).drag_to(to)
    }

    /// `node.port` as its two keys; the port is after the last dot, so a node's key may have dots.
    fn split<'s>(&self, end: &'s str) -> E2eResult<(&'s str, &'s str)> {
        match end.rsplit_once('.') {
            Some((node, port)) if !node.is_empty() && !port.is_empty() => Ok((node, port)),
            _ => Err(self
                .e2e
                .failure(ErrorCode::NotFound, format!("{end:?} does not say node.port"))
                .hint("write the node's key, a dot and the port's id, such as pedido.out")),
        }
    }
}
