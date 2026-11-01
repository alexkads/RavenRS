use crate::context::{BuildCx, NodeId};

/// Anything that can appear in the UI.
///
/// `build` runs whenever the UI rebuilds. It must be cheap and side-effect free:
/// read state, create layout boxes and paint instructions through `cx`, and
/// return the node that represents this view.
pub trait View {
    /// Creates this view's node for the current build.
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId;
}

/// A component composed from other views, SwiftUI style.
///
/// Keep state in the struct's fields (e.g. `State<T>`, created once in a
/// constructor); `body` is called on every rebuild and must not create state.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Button, Component, Text, VStack, View};
///
/// struct Counter {
///     count: State<i32>,
/// }
///
/// impl Component for Counter {
///     fn body(&self) -> impl View {
///         let count = self.count.clone();
///         VStack::new((
///             Text::bound(&self.count, |c| format!("Count: {c}")),
///             Button::new("Increment").on_click(move || count.update(|c| *c += 1)),
///         ))
///     }
/// }
/// ```
pub trait Component {
    /// The views this component is made of; called on every rebuild.
    fn body(&self) -> impl View;
}

impl<C: Component> View for C {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let body = self.body();
        cx.child(&body)
    }
}

/// A type-erased view, for heterogeneous lists and dynamic UI.
pub type AnyView = Box<dyn View>;

impl View for Box<dyn View> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        (**self).build(cx)
    }
}

/// Renders nothing when `None`, so `Option<V>` expresses conditional UI.
impl<V: View> View for Option<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        match self {
            Some(v) => v.build(cx),
            None => cx.empty(),
        }
    }
}

/// Gives a view an explicit identity, like SwiftUI's `.id()`: when the key
/// changes, the view is treated as new (its local state and scroll position
/// start over).
#[derive(Debug)]
pub struct Keyed<V> {
    key: u64,
    view: V,
}

impl<V: View> Keyed<V> {
    /// Wraps `view` with identity `key`.
    pub fn new(key: u64, view: V) -> Self {
        Self { key, view }
    }
}

impl<V: View> View for Keyed<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.keyed_child(self.key, &self.view)
    }
}

/// A view built from the viewport size; see [`responsive`].
pub struct Responsive<F>(F);

impl<F> std::fmt::Debug for Responsive<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Responsive")
    }
}

/// Builds a view from the viewport size (in logical pixels), like SwiftUI's
/// `GeometryReader`: use it to switch layouts at breakpoints.
///
/// ```
/// use ravenrs_widgets::{HStack, Text, VStack, View, responsive, AnyView};
///
/// let layout = responsive(|size| -> AnyView {
///     if size.width < 600.0 { Box::new(VStack::new((Text::new("A"), Text::new("B")))) }
///     else { Box::new(HStack::new((Text::new("A"), Text::new("B")))) }
/// });
/// ```
pub fn responsive<F, V>(f: F) -> Responsive<F>
where
    F: Fn(ravenrs_core::Size) -> V,
    V: View,
{
    Responsive(f)
}

impl<F, V> View for Responsive<F>
where
    F: Fn(ravenrs_core::Size) -> V,
    V: View,
{
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let view = (self.0)(cx.viewport());
        cx.child(&view)
    }
}

/// A view produced by a closure on every build; see [`view_fn`].
pub struct FnView<F>(F);

impl<F> std::fmt::Debug for FnView<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FnView")
    }
}

/// Wraps a closure as a view. Whatever state the closure reads is tracked, so
/// this is the simplest way to make a region of the UI depend on state:
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Text, view_fn};
///
/// let logged_in = State::new(false);
/// let l = logged_in.clone();
/// let greeting = view_fn(move || Text::new(if l.get() { "Welcome back" } else { "Sign in" }));
/// ```
pub fn view_fn<F, V>(f: F) -> FnView<F>
where
    F: Fn() -> V + 'static,
    V: View,
{
    FnView(f)
}

impl<F, V> View for FnView<F>
where
    F: Fn() -> V,
    V: View,
{
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let view = (self.0)();
        cx.child(&view)
    }
}

/// An ordered collection of child views: tuples of up to 12 views, `Vec<V>`
/// and `Vec<AnyView>`.
pub trait ViewSeq {
    /// Builds each child in order and appends its node to `out`.
    fn build_each(&self, cx: &mut BuildCx<'_>, out: &mut Vec<NodeId>);
}

impl<V: View> ViewSeq for Vec<V> {
    fn build_each(&self, cx: &mut BuildCx<'_>, out: &mut Vec<NodeId>) {
        for v in self {
            out.push(cx.child(v));
        }
    }
}

macro_rules! impl_view_seq_for_tuple {
    ($($name:ident),+) => {
        impl<$($name: View),+> ViewSeq for ($($name,)+) {
            #[allow(non_snake_case)]
            fn build_each(&self, cx: &mut BuildCx<'_>, out: &mut Vec<NodeId>) {
                let ($($name,)+) = self;
                $(out.push(cx.child($name));)+
            }
        }
    };
}

impl_view_seq_for_tuple!(A);
impl_view_seq_for_tuple!(A, B);
impl_view_seq_for_tuple!(A, B, C);
impl_view_seq_for_tuple!(A, B, C, D);
impl_view_seq_for_tuple!(A, B, C, D, E);
impl_view_seq_for_tuple!(A, B, C, D, E, F);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G, H);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G, H, I);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_view_seq_for_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
