//! `cargo run --manifest-path examples/quick-app/Cargo.toml`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    quick_app::e2e::serve_if_asked();
    ravenrs::native::run(
        quick_app::app(),
        ravenrs::native::WindowOptions::new("Raven Contas").size(1100.0, 720.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
