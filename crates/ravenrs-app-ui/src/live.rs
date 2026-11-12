//! What the generated registrations of this crate cannot write: functions that take any value
//! of a trait of the app, given here the value an example has.

use ravenrs_app::{Application, ApplicationBuilder, MemoryDatabase, MemoryObjectSpace};
use ravenrs_live::{Host, Registry, Value, bridge};

use crate::{AppShell, EditorRegistry, SystemModule, Words};

fn take<T: 'static>(args: &[Value], at: usize) -> Result<T, String> {
    match args.get(at).map(Value::read) {
        Some(Value::Host(host)) => host.take_as::<T>(),
        Some(other) => Err(format!("expected a {}, found {}", bridge::host_name::<T>(), other.kind())),
        None => Err("an argument is missing".to_owned()),
    }
}

pub(crate) fn expose(registry: &mut Registry) {
    // `.module(..)` takes any `Module`; the playground has the system's.
    registry.method_for::<ApplicationBuilder>("ApplicationBuilder", "module", |args| {
        let builder: ApplicationBuilder = take(&args, 0)?;
        let module: SystemModule = take(&args, 1)?;
        Ok(Value::Host(Host::new("ApplicationBuilder", builder.module(module))))
    });
    // `AppShell::with_options` takes a maker of any `ObjectSpace`; here, of records in memory.
    registry.function("AppShell", "with_options", |args| {
        if args.len() != 4 {
            return Err(
                "`with_options` takes the application, a maker of spaces, the words and the editors".into()
            );
        }
        let application: Application = take(&args, 0)?;
        let space = bridge::closure_of(&args[1])?;
        let words: Words = take(&args, 2)?;
        let editors: EditorRegistry = take(&args, 3)?;
        // When the maker fails, the shell gets an empty space, and the problem is shown.
        let empty = MemoryDatabase::new(Vec::new()).map_err(|e| e.to_string())?;
        let made = move || -> MemoryObjectSpace {
            let made = space.call(Vec::new()).and_then(|value| match value.read() {
                Value::Host(host) => host.take::<MemoryObjectSpace>().map_err(|e| space.error(e)),
                other => Err(space.error(format!("expected a MemoryObjectSpace, found {}", other.kind()))),
            });
            made.unwrap_or_else(|problem| {
                ravenrs_live::report(problem);
                empty.space()
            })
        };
        let shell = AppShell::with_options(application, made, words, editors);
        Ok(Value::Host(Host::new("AppShell", shell)))
    });
}
