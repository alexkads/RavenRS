//! `cargo run -p mini-pdv`. BETA: nothing here has fiscal value.

fn main() -> Result<(), ravenrs::native::PlatformError> {
    let file = std::env::var_os("RAVEN_PDV").map_or_else(mini_pdv::arquivo_padrao, std::path::PathBuf::from);
    ravenrs::native::run(
        mini_pdv::app(Some(&file)),
        ravenrs::native::WindowOptions::new("Mini-PDV RavenRS (BETA)").size(1366.0, 768.0),
    )
}
