//! `cargo run -p mini-pdv`. BETA: nothing here has fiscal value.

fn main() -> Result<(), ravenrs::native::PlatformError> {
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), in
    // memory: the point of sale's file is never opened.
    if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session = ravenrs_e2e::E2e::new(mini_pdv::app(None), ravenrs::prelude::Theme::raven_dark())
            .viewport(ravenrs::prelude::Size::new(1366.0, 768.0), 1.0);
        let hello = ravenrs_e2e_wire::Hello::of("Mini-PDV").scenarios(mini_pdv::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let file = std::env::var_os("RAVEN_PDV").map_or_else(mini_pdv::arquivo_padrao, std::path::PathBuf::from);
    ravenrs::native::run(
        mini_pdv::app(Some(&file)),
        ravenrs::native::WindowOptions::new("Mini-PDV RavenRS (BETA)").size(1366.0, 768.0),
    )
}
