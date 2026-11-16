//! `Audit`, every feature: the accessibility audit the tests run, run here over a small shop put
//! together in code, in a window of its own that no one sees. It walks the whole application or one
//! screen, with or without Tab, lists the items of its navigation, leaves one out, presses a button
//! on each screen and checks the contrast of another theme.

use ravenrs::prelude::*;
use ravenrs_app_a11y::{Audit, Report};
use ravenrs_app_ui::{App, Language, Session};
use ravenrs_widgets::Ui;

use super::{button, layout};

/// The control's name, as the session knows it.
pub const ID: &str = "AuditDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Auditoria de acessibilidade";

/// The item of the shop's navigation that one of the audits leaves out.
const PRODUCTS: &str = "Produtos do caixa";

/// A small shop, put together again for each audit, in a window no one sees.
fn counter() -> Result<Ui, String> {
    let view = App::new("Loja de exemplo")
        .language(Language::BrazilianPortuguese)
        .entity::<crate::model::CounterProduct>()
        .entity::<super::vendas::Sale>()
        .build()
        .map_err(|why| why.0)?;
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    Ok(ui)
}

/// What a report says, in a few lines.
fn told(what: &str, report: &Report) -> Vec<String> {
    let mut lines = vec![format!(
        "{what}: {} problemas em {} telas ({})",
        report.findings.len(),
        report.places.len(),
        report.places.join(", ")
    )];
    lines.extend(report.findings.iter().take(5).map(ToString::to_string));
    if report.is_clean() {
        lines.push("Nenhum problema encontrado.".to_owned());
    }
    lines
}

fn audited(what: &str, run: impl FnOnce(&mut Ui) -> Vec<String>) -> Vec<String> {
    match counter() {
        Ok(mut ui) => run(&mut ui),
        Err(why) => vec![format!("{what}: a janela não montou: {why}")],
    }
}

/// The screen.
struct AuditScreen;

impl View for AuditScreen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let shown = cx.use_state(Vec::<String>::new);
        let said = cx.use_state(|| "Escolha uma auditoria da loja de exemplo.".to_owned());
        let act = |caption: &'static str, run: fn(&mut Ui) -> Vec<String>| {
            let (shown, said) = (shown.clone(), said.clone());
            button(caption, move || {
                shown.set(audited(caption, run));
                said.set(format!("Rodou: {caption}"));
            })
        };
        let buttons = vec![
            act("Auditar a loja inteira", |ui| {
                let audit = Audit::new().theme(Theme::raven_light()).open("Novo");
                told("A loja inteira, com o tema claro e o Novo de cada tela", &audit.app(ui))
            }),
            act("Só a tela de abertura", |ui| {
                told("A tela de abertura", &Audit::new().screen(ui, "abertura"))
            }),
            act("Sem passar o Tab", |ui| {
                told("A tela de abertura, sem o Tab", &Audit::new().without_focus().screen(ui, "abertura"))
            }),
            act("Listar os itens", |ui| {
                let all = Audit::new().items(ui);
                let without = Audit::new().skip(PRODUCTS).items(ui);
                vec![
                    format!("Itens da navegação: {}", all.join(", ")),
                    format!(
                        "Deixando de fora {PRODUCTS}: {}",
                        if without.is_empty() { "nenhum".to_owned() } else { without.join(", ") }
                    ),
                ]
            }),
            act("Auditar um item", |ui| told(PRODUCTS, &Audit::new().open("Novo").item(ui, PRODUCTS))),
        ];
        let intro = "A mesma auditoria dos testes, rodada aqui sobre uma loja pequena, montada em código, numa janela que ninguém vê: \
                     nomes, identificadores, foco, atalhos e contraste dos temas.";
        let view = layout(cx, intro, buttons, shown.get(), said.get());
        cx.child(view.as_ref())
    }
}

/// Registers the screen with the session.
pub fn register(session: &Session) {
    session.register_control(ID, || Box::new(AuditScreen) as AnyView);
}
