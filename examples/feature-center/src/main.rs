//! `cargo run -p feature-center`, or `cargo run -p feature-center -- caixa` for the counter window
//! on its own. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let counter = std::env::args().nth(1).as_deref() == Some("caixa");
    ravenrs::native::run(
        if counter { feature_center::counter_window() } else { feature_center::app() },
        ravenrs::native::WindowOptions::new("Raven Centro de Recursos").size(1400.0, 900.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
