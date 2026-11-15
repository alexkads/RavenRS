//! `cargo run -p feature-center`, with the parameters of [`feature_center::Opening`]:
//! `-- --modo vitrine`, `-- --modo componentes --pagina CablePlugDemo --incorporado`,
//! `-- --tema claro --idioma en`, and `-- caixa` for the counter window on its own. The web build
//! starts from `lib.rs` and reads the same names from the address of the page.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let opening = match feature_center::Opening::from_arguments(std::env::args().skip(1)) {
        Ok(opening) => opening,
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(2);
        }
    };
    ravenrs::native::run(
        feature_center::view(&opening),
        ravenrs::native::WindowOptions::new("Raven Centro de Recursos")
            .size(1400.0, 900.0)
            .theme(feature_center::window_theme(&opening)),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
