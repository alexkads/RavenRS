//! The steps of the pages in src/widgets/basicos/avisos/.

use super::*;
use feature_center::widgets::basicos::avisos::{alertas, menus, progresso};
use ravenrs_core::{Event, Key, Point, PointerButton};

impl Center {
    /// The middle of what is called exactly this.
    fn middle_of(&mut self, name: &str) -> Point {
        self.settle();
        let tree = self.ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name) && n.role != Role::Group);
        let b = node.unwrap_or_else(|| panic!("nothing called {name}: {:?}", self.texts())).bounds;
        Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 2.0)
    }

    /// Rests the pointer over what is called this.
    fn hover_on(&mut self, name: &str) {
        let at = self.middle_of(name);
        self.ui.handle_event(&Event::PointerMove { position: at });
        self.settle();
    }

    /// Clicks what is called this with the right button.
    fn right_click_on(&mut self, name: &str) {
        let at = self.middle_of(name);
        self.ui.handle_event(&Event::PointerMove { position: at });
        self.ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
        self.ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Secondary });
        self.settle();
    }
}

#[test]
fn the_alerts_answer_the_toasts_come_with_actions_and_the_people_take_turns() {
    let mut c = Center::memory("ana");
    c.press("Avisos");
    assert!(c.says("As 32 notas foram enviadas"), "a toned alert: {:?}", c.texts());
    assert!(c.says("Só a mensagem, quando um título repetiria o lugar."), "Alert::message");
    c.press("Tentar de novo");
    assert!(c.says("Sincronização refeita"), "the alert's action: {:?}", c.texts());
    c.press("Revisar");
    assert!(c.says("Revisão pedida pela faixa"), "the compact strip keeps its action: {:?}", c.texts());

    c.activate(alertas::CORNERS[1]);
    c.activate("Acima da barra de status");
    c.press("Mostrar aviso de sucesso");
    assert!(c.says("Avisos na tela: 1"), "show: {:?}", c.texts());
    assert!(c.says("Suas alterações estão guardadas."), "the toaster shows it: {:?}", c.texts());
    c.press("Aviso de 10 segundos");
    c.press("Aviso fixo");
    assert!(c.says("Avisos na tela: 3"), "{:?}", c.texts());
    assert!(c.says("Faltam três arquivos."));
    c.press("Aviso com ação");
    assert!(c.says("Uma fatura foi para a lixeira."), "{:?}", c.texts());
    c.press("Desfazer");
    assert!(
        c.says("Exclusão desfeita") || c.says("O aviso da exclusão saiu"),
        "the toast's button runs and closes it: {:?}",
        c.texts()
    );
    assert!(!c.says("Uma fatura foi para a lixeira."), "closed by its button");

    assert!(c.says("A etiqueta diz: Fatura paga"), "Badge::text: {:?}", c.texts());
    assert!(c.has_name("Atrasada") && c.has_name("Mercado"), "badges by tone and color");

    assert!(c.has_name(alertas::TEAM[0]), "the picture keeps the name");
    assert!(c.says("Escolhida: Ana Souza"));
    c.press("Escolher o próximo");
    assert!(c.says("Escolhida: Bia Lima"), "the ring moves: {:?}", c.texts());
}

#[test]
fn the_layers_open_close_their_own_way_and_explain() {
    let mut c = Center::memory("ana");
    c.press("Camadas por cima");

    c.press("Renomear o projeto");
    assert!(c.says("O nome aparece no topo de cada relatório."), "description: {:?}", c.texts());
    c.type_into("Nome do projeto", " 2");
    c.press("Salvar o nome");
    assert!(c.says("Projeto renomeado para Raven Finanças 2"), "content and action: {:?}", c.texts());
    c.press("Renomear o projeto");
    c.press("Fechar sem renomear");
    assert!(!c.has_name("Salvar o nome"), "close_label names the X, which closes it");
    c.press("Renomear o projeto");
    c.key(Key::Escape);
    assert!(!c.has_name("Salvar o nome"), "Escape closes it");

    c.press("Descartar as alterações");
    c.key(Key::Escape);
    assert!(c.has_name("Continuar editando"), "close_on_escape(false) keeps it open");
    assert!(!c.has_name("Fechar sem renomear"), "no X: close_button(false)");
    c.press("Descartar");
    assert!(c.says("Alterações descartadas"), "{:?}", c.texts());

    c.press("Abrir os filtros à esquerda");
    assert!(c.says("O que entra na lista de faturas."), "the sheet: {:?}", c.texts());
    c.press("Aplicar os filtros");
    assert!(c.says("Filtros aplicados"), "{:?}", c.texts());
    c.press("Abrir os detalhes à direita");
    assert!(c.says("Vence em 20/11"), "the right sheet: {:?}", c.texts());
    c.key(Key::Escape);

    c.press("Opções da exportação");
    c.press("Exportar agora");
    assert!(c.says("Exportação começou"), "the popover's content: {:?}", c.texts());
    c.press("Ver a recusa");
    assert!(c.says("Dois arquivos foram recusados"), "a bare popover above: {:?}", c.texts());
    c.key(Key::Escape);

    assert!(c.says("Salvar o arquivo"), "the tooltip names the button: {:?}", c.texts());
    c.press("Salvar");
    assert!(c.says("Salvo pelo botão"));
    assert!(c.says("Veja o valor na lei do município"), "HelpTip::how: {:?}", c.texts());
    assert!(!c.says("desde 2021"), "the card waits for the pointer");
    c.hover_on("@ana.souza");
    assert!(c.says("Contas a receber · desde 2021"), "the hover card: {:?}", c.texts());
}

#[test]
fn the_menus_run_check_open_from_outside_and_on_a_right_click() {
    let mut c = Center::memory("ana");
    c.press("Menus");
    c.press("Arquivo");
    c.activate("Novo");
    assert!(c.says("Arquivo › Novo"), "on_select: {:?}", c.texts());
    c.press("Abrir o menu Arquivo daqui");
    assert!(c.says("menu Arquivo aberto"), "open from outside: {:?}", c.texts());
    c.activate("Abrir…");
    assert!(c.says("Arquivo › Abrir · menu Arquivo fechado"), "{:?}", c.texts());
    c.press("Arquivo");
    c.activate("Exportar");
    c.activate("Como PDF");
    assert!(c.says("Exportar › PDF"), "the submenu: {:?}", c.texts());
    c.press("Arquivo");
    assert!(c.says("Sem conta de e-mail"), "the disabled item says why: {:?}", c.texts());
    c.key(Key::Escape);

    c.press("Exibir");
    c.activate("Mostrar a grade");
    assert!(c.says("Grade: escondida"), "a checked item: {:?}", c.texts());
    c.press("Mais ações");
    c.activate("Duplicar");
    assert!(c.says("Duplicado"), "the icon-only menu goes by its label: {:?}", c.texts());
    c.press("Novo lançamento");
    c.activate("Despesa");
    assert!(c.says("Nova despesa"));

    assert!(!c.has_name("Marcar como paga"), "the context menu waits for a right click");
    c.right_click_on(menus::ROW);
    c.activate("Marcar como paga");
    assert!(c.says("Fatura marcada como paga"), "the context menu: {:?}", c.texts());
}

#[test]
fn the_progress_advances_the_card_loads_and_the_opening_screen_goes_through_its_steps() {
    let mut c = Center::memory("ana");
    c.press("Progresso e espera");
    assert!(c.says("0 de 8 fotos"), "detail: {:?}", c.texts());
    for _ in 0..4 {
        c.press("Copiar mais duas");
    }
    assert!(c.says("8 de 8 fotos") && c.says("Cópia concluída"), "{:?}", c.texts());
    assert!(c.says("100%"), "the bar's value: {:?}", c.texts());
    c.press("Recomeçar a cópia");
    assert!(c.says("Cópia em 0%"));
    assert!(c.has_name("Cota de envios") && c.has_name("Meta do mês"), "the tones by label");

    assert!(!c.has_name("Ana Souza"), "the placeholder while it loads");
    c.activate("Cartão carregando");
    assert!(c.has_name("Ana Souza"), "the card comes in: {:?}", c.texts());

    c.ui.set_time(10.0);
    c.activate("Com o logotipo do app");
    c.press("Mostrar a abertura");
    assert!(c.has_name("Raven Finanças"), "the opening screen: {:?}", c.texts());
    // It covers the page: only what it says can be read.
    assert!(c.says(progresso::STEPS[0]) && c.says("0%"), "the status and the bar: {:?}", c.texts());
    assert!(!c.has_name("Mostrar a abertura"), "the page is out of reach under it");
    c.ui.set_time(10.0 + progresso::SPLASH_SECONDS / 2.0);
    assert!(c.says(progresso::STEPS[1]) && c.says("50%"), "the status follows: {:?}", c.texts());
    c.ui.set_time(10.0 + progresso::SPLASH_SECONDS + 1.0);
    assert!(c.says("A abertura terminou e o app ficou à mão"), "{:?}", c.texts());
}

#[test]
fn the_live_numbers_follow_the_period() {
    let mut c = Center::memory("ana");
    c.press("Números ao vivo");
    assert!(c.says("Período: Mês · receita R$ 48.200"), "{:?}", c.texts());
    assert!(c.says("+12,4%") && c.says("-0,6 p.p."), "the deltas: {:?}", c.texts());
    c.activate("Ano");
    assert!(c.says("Período: Ano · receita R$ 571.000 · 702 clientes novos"), "{:?}", c.texts());
    assert!(c.says("R$ 571.000"), "the stat follows: {:?}", c.texts());
}
