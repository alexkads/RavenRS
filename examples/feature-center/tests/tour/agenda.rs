//! The steps of the family `ravenrs-widgets-scheduler`: the pages Agenda por recurso,
//! Calendário de eventos and Linha do tempo.

use super::*;
use ravenrs_core::{Event, Key, Point, PointerButton};

impl Center {
    /// Gives the keyboard to the button with this name.
    fn focus_button(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
    }

    /// Presses the first button whose name has this text.
    fn press_containing(&mut self, text: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref().is_some_and(|t| t.contains(text)));
        let id = found.unwrap_or_else(|| panic!("no button with {text}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    /// Drags what has this name sideways by `by` logical pixels.
    fn drag_sideways(&mut self, name: &str, by: f32) {
        self.settle();
        let tree = self.ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let b = node.unwrap_or_else(|| panic!("no button {name}: {:?}", self.texts())).bounds;
        let from = Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 2.0);
        self.ui.handle_event(&Event::PointerMove { position: from });
        self.ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        self.ui.update();
        // A drag is told from a click by the time it lasts, as well as by how far it goes.
        let mut t = 1.0;
        for step in 1..=8 {
            let at = Point::new(from.x + by * step as f32 / 8.0, from.y);
            self.ui.handle_event(&Event::PointerMove { position: at });
            t += 0.05;
            self.ui.set_time(t);
            self.ui.update();
        }
        let to = Point::new(from.x + by, from.y);
        self.ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
        self.settle();
    }
}

#[test]
fn the_scheduler_selects_moves_refuses_opens_and_deletes_an_event() {
    let mut c = Center::memory("ana");
    c.press("Agenda por recurso");
    assert!(c.says("selecionado: nenhum"), "{:?}", c.texts());
    assert!(c.has_name("Reunião semanal"), "the weekly meeting, expanded from its rule: {:?}", c.texts());
    c.activate("Instalação");
    assert!(c.says("selecionado: Instalação"), "selection: {:?}", c.texts());
    c.focus_button("Instalação");
    c.key(Key::ArrowRight);
    assert!(c.says("Instalação foi para 04/11 00:00 até 07/11 00:00"), "on_change: {:?}", c.texts());
    c.key(Key::Enter);
    assert!(c.says("Abriu Instalação"), "on_open: {:?}", c.texts());

    c.activate("Workshop");
    c.focus_button("Workshop");
    c.key(Key::ArrowDown);
    assert!(c.says("Sala 2 está em manutenção"), "can_drop refuses the closed room: {:?}", c.texts());
    c.key(Key::Delete);
    assert!(c.says("Excluiu Workshop"), "on_delete: {:?}", c.texts());

    c.activate("Somente leitura");
    c.activate("Treinamento");
    c.focus_button("Treinamento");
    c.key(Key::ArrowRight);
    assert!(!c.says("Treinamento foi para"), "read_only moves nothing: {:?}", c.texts());
}

#[test]
fn the_event_calendar_picks_changes_view_creates_and_repeats_a_bill() {
    let mut c = Center::memory("ana");
    c.press("Calendário de eventos");
    assert!(c.says("Visão: mês"), "{:?}", c.texts());
    c.press("Aluguel");
    assert!(c.says("Clicou em Aluguel"), "on_event: {:?}", c.texts());
    assert!(c.says("Conta de luz"), "the bill that repeats: {:?}", c.texts());
    c.press("Ver o dia");
    assert!(c.says("Visão: dia") && c.has_name("Dentista"), "view: {:?}", c.texts());
    c.press_containing("15:00");
    assert!(c.says("Criou um compromisso em 16/11 15:00"), "on_create: {:?}", c.texts());
    assert!(c.has_name("Novo compromisso"), "and it shows");
    c.activate("Só o horário comercial");
    assert!(!c.texts().iter().any(|t| t.contains("05:00")), "hours: {:?}", c.texts());
}

#[test]
fn the_timeline_selects_a_block_and_moves_the_playhead() {
    let mut c = Center::memory("ana");
    c.press("Linha do tempo");
    assert!(c.says("Agulha em 0:04"), "position_ms: {:?}", c.texts());
    c.press("Entrevista");
    assert!(
        c.says("Escolheu Entrevista") && c.says("Escolhido: Entrevista, de 0:08 a 0:20"),
        "on_select: {:?}",
        c.texts()
    );
    c.press("Corte 2");
    assert!(c.says("Agulha em 0:20"), "a marker seeks: {:?}", c.texts());
    c.press("Avançar 5 s");
    assert!(c.says("Agulha em 0:25"), "{:?}", c.texts());
    c.press("Aproximar a régua");
    assert!(c.says("42 px por segundo"), "pixels_per_second: {:?}", c.texts());
    c.drag_sideways("Aplausos", 84.0);
    assert!(c.says("Escolhido: Aplausos, de 0:11 a 0:14"), "on_edit moved it two seconds: {:?}", c.texts());
}
