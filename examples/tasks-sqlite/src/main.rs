//! `cargo run -p quick-app`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(
        tasks_sqlite::app(),
        ravenrs::native::WindowOptions::new("Raven Contas").size(1100.0, 720.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
