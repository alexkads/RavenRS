//! Native entry point: `cargo run -p dock-workbench -- editor` (or `ide`, or `chat`).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    use dock_workbench::Workbench;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let which = std::env::args().nth(1).and_then(|word| Workbench::named(&word)).unwrap_or(Workbench::Editor);
    let options = ravenrs::native::WindowOptions::new(which.title()).size(1440.0, 900.0);
    ravenrs::native::run(which.app(), options)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
