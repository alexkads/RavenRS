//! The steps of the pages in src/widgets/kit/campos/.

use super::*;
use feature_center::widgets::kit::campos::{arquivos, busca, filtro, passos, temas};
use ravenrs_core::Key;
use ravenrs_widgets::PickedFile;

/// Drops these files on the middle of what is called `name`, as a person dragging them there.
fn drop_on(c: &mut Center, name: &str, files: &[&str]) -> bool {
    c.settle();
    let tree = c.ui.access_tree();
    // The last of that name: a heading over a field has its name too.
    let found = tree.nodes.iter().rev().find(|n| n.name.as_deref() == Some(name));
    let at = found.unwrap_or_else(|| panic!("nothing called {name}: {:?}", c.texts())).bounds.center();
    let files = files.iter().map(|f| PickedFile::new(*f, f.as_bytes().to_vec())).collect();
    let took = c.ui.drop_files(Some(at), files);
    c.settle();
    took
}

/// Clicks the last button called `name`, for a page that has several, as the menus of themes.
fn press_last(c: &mut Center, name: &str) {
    c.settle();
    let tree = c.ui.access_tree();
    let found = tree.nodes.iter().rev().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
    let id = found.unwrap_or_else(|| panic!("no button {name}: {:?}", c.texts())).id;
    c.ui.perform_action(id, AccessAction::Click);
    c.settle();
}

fn typed(c: &mut Center, text: &str) {
    for character in text.chars() {
        c.key(Key::Character(character.to_string()));
    }
}

#[test]
fn the_form_fields_count_step_leave_rename_and_undo() {
    let mut c = Center::memory("ana");
    c.press("Campos de formulário");
    assert!(c.says("R$ 142,21"), "the total of 3 × R$ 49,90 less 5 %: {:?}", c.texts());
    assert!(c.says("(11) 98765-4321"), "the mask shows the phone: {:?}", c.texts());
    c.press("Aumentar");
    assert!(c.says("R$ 189,62"), "spin_buttons took a step: {:?}", c.texts());
    c.type_into("Quantidade", "");
    c.key(Key::Tab);
    assert!(c.says("Quantidade conferida ao sair do campo"), "on_leave: {:?}", c.texts());

    c.press("Renomear");
    typed(&mut c, " B");
    c.key(Key::Enter);
    assert!(c.says("Renomeado para"), "editing and on_rename: {:?}", c.texts());

    c.press("Desfazer nas observações");
    assert!(c.says("Observações: nada a desfazer"), "history starts empty: {:?}", c.texts());
    c.type_into("Observações para a entrega", " Ok");
    assert!(c.says(" OkEntregar pela manhã."), "typed at the caret, at the start: {:?}", c.texts());
    c.press("Desfazer nas observações");
    assert!(c.says("Observações: desfeito"), "the history kept outside undoes: {:?}", c.texts());
    assert!(!c.says(" OkEntregar"), "{:?}", c.texts());
    c.press("Escrever nas observações");
    assert_eq!(c.focused().as_deref(), Some("Observações para a entrega"), "focus_request");

    c.activate("Avaliação");
    assert!(c.says("Atendimento: 4 de 5"), "the tab of the stars: {:?}", c.texts());
}

#[test]
fn the_dates_and_times_pick_a_time_from_the_list() {
    let mut c = Center::memory("ana");
    c.press("Datas e horas");
    assert!(c.says("Reunião: 2026-11-20 às 09:30 · loja das 08:00 às 18:00"), "{:?}", c.texts());
    c.press("Escolher horário");
    c.activate("09:45");
    assert!(c.says("Reunião: 2026-11-20 às 09:45"), "step_minutes(15): {:?}", c.texts());
}

#[test]
fn the_search_asks_the_server_tags_count_models_change_and_the_scanner_reads() {
    let mut c = Center::memory("ana");
    c.press("Busca e etiquetas");
    c.activate("Cliente");
    assert!(
        c.says(&format!("O servidor recebeu \"\" e mandou {0} de até {0}", busca::LIMIT)),
        "remote and limit: {:?}",
        c.texts()
    );
    assert!(!c.has_name(&format!("Cliente {:03}", busca::LIMIT + 1)), "only the limit came");
    c.activate("Cliente 003");
    assert!(c.says("cliente: Cliente 003"), "{:?}", c.texts());
    c.activate(busca::USES[1]);
    assert!(c.says(&format!("1 de {} etiquetas: urgente", busca::MOST_TAGS)), "{:?}", c.texts());
    c.activate(busca::USES[2]);
    let labels = ravenrs_widgets::ModelPickerLabels::pt_br();
    c.press(&labels.test);
    assert!(c.says("Testado: Médio"), "on_test: {:?}", c.texts());
    assert!(c.says("Respondeu em 0,8 s"), "the result shows: {:?}", c.texts());
    c.press(&labels.stop_using);
    assert!(c.says("Em uso agora: Médio"), "on_use: {:?}", c.texts());
    c.activate(busca::USES[3]);
    c.type_into("Passe o código", "7891234");
    c.key(Key::Enter);
    assert!(c.says("Lido"), "on_scan: {:?}", c.texts());
    assert!(c.says("7891234"), "{:?}", c.texts());
}

#[test]
fn the_files_are_taken_refused_and_listed() {
    let mut c = Center::memory("ana");
    c.press("Arquivos e imagens");
    assert!(drop_on(&mut c, arquivos::IMPORT, &["precos.csv"]), "the zone takes files");
    assert!(c.says("Importadas: precos.csv"), "on_files: {:?}", c.texts());
    drop_on(&mut c, arquivos::IMPORT, &["foto.png"]);
    assert!(c.says("Recusado:"), "accept and on_refused: {:?}", c.texts());
    drop_on(&mut c, arquivos::QUIET, &["tudo.zip"]);
    assert!(c.says("Na área: tudo.zip"), "the quiet zone: {:?}", c.texts());
    // Its button sits inside the place that takes the files; the first such button is its.
    let tree = c.ui.access_tree();
    let button =
        tree.nodes.iter().find(|n| n.name.as_deref() == Some("Escolher arquivos")).map(|n| n.bounds.center());
    let took = c.ui.drop_files(button, vec![PickedFile::new("nota.pdf", b"%PDF".to_vec())]);
    assert!(took, "the upload takes files");
    assert!(c.says("1 anexos: nota.pdf"), "FileUpload keeps the list: {:?}", c.texts());
}

#[test]
fn the_filter_builder_reads_and_writes_the_expression() {
    let mut c = Center::memory("ana");
    c.press("Construtor de filtros");
    assert!(c.says(&format!("Expressão: {}", filtro::START)), "{:?}", c.texts());
    assert!(c.says("maior que"), "the line of the price: {:?}", c.texts());
    c.press("Só ativos de livros e jogos");
    assert!(c.says("Expressão: [Ativo]"), "{:?}", c.texts());
    c.press("Limpar o filtro");
    assert!(c.says("Expressão: nenhum filtro"), "{:?}", c.texts());
}

#[test]
fn the_wizard_refuses_a_step_and_asks_before_publishing_discarding_and_deleting() {
    let mut c = Center::memory("ana");
    c.press("Passo a passo e confirmação");
    c.press("Avançar");
    c.press("Avançar");
    assert!(c.says("Dê um título ao relatório"), "the check of the step: {:?}", c.texts());
    c.type_into("Título do relatório", "Vendas");
    c.press("Avançar");
    assert!(c.says("\"Vendas\" com dados de Vendas"), "{:?}", c.texts());
    c.press(passos::FINISH);
    c.press("Publicar agora");
    assert!(c.says("Relatório publicado"), "on_finish and ask_plainly: {:?}", c.texts());

    c.press("Cancelar");
    c.press("Descartar");
    assert!(c.says("Rascunho descartado"), "on_cancel and ask: {:?}", c.texts());

    c.press("Apagar a conta");
    c.press("Apagar para sempre");
    assert!(!c.says("Conta apagada"), "ask_typing waits for the word: {:?}", c.texts());
    c.type_into("", passos::WORD);
    c.press("Apagar para sempre");
    assert!(c.says("Conta apagada"), "{:?}", c.texts());
}

#[test]
fn the_theme_picker_puts_the_application_theme_and_the_editor_follows() {
    let mut c = Center::memory("ana");
    c.press("Temas e aparência");
    assert!(c.says("Tema na tela: Raven Dark"), "{:?}", c.texts());
    press_last(&mut c, "Tema");
    c.activate(temas::FOREST);
    assert!(c.says(&format!("Tema na tela: {}", temas::FOREST)), "of and theme: {:?}", c.texts());
    assert!(c.says("#2F9E5B"), "the editor shows the accent of the theme: {:?}", c.texts());
}
