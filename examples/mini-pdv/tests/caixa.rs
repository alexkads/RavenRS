//! The whole checkout, used from the keyboard as at a counter, with no window, against the sandbox.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use mini_pdv::app_com_pos;
use ravenrs_app_nfe::engine::sandbox::{autorizacao, cenarios};
use ravenrs_app_pos::{Pos, Tela};
use ravenrs_core::{Event, Key, Modifiers, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

struct Balcao {
    ui: Ui,
    pos: Pos,
}

fn balcao() -> Balcao {
    let (view, pos) = app_com_pos(None).expect("it builds");
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1366.0, 768.0), 1.0);
    let mut balcao = Balcao { ui, pos };
    balcao.assentar();
    balcao
}

impl Balcao {
    fn assentar(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn tecla(&mut self, key: Key) {
        self.ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        self.assentar();
    }

    fn com_alt(&mut self, letra: char) {
        let modifiers = Modifiers { alt: true, base_key: Some(letra), ..Modifiers::default() };
        self.ui.handle_event(&Event::KeyDown { key: Key::Character(letra.to_string()), modifiers });
        self.assentar();
    }

    fn digitar(&mut self, texto: &str) {
        for letra in texto.chars() {
            self.tecla(Key::Character(letra.to_string()));
        }
    }

    fn bipar(&mut self, codigo: &str) {
        self.digitar(codigo);
        self.tecla(Key::Enter);
    }

    fn diz(&mut self, texto: &str) -> bool {
        self.ui.access_tree().nodes.iter().any(|node| {
            node.name.as_deref().is_some_and(|name| name.contains(texto))
                || node.value.as_deref().is_some_and(|value| value.contains(texto))
        })
    }

    /// Waits for the document to come back from the sandbox.
    fn aguardar_nota(&mut self) {
        for _ in 0..2000 {
            self.assentar();
            if !matches!(self.pos.caixa().tela.peek().clone(), Tela::Enviando) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the document did not come back");
    }

    fn abrir_caixa(&mut self) {
        self.tecla(Key::Function(2));
        assert!(self.diz("TOTAL DA VENDA"), "the checkout is up");
    }
}

#[test]
fn everyone_sees_it_is_a_beta_on_a_sandbox_on_every_screen() {
    let mut b = balcao();
    assert!(b.diz("BETA") && b.diz("SANDBOX"), "the first screen says so");
    b.abrir_caixa();
    assert!(b.diz("BETA") && b.diz("SANDBOX"), "the checkout says so");
}

#[test]
fn a_sale_goes_from_the_scanner_to_an_authorized_note() {
    let mut b = balcao();
    b.abrir_caixa();
    b.bipar("7891000053508");
    assert!(b.diz("Arroz branco tipo 1, 5 kg"), "the last item shows");
    b.bipar("3*7891000100103");
    assert!(b.diz("Leite integral 1 L"));
    assert!(b.diz("R$ 44,37"), "27,90 + 3 x 5,49");
    b.tecla(Key::Function(9));
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando);
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(fim.autorizada, "{}: {}", fim.c_stat, fim.motivo);
    assert_eq!(fim.c_stat, 100);
    assert_eq!(fim.numero, 1);
    assert!(fim.motivo.starts_with("[SANDBOX]"), "{}", fim.motivo);
    assert_eq!(fim.chave.len(), 44);
    assert!(b.diz("Nota autorizada"));
    b.tecla(Key::Function(9));
    assert!(b.pos.caixa().carrinho.peek().vazio(), "the next sale starts clean");
}

#[test]
fn numbers_follow_each_other_and_a_rejected_one_comes_again() {
    let mut b = balcao();
    b.abrir_caixa();
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    b.tecla(Key::Function(9));

    b.pos.caixa().service.sandbox_ativo().expect("a sandbox").use_once(autorizacao(cenarios::assinatura_297));
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert_eq!((fim.autorizada, fim.c_stat), (false, 297));
    assert_eq!(fim.numero, 2);
    assert!(b.diz("Nota rejeitada"));

    // F9 takes it back to the payment with the sale as it was, and it goes through.
    b.tecla(Key::Function(9));
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando);
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(fim.autorizada, "{}: {}", fim.c_stat, fim.motivo);
    assert_eq!(fim.numero, 2, "a rejected number comes again");
}

#[test]
fn a_service_that_is_down_can_be_tried_again() {
    let mut b = balcao();
    b.abrir_caixa();
    b.pos.caixa().service.sandbox_ativo().expect("a sandbox").use_once(autorizacao(cenarios::paralisado_108));
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert_eq!((fim.autorizada, fim.tentar_de_novo, fim.c_stat), (false, true, 108));
}

#[test]
fn what_cannot_be_read_says_so_and_nothing_is_added() {
    let mut b = balcao();
    b.abrir_caixa();
    b.bipar("000");
    assert!(b.diz("Produto 000 não encontrado."));
    assert!(b.pos.caixa().carrinho.peek().vazio());
    b.tecla(Key::Function(9));
    assert!(b.diz("Passe ao menos um produto."));
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Vendendo);
}

#[test]
fn the_last_item_can_be_canceled_and_the_sale_thrown_away() {
    let mut b = balcao();
    b.abrir_caixa();
    b.bipar("7891000100202");
    b.bipar("7891000100301");
    b.tecla(Key::Function(6));
    assert_eq!(b.pos.caixa().carrinho.peek().linhas.len(), 1);
    b.tecla(Key::Function(11));
    assert!(b.pos.caixa().carrinho.peek().vazio());
}

#[test]
fn only_cash_gives_change_and_less_than_the_total_does_not_pay() {
    let mut b = balcao();
    b.abrir_caixa();
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.pos.caixa().recebido.set("1,00".to_owned());
    b.assentar();
    b.tecla(Key::Function(9));
    assert!(b.diz("Faltam R$ 3,99."));
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando);
    b.pos.caixa().recebido.set("10,00".to_owned());
    b.assentar();
    assert!(b.diz("R$ 5,01"), "the change shows as it is typed");
    b.com_alt('4');
    assert_eq!(b.pos.caixa().recebido.peek().as_str(), "4,99", "Pix is the exact amount");
}

#[test]
fn a_person_asks_for_their_cpf_on_the_note() {
    let mut b = balcao();
    b.abrir_caixa();
    b.com_alt('c');
    b.digitar("52998224725");
    b.tecla(Key::Enter);
    assert_eq!(b.pos.caixa().carrinho.peek().cliente.as_deref(), Some("52998224725"));
    assert!(b.diz("Consumidor 52998224725"));
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result: {:?} {:?} {:?}", b.pos.caixa().tela.peek(), b.pos.caixa().problema.peek(), b.pos.caixa().carrinho.peek().linhas.len()) };
    assert!(fim.autorizada, "{}: {}", fim.c_stat, fim.motivo);
}

#[test]
fn a_locked_window_needs_the_pin() {
    let mut b = balcao();
    b.abrir_caixa();
    b.tecla(Key::Function(12));
    assert!(b.diz("Caixa bloqueado"));
    assert!(!b.diz("TOTAL DA VENDA"));
    b.digitar("1234");
    b.tecla(Key::Enter);
    assert!(b.diz("TOTAL DA VENDA"), "what was on stays");
}

#[test]
fn the_screens_of_labels_products_and_the_keys_open() {
    let mut b = balcao();
    b.tecla(Key::Function(4));
    assert!(b.diz("Editar modelo no editor de relatórios"), "the labels");
    b.tecla(Key::Escape);
    b.tecla(Key::Function(3));
    assert!(b.diz("Arroz branco tipo 1, 5 kg"), "the products");
    b.tecla(Key::Escape);
    b.tecla(Key::Function(1));
    assert!(b.diz("Consulta de atalhos"));
}
