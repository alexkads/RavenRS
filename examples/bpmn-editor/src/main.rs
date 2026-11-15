//! `cargo run -p bpmn-editor --release` (the local model is slow in a debug build).
fn main() -> Result<(), ravenrs::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("RavenRS · Editor BPMN").size(1440.0, 900.0);
    // Processes and models are kept in a folder of the person's, between runs.
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
    let folder = std::path::PathBuf::from(home).join(".ravenrs").join("bpmn-editor");
    let storage = std::rc::Rc::new(ravenrs::core::FileStorage::new(folder.join("processos")));
    ravenrs::native::run(bpmn_editor::app_kept_in(storage, Some(folder.join("modelos"))), options)
}
