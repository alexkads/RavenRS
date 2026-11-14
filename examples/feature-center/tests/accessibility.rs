//! Every demonstration of the center reaches screen readers and the keyboard, as the one who signs
//! in sees it, with nothing written for them.
#![allow(clippy::expect_used)]

use ravenrs_app::ObjectSpace;
use ravenrs_app_a11y::Audit;
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

use feature_center::seed::PASSWORD;
use feature_center::{memory_database, watched};

fn settle(ui: &mut Ui) {
    for _ in 0..4 {
        ui.update();
    }
}

fn type_into(ui: &mut Ui, field: &str, text: &str) {
    settle(ui);
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
        .expect("the field")
        .id;
    ui.perform_action(id, AccessAction::Focus);
    for character in text.chars() {
        ui.handle_event(&Event::KeyDown {
            key: Key::Character(character.to_string()),
            modifiers: Modifiers::default(),
        });
    }
    settle(ui);
}

/// The center with nobody signed in yet.
fn opened() -> Ui {
    let database = memory_database().expect("the database");
    let app = watched(move || Box::new(database.space()) as Box<dyn ObjectSpace>, |_| {});
    let mut ui = Ui::new(app.build().expect("it puts together"), Theme::raven_dark());
    ui.set_viewport(Size::new(1600.0, 1200.0), 1.0);
    settle(&mut ui);
    ui
}

/// The center as `ana` sees it once she signs in.
fn signed_in() -> Ui {
    let mut ui = opened();
    type_into(&mut ui, "Nome", "ana");
    type_into(&mut ui, "Senha", PASSWORD);
    let tree = ui.access_tree();
    let enter = tree.find(Role::Button, "Entrar").expect("the button");
    ui.perform_action(enter.id, AccessAction::Click);
    settle(&mut ui);
    ui
}

/// What every screen is audited for: the themes the center offers, and the form behind `Novo`.
fn audit() -> Audit {
    Audit::new().theme(Theme::raven_light()).theme(Theme::raven_glass()).open("Novo").skip("Sair")
}

#[test]
fn the_sign_in_passes_the_audit() {
    audit().screen(&mut opened(), "entrar").assert_clean();
}

#[test]
fn the_start_screen_and_the_themes_pass_the_audit() {
    audit().screen(&mut signed_in(), "start").assert_clean();
}

/// One test per demonstration, so they run in parallel: walked one after another in a single test,
/// they took 30 s, and the quick check waited for that test alone (#347).
macro_rules! demonstrations {
    ($($test:ident => $name:literal,)*) => {
        /// Every demonstration that has a test of its own, as the navigation calls it.
        const DEMONSTRATIONS: &[&str] = &[$($name),*];

        $(
            #[test]
            fn $test() {
                audit().item(&mut signed_in(), $name).assert_clean();
            }
        )*
    };
}

demonstrations! {
    regras => "Regras",
    avisos_e_informacoes => "Avisos e informações",
    ao_sair_do_campo => "Ao sair do campo",
    regras_de_aparencia => "Regras de aparência",
    textos_por_chave => "Textos por chave",
    tipos_de_action => "Tipos de Action",
    todos_os_tipos => "Todos os tipos",
    campos_personalizados => "Campos personalizados",
    uma_lista_cinco_visoes => "Uma lista, cinco visões",
    variantes_e_filtros => "Variantes e filtros",
    edicao_na_lista => "Edição na lista",
    muitas_linhas => "Muitas linhas",
    colecoes_e_vinculos => "Coleções e vínculos",
    maquina_de_estados => "Máquina de estados",
    lembretes => "Lembretes",
    documentos_por_papel => "Documentos por papel",
    completa => "Completa",
    leve => "Leve",
    so_criacao => "Só criação",
    campos_de_fora => "Campos de fora",
    colecoes => "Coleções",
    dados_da_loja => "Dados da loja",
    caixa_de_exemplo => "Caixa de exemplo",
    leitores_de_tela_e_teclado => "Leitores de tela e teclado",
    pessoas => "Pessoas",
    eventos => "Eventos",
    paises => "Países",
    enderecos => "Endereços",
    relatorios => "Relatórios",
    paineis => "Painéis",
    usuarios => "Usuários",
    papeis => "Papéis",
    trilha_de_auditoria => "Trilha de auditoria",
    modelo_da_aplicacao => "Modelo da aplicação",
    editor_de_tokens => "Editor de tokens",
    simulacao_de_parcelas => "Simulação de parcelas",
    trocar_senha => "Trocar senha",
}

#[test]
fn every_demonstration_has_its_audit_test() {
    let items = audit().items(&mut signed_in());
    let missing: Vec<&String> =
        items.iter().filter(|item| !DEMONSTRATIONS.contains(&item.as_str())).collect();
    assert!(
        missing.is_empty(),
        "no audit test for {missing:?}: add each one to demonstrations! in this file"
    );
    let gone: Vec<&&str> =
        DEMONSTRATIONS.iter().filter(|name| !items.iter().any(|item| item == *name)).collect();
    assert!(gone.is_empty(), "the navigation no longer has {gone:?}: take them out of demonstrations!");
}
