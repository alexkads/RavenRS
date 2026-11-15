//! `cargo run -p flow-chat-bot`
fn main() -> Result<(), ravenrs::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options =
        ravenrs::native::WindowOptions::new("RavenRS · Chat automation builder").size(1280.0, 800.0);
    ravenrs::native::run(flow_chat_bot::app(), options)
}
