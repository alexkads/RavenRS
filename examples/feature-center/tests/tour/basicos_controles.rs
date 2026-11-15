//! The steps of the pages in src/widgets/basicos/controles/.

use super::*;
use feature_center::widgets::basicos::controles::{botoes, campos_texto, escolhas, ligacoes, numeros};

#[test]
fn the_buttons_count_load_toggle_and_answer_their_shortcut() {
    let mut c = Center::memory("ana");
    c.press("Botões");
    c.press("Contar (0)");
    c.press("Contar (1)");
    assert!(c.says("Cliques: 2"), "on_click: {:?}", c.texts());
    c.press("Excluir tudo");
    assert!(c.says("Perigo: nada foi excluído"), "the danger style still clicks: {:?}", c.texts());
    c.press("Enviar");
    assert!(c.says("Enviando…"), "loading: {:?}", c.texts());
    c.press("Concluir o envio");
    assert!(c.says("Envio concluído"), "the button off while nothing is sent turns on: {:?}", c.texts());
    c.press("Negrito");
    assert!(c.says("Negrito ligado"), "selected: {:?}", c.texts());
    c.press("Apagar o rascunho");
    assert!(c.says("Rascunho apagado"), "an icon-only button goes by its accessible name");
    c.key(ravenrs_core::Key::Function(8));
    assert!(c.says("Salvo pelo atalho F8"), "the key the button shows works: {:?}", c.texts());
    c.press("Fixar o painel");
    assert!(c.says("Painel fixado"), "an icon button that stays on: {:?}", c.texts());
    c.press("Excluir a linha");
    assert!(c.says("Linha excluída"));
    c.press(botoes::PERIODS[2]);
    assert!(c.says("Período: Mês"), "the group keeps each button whole: {:?}", c.texts());
    c.activate("Abertas");
    assert!(c.says("Filtro abertas: não"), "a toggle button flips: {:?}", c.texts());
    c.activate("Ao centro");
    assert!(c.says("alinhado ao centro"), "single: {:?}", c.texts());
    c.activate("Itálico");
    c.activate("Sublinhado");
    assert!(c.says("2 marcas"), "multiple: {:?}", c.texts());
    c.press("Continuar");
    assert!(c.says("Continuar clicado"));
}

#[test]
fn the_choices_switch_check_pick_and_mark() {
    let mut c = Center::memory("ana");
    c.press("Escolhas");
    assert!(c.says("E-mail: sim"), "{:?}", c.texts());
    c.activate("Avisos por e-mail");
    assert!(c.says("E-mail: não"), "a switch: {:?}", c.texts());
    c.activate("Som nos avisos");
    assert!(c.says("som: sim"), "a small switch: {:?}", c.texts());
    c.activate("Conta verificada (só leitura)");
    c.activate("Cópia de segurança diária (do plano)");
    assert!(c.has_name("Conta verificada (só leitura)"), "read only stays readable");
    c.activate("Aceito os termos de uso");
    assert!(c.says("termos: sim"), "a check box: {:?}", c.texts());
    c.activate(escolhas::PLANS[2]);
    assert!(c.says("plano: Equipe"), "a radio: {:?}", c.texts());
    c.activate(escolhas::DENSITIES[0]);
    assert!(c.says("densidade: Compacta"), "a segment: {:?}", c.texts());
    c.activate("Retrato quente");
    assert!(c.says("favorito: sim"), "the heart: {:?}", c.texts());
    c.activate("Projeto principal");
    assert!(c.says("estrela: não"), "the star: {:?}", c.texts());
}

#[test]
fn the_pick_lists_filter_pick_by_value_and_run_commands() {
    let mut c = Center::memory("ana");
    c.press("Listas de escolha");
    assert!(c.says("Destino: Brasil"), "{:?}", c.texts());
    assert!(c.says("cidade não escolhida"));
    assert!(c.says("moeda BRL"), "Select::of holds the value: {:?}", c.texts());
    c.activate("Moeda");
    c.activate("Euro");
    assert!(c.says("moeda EUR"), "a value picked by value: {:?}", c.texts());
    c.press("Abrir a paleta de comandos");
    // The search box goes by the palette's name; "Procure um comando" is what it shows.
    c.type_into("Paleta de comandos", "etiqueta");
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Comando: Imprimir etiqueta"), "the palette filters and runs: {:?}", c.texts());
}

#[test]
fn the_text_fields_submit_recall_validate_and_check_a_code() {
    let mut c = Center::memory("ana");
    c.press("Campos de texto");
    c.type_into("Procurar clientes", "Bia");
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Procurou: Bia"), "on_submit: {:?}", c.texts());
    c.key(ravenrs_core::Key::Escape);
    assert!(c.says("Busca cancelada"), "on_cancel: {:?}", c.texts());
    c.type_into("Comando do console", "status");
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Rodou: status"), "{:?}", c.texts());
    assert!(c.says("1 comandos enviados"), "recall keeps what was sent: {:?}", c.texts());
    c.key(ravenrs_core::Key::PageUp);
    assert!(c.says("Ajuda do console pedida"), "intercept_keys: {:?}", c.texts());
    c.press("Criar conta");
    assert!(c.says("Recusado"), "an empty required field refuses: {:?}", c.texts());
    c.type_into("E-mail", "ana@");
    assert!(c.says("Escreva um e-mail como nome@exemplo.com"), "validate: {:?}", c.texts());
    c.type_into("E-mail", "raven.dev");
    c.type_into("Usuário", campos_texto::TAKEN);
    assert!(c.says("já está em uso"), "error_text: {:?}", c.texts());
    assert!(c.says("Vale 10% na primeira compra"), "note_text");
    assert!(c.says("O rótulo diz: Telefone"), "Label::text");
    let boxes = c_code_name(&mut c);
    c.type_into(&boxes, campos_texto::CODE);
    assert!(c.says("Código 123456 conferido"), "on_complete: {:?}", c.texts());
}

/// The name the boxes of the code go by, as the center's language says it.
fn c_code_name(c: &mut Center) -> String {
    c.settle();
    let tree = c.ui.access_tree();
    tree.nodes
        .iter()
        .filter(|n| n.role == Role::TextInput)
        .filter_map(|n| n.name.clone())
        .find(|name| name.contains("ódigo"))
        .unwrap_or_else(|| panic!("no code boxes: {:?}", c.texts()))
}

#[test]
fn the_numbers_step_zoom_and_turn_pages() {
    let mut c = Center::memory("ana");
    c.press("Números e faixas");
    assert!(c.says("Volume 40"), "{:?}", c.texts());
    assert!(c.says("no meio"), "value_format: {:?}", c.texts());
    assert!(c.says("5500 K"), "{:?}", c.texts());
    assert!(c.says("2 lugares"));
    assert!(c.says("Zoom: 100%"), "{:?}", c.texts());
    assert!(c.says(&format!("página 1 de {}", numeros::PAGES_IN_LIST)));
    c.press("Aumentar");
    assert!(c.says("3 lugares"), "the stepper: {:?}", c.texts());
    c.press("Aproximar");
    assert!(!c.says("Zoom: 100%"), "the zoom buttons move the view: {:?}", c.texts());
    c.press("Encaixar");
    assert!(c.says("Zoom: cabe na tela"), "fit: {:?}", c.texts());
    c.press("Page 3");
    assert!(c.says("página 3 de 12"), "a page number: {:?}", c.texts());
    c.press("Avançar");
    assert!(c.says("folha 5"), "the pagination with its own labels: {:?}", c.texts());
}

#[test]
fn the_dates_pick_a_day_and_offer_periods() {
    let mut c = Center::memory("ana");
    c.press("Datas");
    assert!(c.says("Reserva: nenhuma"), "{:?}", c.texts());
    c.activate("14 novembro de 2026");
    assert!(c.says("Reserva: nenhuma"), "a day before the range is not picked: {:?}", c.texts());
    c.activate("20 novembro de 2026");
    assert!(c.says("Reserva para 20/11/2026"), "on_pick: {:?}", c.texts());
    assert!(c.says("Reserva: 20/11/2026"), "{:?}", c.texts());
    c.activate("Período do relatório");
    c.activate("Últimos 7 dias");
    assert!(c.says("período: 09/11/2026 a 15/11/2026 (7 dias)"), "a preset: {:?}", c.texts());
}

#[test]
fn the_colors_pick_swap_and_switch_the_theme() {
    let mut c = Center::memory("ana");
    c.press("Cores e temas");
    assert!(c.says("Frente #111827 sobre fundo #FFFFFF"), "{:?}", c.texts());
    c.activate("Verde");
    assert!(c.says("Frente #16A34A sobre fundo #FFFFFF"), "the grid picks the front: {:?}", c.texts());
    c.activate("Floresta");
    assert!(c.says("Destaque: Floresta"), "a swatch: {:?}", c.texts());
    c.activate("Sem destaque");
    assert!(c.says("Destaque: nenhum"), "the none swatch: {:?}", c.texts());
    c.press("Trocar frente e fundo");
    assert!(c.says("Frente #FFFFFF sobre fundo #16A34A"), "the pair swaps: {:?}", c.texts());
    c.press("Cores padrão");
    assert!(c.says("Frente #111827 sobre fundo #FFFFFF"), "defaults: {:?}", c.texts());
    c.press("Frente");
    assert!(c.says("Escolher a cor da frente"), "on_foreground: {:?}", c.texts());
    c.press("Fundo");
    assert!(c.says("Escolher a cor do fundo"), "on_background: {:?}", c.texts());
    c.activate("Claro");
    assert!(c.says("Tema: Raven Light"), "the switcher: {:?}", c.texts());
    c.last_called("Vidro");
    assert!(c.says("Tema da loja: Vitrine da loja"), "the app's own themes: {:?}", c.texts());
}

#[test]
fn the_links_jump_and_the_nine_points_choose() {
    let mut c = Center::memory("ana");
    c.press("Ligações e posição");
    assert!(c.says("a partir do ponto no centro"), "{:?}", c.texts());
    c.activate("Documentação dos componentes");
    assert_eq!(c.ui.links().opened(), [ligacoes::DOCS], "a link leaves the app");
    c.activate("Ir para o resumo");
    assert!(c.ui.links().opened().len() == 1, "a link with # stays in the app");
    assert!(c.has_name("Arquivos"), "the turned name still reads the same");
    c.activate("Superior direito");
    assert!(c.says("a partir do ponto acima à direita"), "the nine points: {:?}", c.texts());
}

impl Center {
    /// Clicks the last of the things with this name, as the second of two switchers.
    fn last_called(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found =
            tree.nodes.iter().rev().find(|n| n.name.as_deref() == Some(name) && n.role != Role::Group);
        let id = found.unwrap_or_else(|| panic!("nothing called {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }
}
