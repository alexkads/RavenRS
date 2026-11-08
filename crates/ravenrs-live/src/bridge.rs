//! What `#[expose]` generates calls through: it turns any argument and any result into and
//! out of interpreter values, choosing at compile time the best way each type allows, so the
//! generated code always compiles and only fails, with a message, when a value does not fit.

use std::cell::RefCell;
use std::marker::PhantomData;

use crate::value::{FromLive, Host, IntoLive, Value};

/// The name a host value of `T` goes by: its type's last path segment, without generics.
pub fn host_name<T: ?Sized>() -> &'static str {
    let full = std::any::type_name::<T>();
    let plain = full.split('<').next().unwrap_or(full);
    plain.rsplit("::").next().unwrap_or(plain)
}

/// An argument of type `T` to make from a value.
pub struct Arg<T>(PhantomData<fn() -> T>);

impl<T> Arg<T> {
    /// The argument maker for `T`.
    pub fn of() -> Self {
        Self(PhantomData)
    }
}

impl<T> std::fmt::Debug for Arg<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Arg<{}>", host_name::<T>())
    }
}

impl<T> Clone for Arg<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Arg<T> {}

/// Made through [`FromLive`], for numbers, text, closures, bindings of plain values and the like.
pub trait ArgFromLive<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

impl<T: FromLive> ArgFromLive<T> for &&&&&&Arg<T> {
    fn make(self, value: Value) -> Result<T, String> {
        T::from_live(value)
    }
}

fn host_value<T: 'static>(value: Value) -> Result<T, String> {
    match value.read() {
        Value::Host(host) => host.take_as::<T>(),
        other => Err(format!("expected a {}, found {}", host_name::<T>(), other.kind())),
    }
}

/// A list of host values.
pub trait ArgHostVec<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

impl<T: 'static> ArgHostVec<Vec<T>> for &&&&Arg<Vec<T>> {
    fn make(self, value: Value) -> Result<Vec<T>, String> {
        value.items()?.into_iter().map(host_value::<T>).collect()
    }
}

/// An optional host value.
pub trait ArgHostOption<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

impl<T: 'static> ArgHostOption<Option<T>> for &&&Arg<Option<T>> {
    fn make(self, value: Value) -> Result<Option<T>, String> {
        match value.read() {
            Value::Option(None) => Ok(None),
            Value::Option(Some(inner)) => host_value::<T>(*inner).map(Some),
            other => host_value::<T>(other).map(Some),
        }
    }
}

/// A binding to a state holding host values, which copy as they are read.
pub trait ArgHostBinding<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

fn read_copy<T: Clone + 'static>(value: &Value) -> Option<T> {
    match value.read() {
        Value::Host(host) => {
            let cell = host.cell.borrow();
            cell.as_ref()?.downcast_ref::<T>().cloned()
        }
        _ => None,
    }
}

impl<T: Clone + 'static> ArgHostBinding<ravenrs_reactive::Binding<T>>
    for &&Arg<ravenrs_reactive::Binding<T>>
{
    fn make(self, value: Value) -> Result<ravenrs_reactive::Binding<T>, String> {
        let Some(state) = value.as_binding() else {
            return Err(format!("expected a State, found {}", value.kind()));
        };
        let first = read_copy::<T>(&state.get())
            .ok_or_else(|| format!("this State does not hold a {}", host_name::<T>()))?;
        let last = std::rc::Rc::new(RefCell::new(first));
        let (read, write, kept) = (state.clone(), state, last.clone());
        Ok(ravenrs_reactive::Binding::new(
            move || {
                if let Some(value) = read_copy::<T>(&read.get()) {
                    *last.borrow_mut() = value;
                }
                last.borrow().clone()
            },
            move |value: T| {
                *kept.borrow_mut() = value.clone();
                write.set(Value::Host(Host::cloneable(host_name::<T>(), value)));
            },
        ))
    }
}

/// A value the host made, as the interpreter's value: numbers, text and bools as themselves,
/// anything else as a host value. It looks at the type while the program runs, for closures
/// whose argument types were not known to convert when the code was compiled.
pub fn any_to_value<A: 'static>(value: A) -> Value {
    let boxed: Box<dyn std::any::Any> = Box::new(value);
    macro_rules! plain {
        ($boxed:ident, $($ty:ty => $make:expr),*) => {$(
            let $boxed = match $boxed.downcast::<$ty>() {
                Ok(v) => return $make(*v),
                Err(other) => other,
            };
        )*};
    }
    plain!(boxed,
        i64 => Value::Int, i32 => |v: i32| Value::Int(i64::from(v)), u32 => |v: u32| Value::Int(i64::from(v)),
        usize => |v: usize| Value::Int(v as i64), u64 => |v: u64| Value::Int(v as i64), u8 => |v: u8| Value::Int(i64::from(v)),
        f64 => Value::Float, f32 => |v: f32| Value::Float(f64::from(v)), bool => Value::Bool, char => Value::Char,
        String => Value::Str, () => |_| Value::Unit
    );
    Value::Host(Host {
        type_name: host_name::<A>(),
        cell: std::rc::Rc::new(RefCell::new(Some(boxed))),
        copy: None,
    })
}

/// The closure a value holds.
pub fn closure_of(value: &Value) -> Result<std::rc::Rc<crate::Closure>, String> {
    match value.read() {
        Value::Closure(closure) => Ok(closure),
        other => Err(format!("expected a closure, found {}", other.kind())),
    }
}

fn call_later(closure: &crate::Closure, arguments: Vec<Value>) {
    if let Err(problem) = closure.call(arguments) {
        crate::value::report(problem);
    }
}

/// A closure that returns nothing and gets host values, which arrive as the interpreter sees
/// them while it runs.
pub trait ArgHostClosure<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

impl ArgHostClosure<Box<dyn Fn()>> for &&&&&Arg<Box<dyn Fn()>> {
    fn make(self, value: Value) -> Result<Box<dyn Fn()>, String> {
        let closure = closure_of(&value)?;
        Ok(Box::new(move || call_later(&closure, Vec::new())))
    }
}

impl<A: 'static> ArgHostClosure<Box<dyn Fn(A)>> for &&&&&Arg<Box<dyn Fn(A)>> {
    fn make(self, value: Value) -> Result<Box<dyn Fn(A)>, String> {
        let closure = closure_of(&value)?;
        Ok(Box::new(move |a: A| call_later(&closure, vec![any_to_value(a)])))
    }
}

impl<A: 'static, B: 'static> ArgHostClosure<Box<dyn Fn(A, B)>> for &&&&&Arg<Box<dyn Fn(A, B)>> {
    fn make(self, value: Value) -> Result<Box<dyn Fn(A, B)>, String> {
        let closure = closure_of(&value)?;
        Ok(Box::new(move |a: A, b: B| call_later(&closure, vec![any_to_value(a), any_to_value(b)])))
    }
}

/// A binding to a state holding an optional host value, or a list of them.
pub trait ArgHostBindingOf<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

fn binding_of<T: Clone + 'static>(
    value: Value,
    read: fn(&Value) -> Option<T>,
    write: fn(T) -> Value,
) -> Result<ravenrs_reactive::Binding<T>, String> {
    let Some(state) = value.as_binding() else {
        return Err(format!("expected a State, found {}", value.kind()));
    };
    let first =
        read(&state.get()).ok_or_else(|| format!("this State does not hold a {}", host_name::<T>()))?;
    let last = std::rc::Rc::new(RefCell::new(first));
    let (getter, setter, kept) = (state.clone(), state, last.clone());
    Ok(ravenrs_reactive::Binding::new(
        move || {
            if let Some(value) = read(&getter.get()) {
                *last.borrow_mut() = value;
            }
            last.borrow().clone()
        },
        move |value: T| {
            *kept.borrow_mut() = value.clone();
            setter.set(write(value));
        },
    ))
}

impl<T: Clone + 'static> ArgHostBindingOf<ravenrs_reactive::Binding<Option<T>>>
    for &&&Arg<ravenrs_reactive::Binding<Option<T>>>
{
    fn make(self, value: Value) -> Result<ravenrs_reactive::Binding<Option<T>>, String> {
        binding_of(
            value,
            |v| match v.read() {
                Value::Option(None) => Some(None),
                Value::Option(Some(inner)) => read_copy::<T>(&inner).map(Some),
                other => read_copy::<T>(&other).map(Some),
            },
            |v: Option<T>| {
                Value::Option(v.map(|v| Box::new(Value::Host(Host::cloneable(host_name::<T>(), v)))))
            },
        )
    }
}

impl<T: Clone + 'static> ArgHostBindingOf<ravenrs_reactive::Binding<Vec<T>>>
    for &&&Arg<ravenrs_reactive::Binding<Vec<T>>>
{
    fn make(self, value: Value) -> Result<ravenrs_reactive::Binding<Vec<T>>, String> {
        binding_of(
            value,
            |v| v.items().ok()?.iter().map(read_copy::<T>).collect(),
            |v: Vec<T>| {
                Value::List(
                    v.into_iter().map(|v| Value::Host(Host::cloneable(host_name::<T>(), v))).collect(),
                )
            },
        )
    }
}

/// Taken out of a host value, or converted from one, for everything else.
pub trait ArgFromHost<T> {
    /// The argument.
    fn make(self, value: Value) -> Result<T, String>;
}

impl<T: 'static> ArgFromHost<T> for &Arg<T> {
    fn make(self, value: Value) -> Result<T, String> {
        host_value::<T>(value)
    }
}

/// A result to turn into a value.
pub struct Ret<T>(RefCell<Option<T>>);

impl<T> std::fmt::Debug for Ret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ret<{}>", host_name::<T>())
    }
}

impl<T> Ret<T> {
    /// Holds a result.
    pub fn new(value: T) -> Self {
        Self(RefCell::new(Some(value)))
    }

    fn take(&self) -> Option<T> {
        self.0.borrow_mut().take()
    }
}

fn copied<T: Clone + 'static>(value: T) -> Value {
    Value::Host(Host::cloneable(host_name::<T>(), value))
}

fn moved<T: 'static>(value: T) -> Value {
    Value::Host(Host::new(host_name::<T>(), value))
}

/// Through [`IntoLive`].
pub trait RetIntoLive {
    /// The value.
    fn live(self) -> Value;
}

impl<T: IntoLive> RetIntoLive for &&&&&&&Ret<T> {
    fn live(self) -> Value {
        self.take().map_or(Value::Unit, IntoLive::into_live)
    }
}

/// An optional host value that copies.
pub trait RetOptionCopied {
    /// The value.
    fn live(self) -> Value;
}

impl<T: Clone + 'static> RetOptionCopied for &&&&&&Ret<Option<T>> {
    fn live(self) -> Value {
        Value::Option(self.take().flatten().map(|v| Box::new(copied(v))))
    }
}

/// A list of host values that copy.
pub trait RetVecCopied {
    /// The value.
    fn live(self) -> Value;
}

impl<T: Clone + 'static> RetVecCopied for &&&&&Ret<Vec<T>> {
    fn live(self) -> Value {
        Value::List(self.take().unwrap_or_default().into_iter().map(copied).collect())
    }
}

/// As a host value that copies when used twice.
pub trait RetCloneable {
    /// The value.
    fn live(self) -> Value;
}

impl<T: Clone + 'static> RetCloneable for &&&&Ret<T> {
    fn live(self) -> Value {
        self.take().map_or(Value::Unit, copied)
    }
}

/// An optional host value that moves.
pub trait RetOptionMoved {
    /// The value.
    fn live(self) -> Value;
}

impl<T: 'static> RetOptionMoved for &&&Ret<Option<T>> {
    fn live(self) -> Value {
        Value::Option(self.take().flatten().map(|v| Box::new(moved(v))))
    }
}

/// A list of host values that move.
pub trait RetVecMoved {
    /// The value.
    fn live(self) -> Value;
}

impl<T: 'static> RetVecMoved for &&Ret<Vec<T>> {
    fn live(self) -> Value {
        Value::List(self.take().unwrap_or_default().into_iter().map(moved).collect())
    }
}

/// As a host value that moves when used.
pub trait RetHost {
    /// The value.
    fn live(self) -> Value;
}

impl<T: 'static> RetHost for &Ret<T> {
    fn live(self) -> Value {
        self.take().map_or(Value::Unit, moved)
    }
}

/// Runs `f` on a host value without taking it, for methods that take `&self`.
pub fn with_ref<T: 'static, R>(value: &Value, f: impl FnOnce(&T) -> R) -> Result<R, String> {
    match value.read() {
        Value::Host(host) => {
            let cell = host.cell.borrow();
            let any = cell.as_ref().ok_or_else(|| format!("this {} was already used", host.type_name))?;
            any.downcast_ref::<T>()
                .map(f)
                .ok_or_else(|| format!("a {} is not a {}", host.type_name, host_name::<T>()))
        }
        other => Err(format!("expected a {}, found {}", host_name::<T>(), other.kind())),
    }
}

/// Runs `f` on a host value, changing it in place, for methods that take `&mut self`.
pub fn with_mut<T: 'static, R>(value: &Value, f: impl FnOnce(&mut T) -> R) -> Result<R, String> {
    match value.read() {
        Value::Host(host) => {
            let mut cell = host.cell.borrow_mut();
            let any = cell.as_mut().ok_or_else(|| format!("this {} was already used", host.type_name))?;
            let name = host.type_name;
            any.downcast_mut::<T>().map(f).ok_or_else(|| format!("a {name} is not a {}", host_name::<T>()))
        }
        other => Err(format!("expected a {}, found {}", host_name::<T>(), other.kind())),
    }
}

/// The argument at `index`, or an error saying how many were expected.
pub fn arg(values: &[Value], index: usize, count: usize) -> Result<Value, String> {
    if values.len() != count {
        return Err(format!("this takes {count} arguments, and got {}", values.len()));
    }
    Ok(values[index].clone())
}

/// The items of a list, for an argument made as a `Vec`.
pub fn list(value: &Value) -> Result<Vec<Value>, String> {
    value.items()
}

/// The items of a tuple of `count` values.
pub fn tuple(value: &Value, count: usize) -> Result<Vec<Value>, String> {
    match value.read() {
        Value::Tuple(items) if items.len() == count => Ok(items),
        other => Err(format!("expected a tuple of {count}, found {}", other.kind())),
    }
}

/// `Ok(..)` or `Err(..)`, as the interpreter wrote it; any other value counts as `Ok`.
pub fn result(value: &Value) -> Result<Value, Value> {
    match value.read() {
        Value::Result(Ok(v)) => Ok(*v),
        Value::Result(Err(e)) => Err(*e),
        other => Ok(other),
    }
}

/// `Some(..)` or `None`; `()` counts as `None` and any other value as `Some`.
pub fn option(value: &Value) -> Option<Value> {
    match value.read() {
        Value::Option(inner) => inner.map(|v| *v),
        Value::Unit => None,
        other => Some(other),
    }
}

/// What a closure the host calls gives when it fails, by type.
pub struct FallbackOf<T>(PhantomData<fn() -> T>);

impl<T> std::fmt::Debug for FallbackOf<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FallbackOf<{}>", host_name::<T>())
    }
}

impl<T> FallbackOf<T> {
    /// The fallback finder for `T`.
    pub fn of() -> Self {
        Self(PhantomData)
    }
}

/// Through [`Fallback`](crate::Fallback).
pub trait FallbackOwn<T> {
    /// The function that makes the fallback.
    fn get(self) -> Result<fn() -> T, String>;
}

impl<T: crate::Fallback> FallbackOwn<T> for &&&FallbackOf<T> {
    fn get(self) -> Result<fn() -> T, String> {
        Ok(T::fallback)
    }
}

/// Through `Default`.
pub trait FallbackDefault<T> {
    /// The function that makes the fallback.
    fn get(self) -> Result<fn() -> T, String>;
}

impl<T: Default> FallbackDefault<T> for &&FallbackOf<T> {
    fn get(self) -> Result<fn() -> T, String> {
        Ok(T::default)
    }
}

/// None: such a closure cannot be made.
pub trait FallbackNone<T> {
    /// Why not.
    fn get(self) -> Result<fn() -> T, String>;
}

impl<T> FallbackNone<T> for &FallbackOf<T> {
    fn get(self) -> Result<fn() -> T, String> {
        Err(format!("a closure that gives a {} cannot be written in the playground yet", host_name::<T>()))
    }
}

/// A value the host lends a closure, as the interpreter's value.
pub struct Borrowed<'a, T>(pub &'a T);

impl<T> std::fmt::Debug for Borrowed<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Borrowed<{}>", host_name::<T>())
    }
}

/// Copied into a plain value.
pub trait BorrowedLive {
    /// The value.
    fn live(self) -> Value;
}

impl<T: IntoLive + Clone> BorrowedLive for &&&Borrowed<'_, T> {
    fn live(self) -> Value {
        self.0.clone().into_live()
    }
}

/// Copied into a host value.
pub trait BorrowedCopy {
    /// The value.
    fn live(self) -> Value;
}

impl<T: Clone + 'static> BorrowedCopy for &&Borrowed<'_, T> {
    fn live(self) -> Value {
        copied(self.0.clone())
    }
}

/// Not copied: the closure gets `()`.
pub trait BorrowedNone {
    /// The value.
    fn live(self) -> Value;
}

impl<T> BorrowedNone for &Borrowed<'_, T> {
    fn live(self) -> Value {
        Value::Unit
    }
}

/// A field to copy out of a struct, when its type copies.
pub struct CloneOf<'a, T>(pub &'a T);

impl<T> std::fmt::Debug for CloneOf<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CloneOf<{}>", host_name::<T>())
    }
}

/// It copies.
pub trait CloneOfSome<T> {
    /// The copy.
    fn cloned(self) -> Option<T>;
}

impl<T: Clone> CloneOfSome<T> for &&CloneOf<'_, T> {
    fn cloned(self) -> Option<T> {
        Some(self.0.clone())
    }
}

/// It does not.
pub trait CloneOfNone<T> {
    /// Nothing.
    fn cloned(self) -> Option<T>;
}

impl<T> CloneOfNone<T> for &CloneOf<'_, T> {
    fn cloned(self) -> Option<T> {
        None
    }
}

/// Something the host lends a closure to change, such as the scene a canvas draws into: the
/// closure gets a fresh one, which is then added to the real one.
pub trait Scratch: Sized + 'static {
    /// An empty one.
    fn fresh() -> Self;
    /// Adds what was done to it to the real one.
    fn merge_into(self, target: &mut Self);
}

/// How a `&mut T` reaches a closure.
pub struct ScratchOf<T>(PhantomData<fn() -> T>);

impl<T> std::fmt::Debug for ScratchOf<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScratchOf<{}>", host_name::<T>())
    }
}

impl<T> ScratchOf<T> {
    /// The scratch maker for `T`.
    pub fn of() -> Self {
        Self(PhantomData)
    }
}

/// Through [`Scratch`].
pub trait ScratchSome<T> {
    /// Whether it can.
    fn check(self) -> Result<(), String>;
    /// A fresh one, as a value.
    fn fresh(self) -> Value;
    /// Adds what the closure did to the real one.
    fn merge(self, value: &Value, target: &mut T);
}

impl<T: Scratch> ScratchSome<T> for &&ScratchOf<T> {
    fn check(self) -> Result<(), String> {
        Ok(())
    }
    fn fresh(self) -> Value {
        moved(T::fresh())
    }
    fn merge(self, value: &Value, target: &mut T) {
        if let Value::Host(host) = value
            && let Ok(made) = host.take::<T>()
        {
            made.merge_into(target);
        }
    }
}

/// It cannot.
pub trait ScratchNone<T> {
    /// Why not.
    fn check(self) -> Result<(), String>;
    /// Nothing.
    fn fresh(self) -> Value;
    /// Nothing.
    fn merge(self, value: &Value, target: &mut T);
}

impl<T> ScratchNone<T> for &ScratchOf<T> {
    fn check(self) -> Result<(), String> {
        Err(format!("a closure that changes a {} cannot be written in the playground yet", host_name::<T>()))
    }
    fn fresh(self) -> Value {
        Value::Unit
    }
    fn merge(self, _: &Value, _: &mut T) {}
}

/// Calls a closure of the program for the host, reporting a failure instead of stopping.
pub fn call_back<R>(
    closure: &crate::Closure,
    arguments: Vec<Value>,
    make: impl FnOnce(Value) -> Result<R, String>,
    fallback: fn() -> R,
) -> R {
    match closure.call(arguments).and_then(|v| make(v).map_err(|e| closure.error(e))) {
        Ok(value) => value,
        Err(problem) => {
            crate::value::report(problem);
            fallback()
        }
    }
}

/// `Type::default()`, for types that have one.
pub struct DefaultOf<T>(PhantomData<fn() -> T>);

impl<T> std::fmt::Debug for DefaultOf<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DefaultOf<{}>", host_name::<T>())
    }
}

impl<T> DefaultOf<T> {
    /// The default maker for `T`.
    pub fn of() -> Self {
        Self(PhantomData)
    }
}

/// It has one.
pub trait DefaultSome<T> {
    /// The default.
    fn make(self) -> Option<T>;
}

impl<T: Default> DefaultSome<T> for &&DefaultOf<T> {
    fn make(self) -> Option<T> {
        Some(T::default())
    }
}

/// It does not.
pub trait DefaultNone<T> {
    /// Nothing.
    fn make(self) -> Option<T>;
}

impl<T> DefaultNone<T> for &DefaultOf<T> {
    fn make(self) -> Option<T> {
        None
    }
}

/// A plain value of type `S` going where `impl Into<T>` goes, when `T: From<S>`.
pub struct IntoFrom<T, S>(pub S, PhantomData<fn() -> T>);

impl<T, S> std::fmt::Debug for IntoFrom<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntoFrom<{}, {}>", host_name::<T>(), host_name::<S>())
    }
}

impl<T, S> IntoFrom<T, S> {
    /// Holds the plain value.
    pub fn new(value: S) -> Self {
        Self(value, PhantomData)
    }
}

/// `T: From<S>`.
pub trait IntoFromSome<T> {
    /// The converted value.
    fn get(self) -> Option<T>;
}

impl<T: From<S>, S: Clone> IntoFromSome<T> for &&IntoFrom<T, S> {
    fn get(self) -> Option<T> {
        Some(T::from(self.0.clone()))
    }
}

/// No such conversion.
pub trait IntoFromNone<T> {
    /// Nothing.
    fn get(self) -> Option<T>;
}

impl<T, S> IntoFromNone<T> for &IntoFrom<T, S> {
    fn get(self) -> Option<T> {
        None
    }
}
