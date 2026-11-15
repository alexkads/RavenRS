//! `cargo run -p raven-flows`
fn main() -> Result<(), ravenrs::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("Raven Flows").size(1440.0, 900.0);
    let app = match raven_flows::app_in(raven_flows::Casa::default_dir(), raven_flows::Worker::Thread) {
        Ok(app) => app,
        Err(why) => {
            tracing::error!("{why}");
            std::process::exit(1);
        }
    };
    ravenrs::native::run(app, options)
}
