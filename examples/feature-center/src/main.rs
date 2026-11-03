//! `cargo run -p feature-center`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(
        feature_center::app(),
        ravenrs::native::WindowOptions::new("Raven Centro de Recursos").size(1400.0, 900.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
