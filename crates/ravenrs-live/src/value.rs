//! What the interpreter works with: plain values, its own closures and state, and objects
//! of the host program it only passes along.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::State;

use crate::LiveError;
use crate::eval::Closure;

/// Copies a host value, for types that can be copied.
pub(crate) type Copier = fn(&dyn Any) -> Box<dyn Any>;

/// An object of the host program: a component being built, a color, a binding. Builders take
/// it by value, so using it moves it out, as in Rust; a second use is an error.
#[derive(Clone)]
pub struct Host {
    pub(crate) type_name: &'static str,
    pub(crate) cell: Rc<RefCell<Option<Box<dyn Any>>>>,
    pub(crate) copy: Option<Copier>,
}

impl Host {
    /// Wraps a host value, known to the interpreter by `type_name`.
    pub fn new<T: 'static>(type_name: &'static str, value: T) -> Self {
        Self { type_name, cell: Rc::new(RefCell::new(Some(Box::new(value)))), copy: None }
    }

    /// Wraps a value that `.clone()` and reading by reference copy instead of moving.
    pub fn cloneable<T: Clone + 'static>(type_name: &'static str, value: T) -> Self {
        // The cell only ever holds a T, so the empty box never comes out.
        fn copy<T: Clone + 'static>(any: &dyn Any) -> Box<dyn Any> {
            match any.downcast_ref::<T>() {
                Some(value) => Box::new(value.clone()),
                None => Box::new(()),
            }
        }
        Self { copy: Some(copy::<T>), ..Self::new(type_name, value) }
    }

    /// The name the interpreter knows its type by.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Takes the value out, or copies it when it can be copied.
    pub fn take<T: 'static>(&self) -> Result<T, String> {
        if let Some(copy) = self.copy {
            let cell = self.cell.borrow();
            let any = cell.as_ref().ok_or_else(|| format!("this {} was moved", self.type_name))?;
            return copy(any.as_ref())
                .downcast::<T>()
                .map(|b| *b)
                .map_err(|_| format!("a {} is not a {}", self.type_name, std::any::type_name::<T>()));
        }
        let boxed = self.cell.borrow_mut().take().ok_or_else(|| {
            format!(
                "this {} was already used: in Rust it moved, so clone it first or build it again",
                self.type_name
            )
        })?;
        match boxed.downcast::<T>() {
            Ok(value) => Ok(*value),
            Err(back) => {
                *self.cell.borrow_mut() = Some(back);
                Err(format!("a {} is not a {}", self.type_name, short(std::any::type_name::<T>())))
            }
        }
    }

    /// A copy that shares nothing, when the type can be copied.
    pub(crate) fn duplicate(&self) -> Option<Self> {
        let copy = self.copy?;
        let cell = self.cell.borrow();
        let any = cell.as_ref()?;
        Some(Self {
            type_name: self.type_name,
            cell: Rc::new(RefCell::new(Some(copy(any.as_ref())))),
            copy: self.copy,
        })
    }
}

/// The last part of a Rust type's path, for messages.
pub(crate) fn short(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// A value of the interpreted program.
#[derive(Clone)]
pub enum Value {
    /// `()`.
    Unit,
    /// `true` or `false`.
    Bool(bool),
    /// A whole number of any integer type.
    Int(i64),
    /// A number with a fraction, of either float type.
    Float(f64),
    /// A character.
    Char(char),
    /// Text: a `String` or a `&str`.
    Str(String),
    /// A tuple.
    Tuple(Vec<Value>),
    /// A vector, an array or what an iterator yields.
    List(Vec<Value>),
    /// `Some(..)` or `None`.
    Option(Option<Box<Value>>),
    /// `a..b` or `a..=b`, as the numbers it gives.
    Range(i64, i64),
    /// A closure of the program.
    Closure(Rc<Closure>),
    /// A `State` of the program, holding interpreter values.
    State(State<Value>),
    /// A place a closure was given to change, as `|n| *n += 1` gets.
    Place(Rc<RefCell<Value>>),
    /// An object of the host program.
    Host(Host),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("()"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => write!(f, "{n:?}"),
            Self::Char(c) => write!(f, "{c:?}"),
            Self::Str(s) => write!(f, "{s:?}"),
            Self::Tuple(items) => f.debug_tuple("").field(items).finish(),
            Self::List(items) => f.debug_list().entries(items).finish(),
            Self::Option(Some(v)) => write!(f, "Some({v:?})"),
            Self::Option(None) => f.write_str("None"),
            Self::Range(a, b) => write!(f, "{a}..{b}"),
            Self::Closure(_) => f.write_str("<closure>"),
            Self::State(state) => write!(f, "State({:?})", state.get()),
            Self::Place(place) => write!(f, "&mut {:?}", place.borrow()),
            Self::Host(host) => write!(f, "<{}>", host.type_name),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self.read(), other.read()) {
            (Self::Unit, Self::Unit) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Int(a), Self::Float(b)) | (Self::Float(b), Self::Int(a)) => a as f64 == b,
            (Self::Char(a), Self::Char(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Tuple(a), Self::Tuple(b)) | (Self::List(a), Self::List(b)) => a == b,
            (Self::Option(a), Self::Option(b)) => a == b,
            (Self::Range(a, b), Self::Range(c, d)) => (a, b) == (c, d),
            _ => false,
        }
    }
}

impl Value {
    /// What a place holds, or the value itself.
    pub fn read(&self) -> Value {
        match self {
            Self::Place(place) => place.borrow().read(),
            other => other.clone(),
        }
    }

    /// The name of its kind, for messages.
    pub fn kind(&self) -> String {
        match self {
            Self::Unit => "()".into(),
            Self::Bool(_) => "bool".into(),
            Self::Int(_) => "integer".into(),
            Self::Float(_) => "float".into(),
            Self::Char(_) => "char".into(),
            Self::Str(_) => "String".into(),
            Self::Tuple(_) => "tuple".into(),
            Self::List(_) => "Vec".into(),
            Self::Option(_) => "Option".into(),
            Self::Range(..) => "range".into(),
            Self::Closure(_) => "closure".into(),
            Self::State(_) => "State".into(),
            Self::Place(place) => place.borrow().kind(),
            Self::Host(host) => host.type_name.into(),
        }
    }

    /// The items it gives when iterated: a list's, a range's numbers, an option's one or none.
    pub fn items(&self) -> Result<Vec<Value>, String> {
        match self.read() {
            Self::List(items) | Self::Tuple(items) => Ok(items),
            Self::Range(a, b) => Ok((a..b).map(Value::Int).collect()),
            Self::Option(option) => Ok(option.map(|v| *v).into_iter().collect()),
            Self::Str(text) => Ok(text.chars().map(Value::Char).collect()),
            other => Err(format!("a {} cannot be iterated", other.kind())),
        }
    }
}

/// What a host function's argument can be made from.
pub trait FromLive: Sized {
    /// The value as this type, or why it cannot be.
    fn from_live(value: Value) -> Result<Self, String>;
}

/// What a host function's result becomes.
pub trait IntoLive {
    /// The value for the interpreter.
    fn into_live(self) -> Value;
}

impl FromLive for Value {
    fn from_live(value: Value) -> Result<Self, String> {
        Ok(value)
    }
}

impl IntoLive for Value {
    fn into_live(self) -> Value {
        self
    }
}

impl IntoLive for () {
    fn into_live(self) -> Value {
        Value::Unit
    }
}

impl FromLive for () {
    fn from_live(_: Value) -> Result<Self, String> {
        Ok(())
    }
}

fn wrong(expected: &str, value: &Value) -> String {
    format!("expected {expected}, found {}", value.kind())
}

impl FromLive for bool {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Bool(b) => Ok(b),
            other => Err(wrong("bool", &other)),
        }
    }
}

impl IntoLive for bool {
    fn into_live(self) -> Value {
        Value::Bool(self)
    }
}

macro_rules! whole {
    ($($ty:ty),*) => {$(
        impl FromLive for $ty {
            fn from_live(value: Value) -> Result<Self, String> {
                match value.read() {
                    Value::Int(n) => <$ty>::try_from(n).map_err(|_| format!("{n} does not fit in {}", stringify!($ty))),
                    other => Err(wrong(stringify!($ty), &other)),
                }
            }
        }
        impl IntoLive for $ty {
            fn into_live(self) -> Value {
                Value::Int(self as i64)
            }
        }
    )*};
}

whole!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl FromLive for f64 {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Float(n) => Ok(n),
            Value::Int(n) => Ok(n as f64),
            other => Err(wrong("f64", &other)),
        }
    }
}

impl IntoLive for f64 {
    fn into_live(self) -> Value {
        Value::Float(self)
    }
}

impl FromLive for f32 {
    fn from_live(value: Value) -> Result<Self, String> {
        f64::from_live(value).map(|n| n as f32)
    }
}

impl IntoLive for f32 {
    fn into_live(self) -> Value {
        Value::Float(f64::from(self))
    }
}

impl FromLive for char {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Char(c) => Ok(c),
            other => Err(wrong("char", &other)),
        }
    }
}

impl IntoLive for char {
    fn into_live(self) -> Value {
        Value::Char(self)
    }
}

impl FromLive for String {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Str(s) => Ok(s),
            Value::Char(c) => Ok(c.to_string()),
            other => Err(wrong("text", &other)),
        }
    }
}

impl IntoLive for String {
    fn into_live(self) -> Value {
        Value::Str(self)
    }
}

impl IntoLive for &str {
    fn into_live(self) -> Value {
        Value::Str(self.to_owned())
    }
}

impl<T: FromLive> FromLive for Option<T> {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::Option(None) => Ok(None),
            Value::Option(Some(inner)) => T::from_live(*inner).map(Some),
            other => Err(wrong("Option", &other)),
        }
    }
}

impl<T: IntoLive> IntoLive for Option<T> {
    fn into_live(self) -> Value {
        Value::Option(self.map(|v| Box::new(v.into_live())))
    }
}

impl<T: FromLive> FromLive for Vec<T> {
    fn from_live(value: Value) -> Result<Self, String> {
        value.items()?.into_iter().map(T::from_live).collect()
    }
}

impl<T: IntoLive> IntoLive for Vec<T> {
    fn into_live(self) -> Value {
        Value::List(self.into_iter().map(IntoLive::into_live).collect())
    }
}

impl<T: FromLive + Ord> FromLive for std::collections::BTreeSet<T> {
    fn from_live(value: Value) -> Result<Self, String> {
        value.items()?.into_iter().map(T::from_live).collect()
    }
}

impl<T: IntoLive> IntoLive for std::collections::BTreeSet<T> {
    fn into_live(self) -> Value {
        Value::List(self.into_iter().map(IntoLive::into_live).collect())
    }
}

macro_rules! tuples {
    ($(($($name:ident),+)),*) => {$(
        impl<$($name: FromLive),+> FromLive for ($($name,)+) {
            #[allow(non_snake_case)]
            fn from_live(value: Value) -> Result<Self, String> {
                let Value::Tuple(items) = value.read() else { return Err(wrong("a tuple", &value)) };
                let mut items = items.into_iter();
                $(let $name = $name::from_live(items.next().ok_or("the tuple is too short")?)?;)+
                Ok(($($name,)+))
            }
        }
        impl<$($name: IntoLive),+> IntoLive for ($($name,)+) {
            #[allow(non_snake_case)]
            fn into_live(self) -> Value {
                let ($($name,)+) = self;
                Value::Tuple(vec![$($name.into_live()),+])
            }
        }
    )*};
}

tuples!((A, B), (A, B, C), (A, B, C, D));

impl FromLive for State<Value> {
    fn from_live(value: Value) -> Result<Self, String> {
        match value.read() {
            Value::State(state) => Ok(state),
            other => Err(wrong("State", &other)),
        }
    }
}

/// Something that failed inside a closure the host called later, such as on a click: it
/// cannot stop the program, so it is kept for whoever shows problems.
pub fn report(problem: LiveError) {
    LATE.with(|late| late.borrow_mut().push(problem));
}

thread_local! {
    static LATE: RefCell<Vec<LiveError>> = const { RefCell::new(Vec::new()) };
}

/// What failed in closures the host called since the last time this was asked.
pub fn late_problems() -> Vec<LiveError> {
    LATE.with(|late| std::mem::take(&mut *late.borrow_mut()))
}
