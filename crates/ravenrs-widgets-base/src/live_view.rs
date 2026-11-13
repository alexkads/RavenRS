//! How a view from the playground's interpreter goes where an [`AnyView`] goes: a host value that
//! holds one, a bare text, or a struct of the program with `impl View for ..`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_live::{Fallback, FromLive, Host, Record, Value};
use ravenrs_reactive::State;

use crate::context::{BuildCx, NodeId};
use crate::{AnyView, Spacer, Text, View};

impl FromLive for AnyView {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Host(host) => host.take_as::<AnyView>(),
            // A bare text where a view goes reads as the text, as `impl Into<String>` would.
            Value::Str(text) => Ok(Box::new(Text::new(text))),
            Value::Unit | Value::Option(None) => Ok(Box::new(Spacer::new())),
            Value::Option(Some(inner)) => AnyView::from_live(*inner),
            // A struct of the program with `impl View for ..`.
            Value::Record(record) if record.has_method("build") => Ok(Box::new(LiveView { record })),
            other => Err(format!("expected a view, found {}", other.kind())),
        }
    }
}

impl Fallback for AnyView {
    fn fallback() -> Self {
        Box::new(Spacer::new())
    }
}

/// A view the program declares itself, with `impl View for Name { fn build(&self, cx) .. }`.
/// Its `build` runs in the interpreter at every build, with a `cx` that keeps its states.
struct LiveView {
    record: Rc<Record>,
}

/// What the program's `build` gets as `cx`: its states, kept between builds in order, the time,
/// and the view it hands to `cx.child`.
#[derive(Clone)]
#[doc(hidden)]
#[allow(missing_debug_implementations)] // internal: the facade registers its methods
pub struct LiveCx {
    #[doc(hidden)]
    pub states: Rc<RefCell<Vec<State<Value>>>>,
    #[doc(hidden)]
    pub next: Rc<Cell<usize>>,
    #[doc(hidden)]
    pub now: f64,
    #[doc(hidden)]
    pub wake: Rc<Cell<Option<f64>>>,
    #[doc(hidden)]
    pub child: Rc<RefCell<Option<AnyView>>>,
}

impl View for LiveView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let states = cx.use_state(|| Rc::new(RefCell::new(Vec::<State<Value>>::new()))).get();
        let live = LiveCx {
            states,
            next: Rc::new(Cell::new(0)),
            now: cx.now(),
            wake: Rc::new(Cell::new(None)),
            child: Rc::new(RefCell::new(None)),
        };
        let given = Value::Host(Host::cloneable("BuildCx", live.clone()));
        let built = Record::call_method(&self.record, "build", vec![given]);
        if let Some(time) = live.wake.get() {
            cx.wake_at(time);
        }
        let child = live.child.borrow_mut().take();
        match (built, child) {
            (Ok(_), Some(view)) => cx.child(&*view),
            (Err(problem), _) => {
                let text = Text::new(problem.to_string());
                ravenrs_live::report(problem);
                cx.child(&text)
            }
            (Ok(_), None) => cx.child(&Text::new("`build` gives its view with `cx.child(&view)`")),
        }
    }
}
