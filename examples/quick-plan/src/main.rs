//! `cargo run -p quick-plan`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(
        quick_plan::app(),
        ravenrs::native::WindowOptions::new("Raven Plano").size(1200.0, 800.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
