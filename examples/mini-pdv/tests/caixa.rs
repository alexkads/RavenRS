//! The whole checkout, used from the keyboard as at a counter, with no window, against the sandbox.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use mini_pdv::app_com_pos;
use ravenrs_app_nfe::engine::sandbox::{autorizacao, cenarios};
use ravenrs_app_pos::{Pos, Tela};
use ravenrs_core::{Event, Key, Modifiers, PointerButton, Size};
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

    fn clicar(&mut self, nome: &str) {
        let botao = self.ui.access_tree().nodes.into_iter().find(|node| node.name.as_deref() == Some(nome));
        let lugar = botao.unwrap_or_else(|| panic!("no button {nome}")).bounds.center();
        self.ui.handle_event(&Event::PointerMove { position: lugar });
        self.ui.handle_event(&Event::PointerDown { position: lugar, button: PointerButton::Primary });
        self.ui.handle_event(&Event::PointerUp { position: lugar, button: PointerButton::Primary });
        self.assentar();
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
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else {
        panic!(
            "no result: {:?} {:?} {:?}",
            b.pos.caixa().tela.peek(),
            b.pos.caixa().problema.peek(),
            b.pos.caixa().carrinho.peek().linhas.len()
        )
    };
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

fn vender_a_empresa(b: &mut Balcao) {
    b.abrir_caixa();
    b.com_alt('c');
    b.digitar("11444777000161");
    b.tecla(Key::Enter);
    b.bipar("7891000100202");
}

#[test]
fn a_sale_to_a_company_goes_out_as_a_large_note() {
    let mut b = balcao();
    vender_a_empresa(&mut b);
    b.tecla(Key::Function(8));
    assert!(b.diz("Nota grande"));
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(fim.autorizada, "{}: {}", fim.c_stat, fim.motivo);
    assert_eq!(&fim.chave[20..22], "55", "the model in the key");
    assert!(b.diz("Ver o DANFE"));
}

#[test]
fn a_large_note_for_someone_not_on_record_says_so() {
    let mut b = balcao();
    b.abrir_caixa();
    b.com_alt('c');
    b.digitar("52998224725");
    b.tecla(Key::Enter);
    b.bipar("7891000100202");
    b.tecla(Key::Function(8));
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.assentar();
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando);
    assert!(b.diz("não está cadastrado"));
}

#[test]
fn an_authorized_small_note_can_be_swapped_for_a_large_one() {
    let mut b = balcao();
    vender_a_empresa(&mut b);
    b.tecla(Key::Function(9));
    b.tecla(Key::Function(9));
    b.aguardar_nota();
    let Tela::Resultado(pequena) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert_eq!(&pequena.chave[20..22], "65");
    b.clicar("Trocar por nota grande (cancela a NFC-e)");
    b.aguardar_nota();
    let Tela::Resultado(grande) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(grande.autorizada, "{}: {}", grande.c_stat, grande.motivo);
    assert_eq!(&grande.chave[20..22], "55");
    assert_ne!(pequena.chave, grande.chave);
}

#[test]
fn the_large_note_and_the_receipt_come_as_models_of_the_standard_designer() {
    use ravenrs_app_nfe::{campos_faltando, danfe_report, modelos_de_danfe};
    let (nome, modelo) = modelos_de_danfe().remove(0);
    assert_eq!(nome, "DANFE A4");
    assert_eq!(modelo.page.width, ravenrs_report::PageSettings::a4().width);
    // The receipt's own requirement for a QR Code does not apply to a sheet.
    assert_eq!(campos_faltando(&danfe_report("x")), vec!["qr_code"]);
}

fn fornecedor() -> ravenrs_app_nfe::engine::sandbox::Fornecedor {
    use ravenrs_app_nfe::engine::{Endereco, Uf};
    ravenrs_app_nfe::engine::sandbox::Fornecedor {
        razao_social: "ATACADO EXEMPLO LTDA".to_owned(),
        cnpj: "11444777000161".to_owned(),
        inscricao_estadual: "110042490114".to_owned(),
        endereco: Endereco {
            logradouro: "Rua do Atacado".to_owned(),
            numero: "1".to_owned(),
            bairro: "Centro".to_owned(),
            codigo_municipio: "3550308".to_owned(),
            municipio: "Sao Paulo".to_owned(),
            uf: Uf::SP,
            cep: "01001000".to_owned(),
        },
        comprador_cnpj: "11222333000181".to_owned(),
        emitida_em: "2026-11-05T08:00:00-03:00".to_owned(),
    }
}

fn item_comprado(
    codigo: &str,
    gtin: Option<&str>,
    descricao: &str,
    quantidade: &str,
    valor: &str,
) -> ravenrs_app_nfe::engine::Item {
    ravenrs_app_nfe::engine::Item {
        codigo: codigo.to_owned(),
        gtin: gtin.map(str::to_owned),
        descricao: descricao.to_owned(),
        ncm: "04012010".to_owned(),
        cest: None,
        cfop: "5102".to_owned(),
        unidade: "UN".to_owned(),
        quantidade: quantidade.parse().unwrap(),
        valor_unitario: valor.parse().unwrap(),
        desconto: rust_decimal::Decimal::ZERO,
        csosn: ravenrs_app_nfe::engine::Csosn::S102,
        origem: 0,
    }
}

#[test]
fn a_purchase_goes_into_stock_a_new_product_gets_a_code_and_a_rebuy_suggests_its_label() {
    use ravenrs_app_nfe::engine::sandbox::gerar_nota_de_compra;
    let mut b = balcao();
    let session = b.pos.sessao().expect("a session");
    let entradas = b.pos.entradas().clone();
    let xml = gerar_nota_de_compra(
        &fornecedor(),
        vec![
            item_comprado("F-LEITE", Some("7891000100103"), "LEITE INTEGRAL 1L", "2", "36.00"),
            item_comprado("F-NOVO", None, "BISCOITO SEM CODIGO", "10", "2.00"),
        ],
    )
    .expect("a purchase");
    entradas.carregar(&session, &xml).expect("it reads");
    let previa = entradas.previa(&session);
    assert_eq!(previa[0].produto.as_deref(), Some("7"), "the milk is on record, by its barcode");
    assert_eq!(previa[1].produto, None, "the biscuit is new");

    // The box of the milk has 12 and gets a code of its own, worth 12 at the counter.
    entradas.fatores.update(|all| {
        all.insert(previa[0].numero, "12".to_owned());
    });
    entradas.codigos.update(|all| {
        all.insert(previa[0].numero, "FARDO-LEITE".to_owned());
    });
    entradas.quantidades_dos_codigos.update(|all| {
        all.insert(previa[0].numero, "12".to_owned());
    });
    assert_eq!(entradas.previa(&session)[0].custo.normalize().to_string(), "3");
    assert_eq!(entradas.confirmar(&session).expect("it goes in"), 2);
    assert_eq!(entradas.etiquetas.peek().get("7").map(String::as_str), Some("1"), "the label is suggested");

    b.assentar();
    b.abrir_caixa();
    b.bipar("FARDO-LEITE");
    let linha = b.pos.caixa().carrinho.peek().ultimo().cloned().expect("a line");
    assert_eq!(linha.codigo, "7");
    assert_eq!(linha.quantidade.to_string(), "12", "the code counts as 12");

    // The same invoice does not go in twice.
    assert!(entradas.carregar(&session, &xml).unwrap_err().contains("já entrou"));

    // The new product can be read at the counter by the code the store made for it.
    let space = session.frame().space();
    let produtos = space.borrow().query(ravenrs_app_pos::PRODUTO, &ravenrs_app::Query::all()).unwrap();
    let novo = produtos
        .iter()
        .find(|p| p.get("descricao") == ravenrs_app::Value::Text("BISCOITO SEM CODIGO".into()))
        .expect("the new product");
    let ravenrs_app::Value::Text(codigo) = novo.get("gtin") else { panic!("no code") };
    assert!(codigo.starts_with("29"), "{codigo}");
    b.bipar(&codigo);
    assert_eq!(
        b.pos.caixa().carrinho.peek().ultimo().map(|l| l.descricao.clone()).as_deref(),
        Some("BISCOITO SEM CODIGO")
    );
}

#[test]
fn an_invoice_for_someone_else_is_refused() {
    use ravenrs_app_nfe::engine::sandbox::gerar_nota_de_compra;
    let b = balcao();
    let session = b.pos.sessao().expect("a session");
    let outro = ravenrs_app_nfe::engine::sandbox::Fornecedor {
        comprador_cnpj: "04252011000110".to_owned(),
        ..fornecedor()
    };
    let xml = gerar_nota_de_compra(&outro, vec![item_comprado("A", None, "COISA", "1", "1.00")])
        .expect("a purchase");
    assert!(b.pos.entradas().carregar(&session, &xml).unwrap_err().contains("CNPJ"));
}

fn pagar_com(b: &mut Balcao, tecla_alt: char) {
    b.abrir_caixa();
    b.bipar("7891000100202");
    b.tecla(Key::Function(9));
    b.com_alt(tecla_alt);
    b.tecla(Key::Function(9));
}

fn operacoes(b: &Balcao) -> Vec<String> {
    let simulador = b.pos.caixa().simulador.clone().expect("a simulator");
    simulador.requests().iter().map(|r| format!("{:?}", r.operacao)).collect()
}

#[test]
fn a_card_approved_goes_into_the_note_and_is_confirmed_only_after_it() {
    let mut b = balcao();
    pagar_com(&mut b, '2');
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(fim.autorizada, "{}: {}", fim.c_stat, fim.motivo);
    let feitas = operacoes(&b);
    assert!(feitas.iter().any(|o| o.contains("Confirmar")), "confirmed after the note: {feitas:?}");
}

#[test]
fn a_card_refused_goes_back_to_the_payment() {
    use ravenrs_pagamento::simulador::{cenarios, iniciar};
    let mut b = balcao();
    b.pos.caixa().simulador.clone().expect("a simulator").use_once(iniciar(cenarios::negado));
    pagar_com(&mut b, '3');
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando);
    assert!(b.diz("não autorizado"));
}

#[test]
fn a_note_refused_after_the_card_undoes_the_card() {
    use ravenrs_pagamento::simulador::{cenarios, iniciar};
    let mut b = balcao();
    // Approved without the authorization code: an integrated card group without cAut is refused.
    b.pos.caixa().simulador.clone().expect("a simulator").use_once(iniciar(cenarios::c_aut_ausente));
    pagar_com(&mut b, '2');
    b.aguardar_nota();
    let Tela::Resultado(fim) = b.pos.caixa().tela.peek().clone() else { panic!("no result") };
    assert!(!fim.autorizada, "the note is refused");
    let feitas = operacoes(&b);
    assert!(feitas.iter().any(|o| o.contains("Desfazer")), "the card was undone: {feitas:?}");
}

#[test]
fn the_provider_can_ask_and_the_operator_answers() {
    use ravenrs_pagamento::simulador::{cenarios, iniciar};
    let mut b = balcao();
    b.pos.caixa().simulador.clone().expect("a simulator").use_once(iniciar(cenarios::menu_debito_credito));
    pagar_com(&mut b, '2');
    assert!(matches!(*b.pos.caixa().tela.peek(), Tela::Cobrando { pergunta: Some(_), .. }));
    b.tecla(Key::Escape);
    assert_eq!(*b.pos.caixa().tela.peek(), Tela::Pagando, "Esc cancels with the provider");
}

#[test]
fn the_devices_say_their_exact_model_driver_version_and_how_far_they_were_tested() {
    let mut b = balcao();
    b.pos.sessao().expect("a session").open_view(ravenrs_app_pos::VIEW_PERIFERICOS);
    b.assentar();
    assert!(b.diz("Periféricos"));
    assert!(b.diz("L42 Pro"), "a label printer by its model");
    assert!(b.diz("PPC930"), "a pinpad by its model");
    assert!(b.diz("versão"));
    assert!(b.diz("documentação") || b.diz("Presumido"), "nothing was tested on a device yet");
    let trabalho = b
        .pos
        .caixa()
        .perifericos
        .trabalho("bematech.mp-4200-th.cupom", ravenrs_report_label::Language::EscPos);
    assert_eq!(trabalho.language, ravenrs_report_label::Language::EscPos);
}

#[test]
fn purchases_addressed_to_the_store_come_from_the_sefaz() {
    use ravenrs_app_nfe::engine::sandbox::gerar_nota_de_compra;
    let mut b = balcao();
    let xml = gerar_nota_de_compra(
        &fornecedor(),
        vec![item_comprado("F-CAFE", None, "CAFE ESPECIAL", "4", "20.00")],
    )
    .expect("a purchase");
    b.pos
        .caixa()
        .service
        .sandbox_ativo()
        .expect("a sandbox")
        .colocar_nota_recebida(&xml)
        .expect("it is there");
    b.pos.sessao().expect("a session").open_view(ravenrs_app_pos::VIEW_ENTRADA);
    b.assentar();
    b.clicar("Buscar na SEFAZ");
    let sessao = b.pos.sessao().expect("a session");
    sessao.finish_background();
    b.assentar();
    assert!(b.diz("1 nota de compra encontrada na SEFAZ."), "found it");
    let encontradas = b.pos.entradas().encontradas.peek().clone();
    assert_eq!(encontradas.len(), 1);
    assert!(encontradas[0].0.contains("ATACADO EXEMPLO"));
}

#[test]
fn the_sample_purchase_is_waiting_at_the_sandbox() {
    let mut b = balcao();
    mini_pdv::compra_de_exemplo(&b.pos).expect("it is put there");
    b.pos.sessao().expect("a session").open_view(ravenrs_app_pos::VIEW_ENTRADA);
    b.assentar();
    b.clicar("Buscar na SEFAZ");
    b.pos.sessao().expect("a session").finish_background();
    b.assentar();
    let encontradas = b.pos.entradas().encontradas.peek().clone();
    assert_eq!(encontradas.len(), 1, "{encontradas:?}");
    b.clicar(&format!("Abrir: {}", encontradas[0].0));
    b.assentar();
    assert!(b.diz("ARROZ BRANCO T1 5KG"), "the purchase opens for the entry of goods");
    // Everything was delivered (ultNSU = maxNSU): the national environment asks for one hour, and
    // a new search, even by another emissor, is held back here instead of earning a 656.
    let enviadas = b.pos.caixa().service.sandbox_ativo().expect("a sandbox").requests().len();
    b.clicar("Buscar na SEFAZ");
    b.pos.sessao().expect("a session").finish_background();
    b.assentar();
    assert!(b.diz("aguarde até"), "held back");
    assert_eq!(b.pos.caixa().service.sandbox_ativo().expect("a sandbox").requests().len(), enviadas);
}
