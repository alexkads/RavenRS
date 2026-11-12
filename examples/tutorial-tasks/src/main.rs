//! Execute com `cargo run -p tutorial-tasks`.

fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(
        tutorial_tasks::app(),
        ravenrs::native::WindowOptions::new("Minhas tarefas").size(1200.0, 760.0),
    )
}
