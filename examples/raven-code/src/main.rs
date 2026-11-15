//! Native entry point: `cargo run -p raven-code -- <folder>`, or with no folder for the sample.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let project = raven_code::from_args(std::env::args().skip(1))?;
    let title = format!("{} — Raven Code", project.name());
    let options = ravenrs::native::WindowOptions::new(title).size(1440.0, 900.0);
    let mut app = raven_code::RavenCode::new(project).with_language_server();
    if let Some(config) = raven_code::config::Config::of_user() {
        app = app.remembering(config);
    }
    ravenrs::native::run(app, options)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
