//! The news of each version (#744): the dialog every RavenRS app opens from the version at the foot
//! of its window, on the history of an example app, with no network.
use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app_update::ui::{NewsDialog, version_button, with_notice};
use ravenrs_app_update::{InstalledApp, Updater};

/// The control registered in the center.
pub const ID: &str = "NewsDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Novidades da versão";

/// An app with three versions, the middle one running, so the dialog shows a newer one, the
/// one installed and an older one. The page of the service uses it too.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Exemplo",
    package: "raven-exemplo",
    home_dir: ".raven-exemplo-do-centro",
    bundle_id: "dev.ravenrs.raven-exemplo",
    icons: "examples/raven-exemplo/assets",
    mac_category: "public.app-category.productivity",
    linux_categories: "Office;",
    source: "https://example.invalid/raven-exemplo/master.tar.gz",
    env_prefix: "RX",
    installed_env: "RAVEN_EXEMPLO_DO_CENTRO",
    news_urls: &[],
    history_urls: &[],
    installer_url: "https://example.invalid/raven-exemplo/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: "0.2.0",
    installed: false,
    history: r#"[
        {"versao":"0.3.0","data":"2026-11-20","titulo":"Relatórios agendados","novidades":["Um relatório pode ir por e-mail toda segunda."],"por_que_atualizar":"Para receber o resumo sem abrir o app.","contribuidores":["Alex S S Fonseca"],"issues":[744]},
        {"versao":"0.2.0","data":"2026-11-17","titulo":"Busca em todas as colunas","novidades":["A busca acha o texto em qualquer coluna.","O filtro lembra a última escolha."],"por_que_atualizar":"Para achar um lançamento sem saber onde ele está.","contribuidores":["Alex S S Fonseca","Matheus"],"issues":[729,731]},
        {"versao":"0.1.0","data":"2026-11-01","titulo":"Primeira versão","novidades":["Lançar despesas e receitas."],"por_que_atualizar":"O começo de tudo.","contribuidores":["Alex S S Fonseca"],"issues":[]}
    ]"#,
};

/// Every public feature of NewsDialog on one page.
#[derive(Debug, Default)]
pub struct NewsPage;

impl View for NewsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let updater = cx.use_state(|| Rc::new(Updater::idle(&APP))).get();
        let opened_file = cx.use_state(|| Rc::new(RefCell::new(String::new()))).get();
        let said = opened_file.clone();
        let dialog = cx
            .use_state(move || {
                NewsDialog::new().on_open_file(move |path| *said.borrow_mut() = path.display().to_string())
            })
            .get();
        let (installed, latest, details) = (dialog.clone(), dialog.clone(), dialog.clone());
        let installed_updater = updater.clone();
        let state = format!(
            "Novidades {} · versão {} da lista · detalhes {}",
            if dialog.is_open() { "abertas" } else { "fechadas" },
            dialog.at() + 1,
            if dialog.details_open() { "abertos" } else { "fechados" },
        );
        let file = opened_file.borrow().clone();
        let bar = with_notice(
            StatusBar::new().left((version_button(
                format!("{} · versão {}", APP.name, APP.version),
                &updater,
                &dialog,
            ),)),
            &updater,
            &dialog,
        );
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "O mesmo diálogo em todos os apps que se atualizam compilando: o que mudou, por que atualizar, quem contribuiu, as issues com link e, numa versão nova, como atualizar.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                Text::new("Abrir").style(TextStyle::Headline),
                HStack::new((
                    Button::new("Versão em uso").on_click(move || installed.show_installed(&installed_updater)),
                    Button::new("Mais recente").style(ButtonStyle::Secondary).on_click(move || latest.show_latest()),
                    Button::new("Detalhes de uma falha")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || details.show_details()),
                ))
                .spacing(8.0)
                .wrap(),
                Text::new(state).test_id("estado"),
                Text::new(if file.is_empty() {
                    "Abrir o registro: nenhum arquivo pedido".to_owned()
                } else {
                    format!("Abrir o registro: {file}")
                })
                .test_id("registro"),
                Text::new("Na barra do app").style(TextStyle::Headline),
                Text::new("A versão, à esquerda, abre as novidades dela; um aviso de versão nova aparece no meio, com seus botões.")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                bar,
                dialog.view(&updater),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
