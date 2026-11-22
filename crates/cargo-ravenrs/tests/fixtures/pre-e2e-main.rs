//! The desktop: `cargo run`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let options = ravenrs::native::WindowOptions::new("sample-app").size(1000.0, 700.0);
    ravenrs::native::run(sample_app::app(), options)
}

// The browser starts from `lib.rs`.
#[cfg(target_arch = "wasm32")]
fn main() {}
