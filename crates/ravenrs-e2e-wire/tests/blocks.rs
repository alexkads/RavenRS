//! The blocks past click and type: fill, select, expand, collapse, scroll and wait, as a
//! `.raventest` file writes them and as a session runs them.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_core::Size;
use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Command, Kind, Scenario};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, NodeId, Select, Text, TextField, VStack, View, ViewExt, VirtualList};

/// A form with a category, a note, a long list of rows and a line that says "Pronto" a moment
/// after the screen opens.
struct Screen {
    category: State<usize>,
    note: State<String>,
    opened: State<Option<usize>>,
}

impl View for Screen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let ready = cx.now() >= 0.3;
        if !ready {
            cx.wake_at(0.3);
        }
        let shown = self.opened.get().map_or_else(|| "Nenhuma".to_owned(), |i| format!("Escolhida {i}"));
        cx.child(&VStack::new((
            Select::new(self.category.binding(), ["Aluguel", "Mercado", "Lazer"]).test_id("categoria"),
            TextField::new(self.note.binding()).test_id("nota"),
            Text::new(if ready { "Pronto" } else { "Carregando" }).test_id("aviso"),
            Text::new(shown).test_id("aberta"),
            VirtualList::new(300, |i| Text::new(format!("Linha {i}")))
                .row_height(28.0)
                .key(|i| i as u64)
                .selection(self.opened.binding())
                .height(160.0),
        )))
    }
}

fn session() -> (E2e, State<usize>, State<String>) {
    let (category, note) = (State::new(0), State::new("rascunho".to_owned()));
    let screen = Screen { category: category.clone(), note: note.clone(), opened: State::new(None) };
    let e2e = E2e::new(screen, Theme::raven_light()).viewport(Size::new(480.0, 420.0), 1.0);
    (e2e, category, note)
}

const FILE: &str = "\
cenário: Os blocos novos
abrir Tela | Abrir
escolher #categoria = Mercado | Escolher a categoria
preencher #nota = Feira de sábado | Trocar a nota
esperar #aviso = Pronto | Esperar a tela
rolar #row:120 | Rolar até a linha 120
clicar #row:250 | Clicar numa linha que ainda não apareceu
conferir #aviso = Pronto | Conferir
";

#[test]
fn the_new_words_read_in_portuguese_and_write_in_english() {
    let read = Scenario::parse(FILE).unwrap();
    let kinds: Vec<Kind> = read.blocks.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        [Kind::Open, Kind::Select, Kind::Fill, Kind::Wait, Kind::Scroll, Kind::Click, Kind::Check]
    );
    let written = read.write();
    for word in
        ["select #categoria = Mercado", "fill #nota = Feira", "wait #aviso = Pronto", "scroll #row:120"]
    {
        assert!(written.contains(word), "{written}");
    }
    assert_eq!(Scenario::parse(&written).unwrap(), read);
    let both = Scenario::parse("scenario: x\nopen A\nexpandir #a\nrecolher #a\nesperar #a\n").unwrap();
    assert_eq!(both.blocks[3].text, None, "wait without text waits for the part to show");
    let wrong = Scenario::parse("scenario: x\nopen A\nexpand #a = b\n").unwrap_err();
    assert!(wrong.message.contains("não leva texto"), "{wrong}");
    let unknown = Scenario::parse("scenario: x\nopen A\npular #a\n").unwrap_err();
    assert!(unknown.message.contains("escolher") && unknown.message.contains("esperar"), "{unknown}");
}

#[test]
fn the_blocks_drive_the_screen_as_written() {
    let (mut e2e, category, note) = session();
    for block in &Scenario::parse(FILE).unwrap().blocks[1..] {
        block
            .command()
            .unwrap()
            .apply(&mut e2e)
            .unwrap_or_else(|f| panic!("{}: {f}\n{}", block.title, f.screen));
    }
    assert_eq!(category.get(), 1);
    assert_eq!(note.get(), "Feira de sábado");
    Command::Check { target: "#aberta".into(), text: "Escolhida 250".into() }.apply(&mut e2e).unwrap();
}

#[test]
fn a_part_that_does_not_open_says_so() {
    let (mut e2e, ..) = session();
    let refused = Command::Expand { target: "#nota".into() }.apply(&mut e2e).unwrap_err();
    assert_eq!(refused.code, "E2E_EXPECTATION");
    assert!(refused.message.contains("does not open"), "{}", refused.message);
}
