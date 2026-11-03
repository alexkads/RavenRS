//! Functions an application adds to the criteria language, for everything that reads an expression.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_criteria::{Environment, Value};

type Function = Rc<dyn Fn(&[Value]) -> Result<Value, String>>;

thread_local! {
    static FUNCTIONS: RefCell<Vec<(String, Function)>> = const { RefCell::new(Vec::new()) };
}

/// Adds a function to expressions in rules, appearance, actions, calculated fields and filters.
/// An application's state is of one thread, and so is this. A filter with one runs in memory, as
/// the database does not know it.
pub fn register_function(
    name: impl Into<String>,
    function: impl Fn(&[Value]) -> Result<Value, String> + 'static,
) {
    FUNCTIONS.with(|all| all.borrow_mut().push((name.into(), Rc::new(function))));
}

/// What an expression is evaluated with: the language and what was added to it.
pub fn environment() -> Environment {
    FUNCTIONS.with(|all| {
        all.borrow().iter().fold(Environment::new(), |environment, (name, function)| {
            let function = function.clone();
            environment.register(name, move |values| function(values))
        })
    })
}
