//! The components as the playground's interpreter sees them (feature `live`): every exposed
//! type, how views become an [`AnyView`], adapters for the few generic functions an example
//! uses, and a [`Runner`] for the [`Playground`](crate::Playground).

use std::rc::Rc;

use ravenrs_live::{Fallback, FromLive, Registry, Value, bridge};
use ravenrs_reactive::State;

use crate::{AnyView, CodeDiagnostic, Runner, Spacer, Text, View};

/// How a view of type `T` goes where an [`AnyView`] goes.
pub(crate) fn view<T: View + 'static>(registry: &mut Registry) {
    registry.conversion::<T, AnyView>(bridge::host_name::<T>(), |view| Box::new(view));
}

impl FromLive for AnyView {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Host(host) => host.take_as::<AnyView>(),
            // A bare text where a view goes reads as the text, as `impl Into<String>` would.
            Value::Str(text) => Ok(Box::new(Text::new(text))),
            Value::Unit | Value::Option(None) => Ok(Box::new(Spacer::new())),
            other => Err(format!("expected a view, found {}", other.kind())),
        }
    }
}

impl Fallback for AnyView {
    fn fallback() -> Self {
        Box::new(Spacer::new())
    }
}

/// `Text::bound(&state, |value| ..)`: generic over the state's type, so written out here.
fn generic_functions(registry: &mut Registry) {
    registry.function("Text", "bound", |args: Vec<Value>| {
        let [state, format] =
            <[Value; 2]>::try_from(args).map_err(|_| "`bound` takes a state and a closure")?;
        let state: State<Value> = State::<Value>::from_live(state)?;
        let Value::Closure(format) = format.read() else { return Err("`bound` takes a closure".into()) };
        let text = Text::bound(&state, move |value: &Value| match format.call(vec![value.clone()]) {
            Ok(Value::Str(text)) => text,
            Ok(other) => format!("{other:?}"),
            Err(problem) => problem.message,
        });
        Ok(Value::Host(ravenrs_live::Host::new("Text", text)))
    });
}

/// Methods every view has, from `ViewExt`, kept under `View` for any host view.
fn view_methods(registry: &mut Registry) {
    use crate::ViewExt;
    use ravenrs_core::Insets;

    fn this(args: &[Value]) -> Result<AnyView, String> {
        AnyView::from_live(args.first().cloned().unwrap_or(Value::Unit))
    }
    fn arg<T: FromLive>(args: &[Value], i: usize) -> Result<T, String> {
        T::from_live(args.get(i).cloned().ok_or("an argument is missing")?)
    }
    fn styled(view: crate::Styled<AnyView>) -> Result<Value, String> {
        Ok(Value::Host(ravenrs_live::Host::new("Styled", view)))
    }
    registry.method("View", "padding", |a| styled(this(&a)?.padding(arg::<Insets>(&a, 1)?)));
    registry.method("View", "background", |a| {
        let fill = match a.get(1).map(Value::read) {
            Some(Value::Host(host)) => host.take_as::<crate::Fill>()?,
            _ => return Err("a color or a semantic color goes here".to_owned()),
        };
        styled(this(&a)?.background(fill))
    });
    registry.method("View", "frame", |a| styled(this(&a)?.frame(arg(&a, 1)?, arg(&a, 2)?)));
    registry.method("View", "width", |a| styled(this(&a)?.width(arg(&a, 1)?)));
    registry.method("View", "height", |a| styled(this(&a)?.height(arg(&a, 1)?)));
    registry.method("View", "grow", |a| styled(this(&a)?.grow(arg(&a, 1)?)));
    registry.method("View", "stretch", |a| styled(this(&a)?.stretch()));
    registry.method("View", "centered", |a| styled(this(&a)?.centered()));
    registry.method("View", "tooltip", |a| {
        let view = this(&a)?.tooltip(arg::<String>(&a, 1)?);
        Ok(Value::Host(ravenrs_live::Host::new("Tooltip", view)))
    });
    registry.conversion::<ravenrs_style::Semantic, crate::Fill>("Semantic", crate::Fill::from);
    registry.conversion::<ravenrs_core::Color, crate::Fill>("Color", crate::Fill::from);
    // `view_fn(|| ..)` builds its view each time, by calling the closure again.
    registry.free_function("view_fn", |args: Vec<Value>| {
        let Some(Value::Closure(make)) = args.first().map(Value::read) else {
            return Err("`view_fn` takes a closure".into());
        };
        let view = crate::view_fn(move || {
            match make.call(Vec::new()).and_then(|v| AnyView::from_live(v).map_err(|e| make.error(e))) {
                Ok(view) => view,
                Err(problem) => {
                    let shown: AnyView = Box::new(Text::new(problem.message));
                    shown
                }
            }
        });
        let view: AnyView = Box::new(view);
        Ok(Value::Host(ravenrs_live::Host::new("AnyView", view)))
    });
}

/// `Canvas::new(|frame, scene| ..)`: the closure draws into a scene of its own, which is then
/// added to the real one, as a `&mut Scene` cannot leave the paint.
fn canvas(registry: &mut Registry) {
    use ravenrs_core::{Rect, Scene};
    registry.function("Canvas", "new", |args: Vec<Value>| {
        let Some(Value::Closure(draw)) = args.first().map(Value::read) else {
            return Err("`Canvas::new` takes a closure of the frame and the scene".into());
        };
        let canvas = crate::Canvas::new(move |frame: Rect, scene: &mut Scene| {
            let own = ravenrs_live::Host::new("Scene", Scene::new());
            let frame = Value::Host(ravenrs_live::Host::cloneable("Rect", frame));
            if let Err(problem) = draw.call(vec![frame, Value::Host(own.clone())]) {
                ravenrs_live::report(problem);
            }
            if let Ok(drawn) = own.take::<Scene>() {
                scene.extend_from_slice(drawn.primitives());
            }
        });
        Ok(Value::Host(ravenrs_live::Host::new("Canvas", canvas)))
    });
}

/// Everything the interpreter can reach in the components and the crates under them.
pub fn live_registry() -> Registry {
    let mut registry = Registry::new();
    crate::live_list::expose_all(&mut registry);
    // Made by a `macro_rules!`, so out of the generated list's sight.
    crate::VStack::<Vec<AnyView>>::__live_expose(&mut registry);
    crate::HStack::<Vec<AnyView>>::__live_expose(&mut registry);
    view::<crate::VStack<Vec<AnyView>>>(&mut registry);
    view::<crate::HStack<Vec<AnyView>>>(&mut registry);
    generic_functions(&mut registry);
    view_methods(&mut registry);
    canvas(&mut registry);
    registry
}

/// Runs the code of a [`Playground`](crate::Playground) with the interpreter: its first
/// function, which returns `impl View`, is called and shown. The program's own `use` lines
/// are read and left alone; what it does not read comes back as diagnostics with their place.
///
/// ```
/// use ravenrs_widgets::{LiveRunner, Runner};
///
/// let view = LiveRunner::new().run(r#"fn hello() -> impl View { Text::new("hello") }"#);
/// assert!(view.is_ok());
/// ```
#[derive(Clone)]
pub struct LiveRunner {
    registry: Rc<Registry>,
}

impl std::fmt::Debug for LiveRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveRunner").finish_non_exhaustive()
    }
}

impl Default for LiveRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveRunner {
    /// A runner over [`live_registry`].
    pub fn new() -> Self {
        Self::with_registry(live_registry())
    }

    /// A runner over a registry of the app's own, such as one with more types added.
    pub fn with_registry(registry: Registry) -> Self {
        Self { registry: Rc::new(registry) }
    }
}

fn diagnostic(error: ravenrs_live::LiveError) -> CodeDiagnostic {
    CodeDiagnostic::error(error.line, error.column, error.length, error.message)
}

impl Runner for LiveRunner {
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
        let value = ravenrs_live::run(source, None, self.registry.clone())
            .map_err(|errors| errors.into_iter().map(diagnostic).collect::<Vec<_>>())?;
        AnyView::from_live(value).map_err(|message| vec![CodeDiagnostic::error(1, 1, 1, message)])
    }
}
