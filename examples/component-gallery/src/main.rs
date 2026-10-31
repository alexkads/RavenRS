//! Native entry point: `cargo run -p component-gallery --release`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), raven::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = raven::native::WindowOptions::new("RavenRS — Components").size(1280.0, 820.0);
    raven::native::run(component_gallery::Gallery::new(), options)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
