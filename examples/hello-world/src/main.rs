//! Native entry point: `cargo run -p hello-world`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("RavenRS — Hello World").size(760.0, 560.0);
    ravenrs::native::run(hello_world::app(), options)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
