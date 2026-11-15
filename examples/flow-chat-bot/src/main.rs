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
    // The diagrams are kept in a folder of the person's, between runs.
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
    let folder = std::path::PathBuf::from(home).join(".ravenrs").join("flow-chat-bot");
    let storage = std::rc::Rc::new(ravenrs::core::FileStorage::new(folder));
    ravenrs::native::run(flow_chat_bot::app_kept_in(storage), options)
}
