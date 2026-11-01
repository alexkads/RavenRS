//! Records the training films from the real app, and checks the ones in the app
//! still open. Recording runs when asked, after screens change:
//!
//! ```text
//! FF_RECORD_TRAINING=1 cargo test -p family-finance --test training
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs::movie::{Movie, OverlayKind, RecordError, Recorder, Region};
use ravenrs::prelude::*;

const SIZE: Size = Size::new(1280.0, 800.0);

fn script(take: &mut Recorder) -> Result<(), RecordError> {
    take.chapter("Conhecendo o Painel", "O resumo do mês: saldo, receitas, despesas e o que vem por aí.");
    take.wait(1.0);
    take.note("Painel", "Painel", "Tudo começa aqui: o resumo das suas finanças.", 3.0)?;

    take.chapter("Lançar uma despesa", "Como registrar o que você gastou.");
    take.guide(
        "Lançamentos",
        "Abra os Lançamentos",
        "Clique em Lançamentos, onde fica tudo o que entrou e saiu.",
    )?;
    take.click("Lançamentos")?;
    take.guide("Nova despesa", "Comece uma despesa", "Clique em Nova despesa.")?;
    take.click("Nova despesa")?;
    take.wait(0.8);
    take.type_into("Descrição", "Mercado da semana")?;
    take.type_into("Valor", "187,50")?;
    take.guide("Salvar lançamento", "Salve", "Confira e clique em Salvar lançamento.")?;
    take.click("Salvar lançamento")?;
    take.wait(1.5);

    take.chapter("Pagar uma conta", "Como dar baixa no que venceu.");
    take.note(
        "Contas a pagar",
        "Contas a pagar",
        "Aqui ficam todas as contas, e o número mostra as vencidas.",
        2.5,
    )?;
    take.guide("Painel", "Volte ao Painel", "As próximas contas também aparecem no Painel.")?;
    take.click("Painel")?;
    take.wait(0.8);
    take.move_to("Saldo nos últimos 12 meses")?;
    take.scroll(420.0);
    take.guide("Pagar", "Pague uma conta", "Clique em Pagar na conta que você quitou.")?;
    take.click("Pagar")?;
    take.wait(1.0);
    take.guide(
        "Confirmar pagamento",
        "Confirme",
        "Confira o valor e a conta, e clique em Confirmar pagamento.",
    )?;
    take.click("Confirmar pagamento")?;
    take.wait(1.5);

    take.chapter("Ver para onde foi o dinheiro", "Os relatórios mostram o mês por categoria.");
    take.click("Relatórios")?;
    take.wait(2.0);

    take.chapter("Guardar seus dados", "Faça um backup e guarde fora deste computador.");
    take.guide("Ajustes", "Abra os Ajustes", "Clique em Ajustes.")?;
    take.click("Ajustes")?;
    take.wait(1.0);
    take.note(
        "Fazer backup",
        "Fazer backup",
        "Salva um arquivo com todos os seus dados. Guarde em um pen drive ou na nuvem.",
        3.5,
    )?;
    take.note("Restaurar backup", "Restaurar backup", "Traz os dados de volta, aqui ou em outro computador. Antes de trocar, o app mostra o que tem no arquivo.", 3.5)?;
    take.wait(1.0);
    Ok(())
}

/// The finishing touches, the same ones the editor offers.
fn edit(film: Movie) -> Movie {
    let mut project = film.project().clone();
    let total = project.duration_ms();
    project.add_overlay(OverlayKind::FadeOut, total.saturating_sub(800), 800);
    project.add_overlay(OverlayKind::PointerHighlight, 0, total);
    let spot = film.hotspots().iter().find(|spot| spot.title == "Salve").map(|spot| spot.from_ms);
    if let Some(at) = spot {
        // Near what is being typed, which is small on a large screen.
        let dialog = Region::new(360.0, 180.0, 560.0, 440.0);
        project.add_overlay(
            OverlayKind::Caption { text: "Só a descrição, o valor e a conta são essenciais.".to_owned() },
            at.saturating_sub(4000),
            3500,
        );
        project.add_overlay(OverlayKind::Zoom { region: dialog }, at.saturating_sub(5000), 5000);
    }
    let shift = 3000;
    project.insert_title(0, "O essencial do Raven Finanças", "Do primeiro lançamento ao backup", shift);
    project.add_overlay(OverlayKind::FadeIn, 0, 600);
    film.with_project(project)
}

fn record() -> Movie {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let app = App::new(db, today, None, Updater::idle());
    let mut take = Recorder::new(app, Theme::raven_light(), SIZE);
    script(&mut take).unwrap_or_else(|e| panic!("the script no longer fits the app: {e}"));
    edit(take.finish().expect("film"))
}

#[test]
fn the_films_in_the_app_open_and_teach_each_part() {
    if std::env::var_os("FF_RECORD_TRAINING").is_some() {
        let film = record();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/treinamentos.ravenmovie");
        std::fs::write(path, film.to_bytes()).expect("written");
    }
    let bytes = include_bytes!("../assets/treinamentos.ravenmovie");
    let film = Movie::from_bytes(bytes);
    if std::env::var_os("FF_RECORD_TRAINING").is_some() {
        return;
    }
    let film = film.expect("the film opens");
    assert_eq!(film.chapters().len(), 5);
    assert!(film.hotspots().len() >= 8);
    assert!(film.duration_ms() > 30_000);
    assert!(bytes.len() < 2_000_000, "vectors should stay small: {} bytes", bytes.len());
}

#[test]
fn the_script_still_fits_the_app() {
    let film = record();
    assert_eq!(film.chapters().len(), 5);
}
