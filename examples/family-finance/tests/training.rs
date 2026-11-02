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
use ravenrs::core::{AccessAction, AccessNode, AccessTree};
use ravenrs::movie::{Movie, MovieLibrary, OverlayKind, RecordError, Recorder, Region};
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
fn questions_in_plain_words_find_the_right_moment_of_the_film() {
    let film =
        Movie::from_bytes(include_bytes!("../assets/treinamentos.ravenmovie")).expect("the film opens");
    let library = MovieLibrary::new().film("Manual", film);
    let best = |question: &str| library.ask(question, 4).first().map(|a| a.moment.title.clone());
    let cases = [
        ("como lançar uma despesa", "Lançar uma despesa"),
        ("lancar gasto", "Lançar uma despesa"),
        ("pagar", "Pagar uma conta"),
        ("como pago uma conta?", "Pagar uma conta"),
        ("quitei", "Pague uma conta"),
        ("trocar de computador", "Restaurar backup"),
        ("fazer backup", "Fazer backup"),
        ("restaurar meus dados", "Restaurar backup"),
        ("ver os relatórios", "Ver para onde foi o dinheiro"),
    ];
    for (question, moment) in cases {
        assert_eq!(best(question).as_deref(), Some(moment), "{question}");
    }
    assert_eq!(best("cartão de crédito"), None);
    assert_eq!(best("como cadastrar uma compra"), None);
    assert_eq!(best("convidar outra pessoa da família"), None);
}

#[test]
fn the_script_still_fits_the_app() {
    let film = record();
    assert_eq!(film.chapters().len(), 5);
}

fn named<'a>(tree: &'a AccessTree, name: &str) -> Option<&'a AccessNode> {
    tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
}

#[test]
fn the_assistant_opens_the_film_from_any_screen() {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let mut ui = Ui::new(App::new(db, today, None, Updater::idle()), Theme::raven_light());
    ui.set_viewport(SIZE, 1.0);
    ui.update();

    let click = |ui: &mut Ui, name: &str| {
        let id = named(&ui.access_tree(), name).unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    click(&mut ui, "Assistente");
    click(&mut ui, "Como pagar uma conta?");
    assert!(named(&ui.access_tree(), "Estes momentos").is_some());
    click(&mut ui, "Pagar uma conta");
    let tree = ui.access_tree();
    assert!(named(&tree, "0:18 / 0:").is_some(), "the film opened at that moment");
    assert!(named(&tree, "Estes momentos").is_none(), "and the panel closed");
}

/// Runs only where the real model is, as it is more than a gigabyte:
/// `FF_AI_MODELS=folder cargo test --release -p family-finance --test training real -- --ignored --nocapture`.
#[test]
#[ignore = "needs the real model"]
fn real_model_choices() {
    use family_finance::ui::{ASSISTANT_INTRO, ASSISTANT_TOOLS};
    use ravenrs_ai::{LocalEngine, ModelFolder, model_by_id, read_tool_call, tool_rules};
    const CASES: &[(&str, &str)] = &[
        ("qual é o meu saldo?", "saldo_das_contas"),
        ("quanto dinheiro eu tenho nas contas?", "saldo_das_contas"),
        ("quanto gastei este mês?", "gastos_por_categoria"),
        ("quanto gastei com mercado?", "gastos_por_categoria"),
        ("onde estou gastando mais?", "gastos_por_categoria"),
        ("tenho alguma conta vencida?", "contas_a_pagar"),
        ("o que tenho para pagar essa semana?", "contas_a_pagar"),
        ("quais contas vencem em breve?", "contas_a_pagar"),
        ("abra os relatórios", "abrir_tela"),
        ("me leve para a agenda", "abrir_tela"),
        ("quero ver os orçamentos", "abrir_tela"),
        ("ir para os ajustes", "abrir_tela"),
        ("gastei 50 reais na padaria", "lancar_despesa"),
        ("lança uma despesa de 120 de farmácia", "lancar_despesa"),
        ("paguei 80 de gasolina hoje", "lancar_despesa"),
        ("criar uma despesa de 30 de lanche", "lancar_despesa"),
        ("anota uma despesa de 200 no cartão", "lancar_despesa"),
        ("agendar pagamento de luz para o dia 3", "criar_conta"),
        ("criar um pagamento para o dia 3 no valor de 100", "criar_conta"),
        ("tenho que pagar o aluguel dia 10", "criar_conta"),
        ("nova conta de internet de 120 para o dia 15", "criar_conta"),
        ("agendar pagamento de luz para o dia 3. mas eu quero criar", "criar_conta"),
        ("criar conta de luz de 330 para o dia 3, da Ana, todo mês", "criar_conta"),
        ("nova conta do carro de 12000 em 10x", "criar_conta"),
        ("gastei 200 no mercado ontem com o cartão", "lancar_despesa"),
        ("marca a conta de luz como paga", "pagar_conta"),
        ("paguei o streaming", "pagar_conta"),
        ("como faço um backup?", "ajuda"),
        ("como funciona o orçamento?", "ajuda"),
        ("como restauro meus dados em outro computador?", "ajuda"),
        ("qual a capital da França?", "nenhuma"),
        ("me conta uma piada", "nenhuma"),
    ];
    let root = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    // `FF_AI_MODEL` names which one, by its id.
    let wanted = std::env::var("FF_AI_MODEL").unwrap_or_else(|_| "qwen3-1.7b-q4".to_owned());
    let model = model_by_id(&wanted).expect("a known model");
    let mut engine = LocalEngine::load(&ModelFolder::new(root), &model).expect("the model");
    let rules = format!(
        "{}Categorias de gasto: Assinaturas, Contas da casa, Educação, Lazer, Mercado, Moradia, Saúde, Transporte.",
        tool_rules(ASSISTANT_INTRO, ASSISTANT_TOOLS)
    );
    let mut right = 0;
    for (question, wanted) in CASES {
        let mut answer = String::new();
        engine
            .generate(&rules, question, 50, &mut |piece| {
                answer.push_str(piece);
                true
            })
            .expect("an answer");
        let chosen = read_tool_call(&answer, ASSISTANT_TOOLS).map_or("?".to_owned(), |call| call.name);
        right += usize::from(chosen == *wanted);
        println!(
            "{} {question:?} -> {}",
            if chosen == *wanted { "OK " } else { "ERR" },
            answer.trim().replace('\n', " ")
        );
    }
    println!("{right}/{} right", CASES.len());
    assert!(right * 10 >= CASES.len() * 6, "too many wrong: {right}");
}

/// The whole way, with the real model: the same command as above.
#[test]
#[ignore = "needs the real model"]
fn real_model_answers_in_the_chat_with_the_numbers_of_the_database() {
    let models = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    let folder = std::env::temp_dir().join(format!("ff-assistant-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("modelos")).expect("folder");
    let wanted = std::env::var("FF_AI_MODEL").unwrap_or_else(|_| "qwen3-1.7b-q4".to_owned());
    let model = ravenrs_ai::model_by_id(&wanted).expect("a known model");
    std::os::unix::fs::symlink(format!("{models}/{}", model.id), folder.join("modelos").join(model.id))
        .expect("link");
    std::fs::write(folder.join("modelos/in-use"), model.id).expect("in use");
    let today = Date::new(2026, 11, 15).expect("valid date");
    let path = folder.join("financas.db");
    let db = Database::open(&path).expect("database");
    seed::load_demo(&db, today).expect("seed");
    let total: i64 = db.accounts().expect("accounts").iter().map(|a| a.balance_cents).sum();
    let mut ui = Ui::new(App::new(db, today, Some(path), Updater::idle()), Theme::raven_light());
    ui.set_viewport(SIZE, 1.0);
    ui.update();
    let click = |ui: &mut Ui, name: &str| {
        let id = named(&ui.access_tree(), name).unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    click(&mut ui, "Assistente");
    click(&mut ui, "Qual é o meu saldo?");
    let started = std::time::Instant::now();
    let wanted = format!("Você tem {} nas contas", family_finance::money::format_brl(total));
    while named(&ui.access_tree(), &wanted).is_none() {
        assert!(started.elapsed().as_secs() < 60, "no answer");
        std::thread::sleep(std::time::Duration::from_millis(50));
        ui.update();
    }
    let _ = std::fs::remove_dir_all(folder);
}
