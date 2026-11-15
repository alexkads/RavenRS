//! The ready-made modules of the framework, each on a screen of its own: the counter of a shop
//! (`ravenrs-app-pos`), requests decided step by step (`ravenrs-app-requests`), the catalog of
//! texts (`ravenrs-app-i18n`), the accessibility audit (`ravenrs-app-a11y`), modules and their
//! manifests put together in code, and a panel declared over a Rust struct (`ravenrs-app-report`).

pub mod auditoria;
pub mod catalogo;
pub mod manifestos;
pub mod pdv;
pub mod pedidos;
pub mod vendas;

use ravenrs::prelude::*;
use ravenrs_app::{ApplicationBuilder, DashboardInfo};
use ravenrs_app_ui::{App, Session};

use super::screen;

/// The group of the navigation of the modules put together in code.
pub const MODULES: &str = "Módulos";

/// The dashboards of this part.
pub fn dashboards() -> Vec<DashboardInfo> {
    vec![
        screen(pdv::ID, pdv::PAGE, "Balcão", "credit-card"),
        screen(pedidos::ID, pedidos::PAGE, "Estados e lembretes", "inbox"),
        screen(catalogo::ID, catalogo::PAGE, "Aparência condicional", "languages"),
        screen(auditoria::ID, auditoria::PAGE, "Acessibilidade", "eye"),
        screen(manifestos::ID, manifestos::PAGE, MODULES, "package"),
        vendas::panel().info(),
    ]
}

/// The controls of [`dashboards`].
pub fn register(session: &Session) {
    pdv::register(session);
    pedidos::register(session);
    catalogo::register(session);
    auditoria::register(session);
    manifestos::register(session);
    vendas::panel().register(session);
}

/// What this part adds to the application: the sales the panel reads.
pub fn extend(app: App) -> App {
    app.module(vendas::SalesModule)
}

/// The modules of [`extend`], for the model storage builds.
pub fn storage(builder: ApplicationBuilder) -> ApplicationBuilder {
    builder.module(vendas::SalesModule)
}

/// What every screen here has: what it shows, its buttons, what is there now and a line with the
/// last thing that happened.
pub(crate) fn layout(
    cx: &BuildCx<'_>,
    intro: &str,
    buttons: Vec<AnyView>,
    shown: Vec<String>,
    said: String,
) -> AnyView {
    let narrow = cx.viewport().width < 720.0;
    let lines: Vec<AnyView> = shown.into_iter().map(|line| Box::new(Text::new(line)) as AnyView).collect();
    Box::new(
        VStack::new((
            Text::new(intro.to_owned()).foreground(Semantic::InkMuted),
            HStack::new(buttons).spacing(8.0).wrap(),
            VStack::new(lines).spacing(4.0).align(Align::Start),
            Text::new(said),
        ))
        .spacing(12.0)
        .padding(if narrow { 0.0 } else { 16.0 })
        .align(Align::Stretch),
    )
}

/// A button that does this when pressed.
pub(crate) fn button(caption: &str, run: impl Fn() + 'static) -> AnyView {
    Box::new(Button::new(caption.to_owned()).style(ButtonStyle::Secondary).on_click(run))
}
