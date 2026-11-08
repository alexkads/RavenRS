//! What the interpreter can call in the host program: associated functions such as
//! `Button::new`, methods such as `.style(..)`, constants such as `ButtonStyle::Ghost`, and
//! conversions such as from a `Button` to an `AnyView`.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::value::{FromLive, Host, IntoLive, Value};

/// A function of the host program, taking the interpreter's values. For a method, the first
/// value is the receiver.
pub type HostFn = Rc<dyn Fn(Vec<Value>) -> Result<Value, String>>;

type Convert = Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>;

/// A method, with the Rust type its receiver holds when it is one of several of that name.
type Method = (Option<TypeId>, HostFn);

/// Reads a field of a host value.
pub type FieldGet = Rc<dyn Fn(&Value) -> Result<Value, String>>;

/// Changes a field of a host value in place.
pub type FieldSet = Rc<dyn Fn(&Value, Value) -> Result<(), String>>;

/// Builds a host struct from its fields, written as `Name { a: .., ..base }`.
pub type RecordFn = Rc<dyn Fn(Vec<(String, Value)>, Option<Value>) -> Result<Value, String>>;

/// Everything of the host program an interpreted program can reach.
#[derive(Default)]
pub struct Registry {
    functions: HashMap<(String, String), HostFn>,
    methods: HashMap<(String, String), Vec<Method>>,
    constants: HashMap<(String, String), Rc<dyn Fn() -> Value>>,
    free: HashMap<String, HostFn>,
    conversions: HashMap<(TypeId, TypeId), Convert>,
    aliases: HashMap<String, String>,
    names: HashMap<TypeId, &'static str>,
    fields: HashMap<(String, String), (Option<FieldGet>, Option<FieldSet>)>,
    records: HashMap<String, RecordFn>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("functions", &self.functions.len())
            .field("methods", &self.methods.len())
            .field("constants", &self.constants.len())
            .finish_non_exhaustive()
    }
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// `Type::name(..)`, taking the arguments as they are.
    pub fn function(
        &mut self,
        ty: &str,
        name: &str,
        f: impl Fn(Vec<Value>) -> Result<Value, String> + 'static,
    ) {
        self.functions.insert((ty.to_owned(), name.to_owned()), Rc::new(f));
    }

    /// `value.name(..)` on a host value of `ty`; the receiver comes first.
    pub fn method(
        &mut self,
        ty: &str,
        name: &str,
        f: impl Fn(Vec<Value>) -> Result<Value, String> + 'static,
    ) {
        self.methods.entry((ty.to_owned(), name.to_owned())).or_default().push((None, Rc::new(f)));
    }

    /// `value.name(..)` on a host value holding a `T`, for types that share a name, such as
    /// the forms of a generic component.
    pub fn method_for<T: 'static>(
        &mut self,
        ty: &str,
        name: &str,
        f: impl Fn(Vec<Value>) -> Result<Value, String> + 'static,
    ) {
        let list = self.methods.entry((ty.to_owned(), name.to_owned())).or_default();
        list.retain(|(id, _)| *id != Some(TypeId::of::<T>()));
        list.push((Some(TypeId::of::<T>()), Rc::new(f)));
    }

    /// `Type::NAME`, made each time it is read.
    pub fn constant(&mut self, ty: &str, name: &str, make: impl Fn() -> Value + 'static) {
        self.constants.insert((ty.to_owned(), name.to_owned()), Rc::new(make));
    }

    /// A function called by its name alone, such as a helper the samples share.
    pub fn free_function(&mut self, name: &str, f: impl Fn(Vec<Value>) -> Result<Value, String> + 'static) {
        self.free.insert(name.to_owned(), Rc::new(f));
    }

    /// How `value.name` reads a field of a host struct of `ty`.
    pub fn field(&mut self, ty: &str, name: &str, get: impl Fn(&Value) -> Result<Value, String> + 'static) {
        self.fields.entry((ty.to_owned(), name.to_owned())).or_default().0 = Some(Rc::new(get));
    }

    /// How `value.name = ..` changes a field of a host struct of `ty`.
    pub fn set_field(
        &mut self,
        ty: &str,
        name: &str,
        set: impl Fn(&Value, Value) -> Result<(), String> + 'static,
    ) {
        self.fields.entry((ty.to_owned(), name.to_owned())).or_default().1 = Some(Rc::new(set));
    }

    /// How `Ty { a: .., b: .., ..base }` builds a host struct.
    pub fn record(
        &mut self,
        ty: &str,
        build: impl Fn(Vec<(String, Value)>, Option<Value>) -> Result<Value, String> + 'static,
    ) {
        self.records.insert(ty.to_owned(), Rc::new(build));
    }

    pub(crate) fn find_field(&self, ty: &str, name: &str) -> Option<FieldGet> {
        self.fields.get(&(ty.to_owned(), name.to_owned()))?.0.clone()
    }

    pub(crate) fn find_set_field(&self, ty: &str, name: &str) -> Option<FieldSet> {
        self.fields.get(&(ty.to_owned(), name.to_owned()))?.1.clone()
    }

    pub(crate) fn find_record(&self, ty: &str) -> Option<RecordFn> {
        self.records.get(self.named(ty)).cloned()
    }

    /// Remembers which name a Rust type goes by, for conversions.
    pub fn name_type<T: 'static>(&mut self, name: &'static str) {
        self.names.insert(TypeId::of::<T>(), name);
    }

    /// How a host value of type `From` becomes a `To` when a function asks for a `To`, as a
    /// `Button` becomes an `AnyView`.
    pub fn conversion<From: 'static, To: 'static>(&mut self, name: &'static str, convert: fn(From) -> To) {
        self.name_type::<From>(name);
        let convert: Convert = Rc::new(move |any: Box<dyn Any>| -> Box<dyn Any> {
            match any.downcast::<From>() {
                Ok(value) => Box::new(convert(*value)),
                Err(other) => other,
            }
        });
        self.conversions.insert((TypeId::of::<From>(), TypeId::of::<To>()), convert);
    }

    /// Lets `alias` name the type registered as `ty`, as a crate re-exports `Stack as ChartStack`.
    pub fn alias(&mut self, alias: &str, ty: &str) {
        self.aliases.insert(alias.to_owned(), ty.to_owned());
    }

    fn named<'a>(&'a self, ty: &'a str) -> &'a str {
        self.aliases.get(ty).map_or(ty, String::as_str)
    }

    pub(crate) fn find_function(&self, ty: &str, name: &str) -> Option<HostFn> {
        self.functions.get(&(self.named(ty).to_owned(), name.to_owned())).cloned()
    }

    pub(crate) fn find_method(&self, ty: &str, name: &str) -> Option<HostFn> {
        self.methods.get(&(ty.to_owned(), name.to_owned()))?.first().map(|(_, f)| f.clone())
    }

    /// The method for a receiver holding a value of `held`, or the first of that name.
    pub(crate) fn find_method_for(&self, ty: &str, name: &str, held: Option<TypeId>) -> Option<HostFn> {
        let list = self.methods.get(&(ty.to_owned(), name.to_owned()))?;
        list.iter()
            .find(|(id, _)| id.is_some() && *id == held)
            .or_else(|| list.first())
            .map(|(_, f)| f.clone())
    }

    pub(crate) fn find_constant(&self, ty: &str, name: &str) -> Option<Value> {
        self.constants.get(&(self.named(ty).to_owned(), name.to_owned())).map(|make| make())
    }

    pub(crate) fn find_free(&self, name: &str) -> Option<HostFn> {
        self.free.get(name).cloned()
    }

    /// Whether anything is known under this type name.
    pub(crate) fn knows_type(&self, ty: &str) -> bool {
        let ty = self.named(ty);
        self.functions.keys().chain(self.constants.keys()).any(|(t, _)| t == ty)
            || self.records.contains_key(ty)
    }

    fn convert(&self, held: TypeId, target: TypeId) -> Option<Convert> {
        self.conversions.get(&(held, target)).cloned()
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Registry>>> = const { RefCell::new(None) };
}

/// Makes `registry` the one conversions use on this thread while `f` runs, and after, for
/// closures the host calls later.
pub(crate) fn use_registry(registry: Rc<Registry>) {
    CURRENT.with(|current| *current.borrow_mut() = Some(registry));
}

fn current() -> Option<Rc<Registry>> {
    CURRENT.with(|current| current.borrow().clone())
}

impl Host {
    /// Takes the value out as a `T`: the same type, or one the registry converts to `T`.
    pub fn take_as<T: 'static>(&self) -> Result<T, String> {
        if let Ok(value) = self.take::<T>() {
            return Ok(value);
        }
        let convert = current()
            .zip(self.held_type())
            .and_then(|(registry, held)| registry.convert(held, TypeId::of::<T>()))
            .ok_or_else(|| {
                format!(
                    "a {} cannot go where a {} goes",
                    self.type_name,
                    crate::value::short(std::any::type_name::<T>())
                )
            })?;
        // A value that copies stays where it is, as `take` leaves it.
        let boxed = match self.copy {
            Some(copy) => {
                let cell = self.cell.borrow();
                let any = cell.as_ref().ok_or_else(|| format!("this {} was already used", self.type_name))?;
                copy(any.as_ref())
            }
            None => self
                .cell
                .borrow_mut()
                .take()
                .ok_or_else(|| format!("this {} was already used", self.type_name))?,
        };
        convert(boxed)
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| format!("a {} did not convert", self.type_name))
    }
}

/// A value the host can always fall back to when a closure it calls fails, so the failure is
/// reported instead of stopping the program.
pub trait Fallback {
    /// The value to use instead.
    fn fallback() -> Self;
}

macro_rules! defaults {
    ($($ty:ty),*) => {$(impl Fallback for $ty { fn fallback() -> Self { <$ty>::default() } })*};
}

defaults!((), bool, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize, f32, f64, char, String);

impl<T> Fallback for Option<T> {
    fn fallback() -> Self {
        None
    }
}

impl<T, E: Fallback> Fallback for Result<T, E> {
    fn fallback() -> Self {
        Err(E::fallback())
    }
}

impl<T> Fallback for Vec<T> {
    fn fallback() -> Self {
        Vec::new()
    }
}

macro_rules! callbacks {
    ($(($($arg:ident),*)),*) => {$(
        impl<$($arg: IntoLive + 'static,)* R: FromLive + Fallback + 'static> FromLive for Box<dyn Fn($($arg),*) -> R> {
            #[allow(non_snake_case)]
            fn from_live(value: Value) -> Result<Self, String> {
                let Value::Closure(closure) = value.read() else {
                    return Err(format!("expected a closure, found {}", value.kind()));
                };
                Ok(Box::new(move |$($arg: $arg),*| {
                    let arguments = vec![$($arg.into_live()),*];
                    match closure.call(arguments).and_then(|v| R::from_live(v).map_err(|e| closure.error(e))) {
                        Ok(result) => result,
                        Err(problem) => {
                            crate::value::report(problem);
                            R::fallback()
                        }
                    }
                }))
            }
        }
        impl<$($arg: IntoLive + 'static,)* R: FromLive + Fallback + 'static> FromLive for Rc<dyn Fn($($arg),*) -> R> {
            fn from_live(value: Value) -> Result<Self, String> {
                Box::<dyn Fn($($arg),*) -> R>::from_live(value).map(Rc::from)
            }
        }
    )*};
}

callbacks!((), (A), (A, B), (A, B, C));

impl<T: FromLive + IntoLive + Clone + 'static> FromLive for ravenrs_reactive::Binding<T> {
    fn from_live(value: Value) -> Result<Self, String> {
        let Some(state) = value.as_binding() else {
            return Err(format!("expected a State or its binding, found {}", value.kind()));
        };
        let first = T::from_live(state.get())?;
        let last = Rc::new(RefCell::new(first));
        let (read, write, kept) = (state.clone(), state, last.clone());
        Ok(ravenrs_reactive::Binding::new(
            move || match T::from_live(read.get()) {
                Ok(value) => {
                    *last.borrow_mut() = value.clone();
                    value
                }
                Err(_) => last.borrow().clone(),
            },
            move |value: T| {
                *kept.borrow_mut() = value.clone();
                write.set(value.into_live());
            },
        ))
    }
}
