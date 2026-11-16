//! `Caixa`, the checkout of `ravenrs-app-pos`, every feature: the point of sale runs below as it
//! would at a counter, over the sandbox of the SEFAZ and a simulated card provider, and the buttons
//! above drive the same checkout the way its keys do: scan, take out, keep and pick up a sale, move
//! on and back, answer the provider, print the receipt, swap it for a NF-e, settle what an earlier
//! session left open. Native only, as the drivers of its devices are.

use ravenrs_app_ui::Session;

/// The control's name, as the session knows it.
pub const ID: &str = "PosDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "PDV de exemplo";

/// Registers the screen with the session.
pub fn register(session: &Session) {
    #[cfg(not(target_arch = "wasm32"))]
    native::register(session);
    #[cfg(target_arch = "wasm32")]
    session.register_control(ID, || {
        Box::new(ravenrs::prelude::Alert::new(
            PAGE,
            "O ponto de venda usa os drivers dos periféricos e só roda no desktop: cargo run -p feature-center.",
        )) as ravenrs::prelude::AnyView
    });
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::rc::Rc;

    use ravenrs::prelude::*;
    use ravenrs_app::{ObjectSpace, Query};
    use ravenrs_app_nfe::engine::BetaAck;
    use ravenrs_app_nfe::{NFE_EMITENTE, NfeService};
    use ravenrs_app_pos::{CLIENTE, CONFIGURACAO, PRODUTO, Pos, Tela, VIEW_CAIXA, tef_simulado};
    use ravenrs_app_ui::{App, Session};
    use ravenrs_pagamento::simulador::{cenarios, iniciar};
    use ravenrs_pagamento::{EntradaOperador, IdPagamento, Pergunta};
    use ravenrs_pos::{Forma, valor_em_texto};

    use super::super::button;
    use super::{ID, PAGE};

    /// The rice the scanner reads, the milk three at a time, and the customer of the large note.
    pub const RICE: &str = "7891000053508";
    const MILK: &str = "3*7891000100103";
    const CUSTOMER: &str = "11444777000161";

    /// What it sells and who it sells to, the first time.
    fn seed(space: &mut dyn ObjectSpace) -> Result<(), String> {
        let wrong = |error: ravenrs_app::SpaceError| error.to_string();
        if !space.query(PRODUTO, &Query::all()).map_err(wrong)?.is_empty() {
            return Ok(());
        }
        for (code, gtin, name, cents, ncm) in [
            ("1", RICE, "Arroz branco tipo 1, 5 kg", 2790.0, "10063021"),
            ("2", "7891000100103", "Leite integral 1 L", 549.0, "04012010"),
            ("3", "7891000100202", "Açúcar refinado 1 kg", 499.0, "17019900"),
        ] {
            let mut product = space.new_object(PRODUTO).map_err(wrong)?;
            product.set("codigo", code);
            product.set("gtin", gtin);
            product.set("descricao", name);
            product.set("preco", cents);
            product.set("ncm", ncm);
            space.save(product).map_err(wrong)?;
        }
        let address = [
            ("logradouro", "Rua de Exemplo"),
            ("numero", "100"),
            ("bairro", "Centro"),
            ("codigo_municipio", "3550308"),
            ("municipio", "Sao Paulo"),
            ("uf", "SP"),
            ("cep", "01001000"),
        ];
        let mut issuer = space.new_object(NFE_EMITENTE).map_err(wrong)?;
        for (field, value) in [
            ("razao_social", "MERCADO EXEMPLO LTDA"),
            ("nome_fantasia", "Mercado Exemplo"),
            ("cnpj", "11222333000181"),
            ("inscricao_estadual", "110042490114"),
        ]
        .into_iter()
        .chain(address)
        {
            issuer.set(field, value);
        }
        space.save(issuer).map_err(wrong)?;
        let mut customer = space.new_object(CLIENTE).map_err(wrong)?;
        customer.set("nome", "RESTAURANTE EXEMPLO LTDA");
        customer.set("documento", CUSTOMER);
        for (field, value) in address {
            customer.set(field, value);
        }
        space.save(customer).map_err(wrong)?;
        let mut settings = space.new_object(CONFIGURACAO).map_err(wrong)?;
        settings.set("operador", "Maria");
        space.save(settings).map_err(wrong)?;
        space.commit().map_err(|error| error.to_string())
    }

    /// The point of sale and its window, put together once per session.
    #[derive(Clone)]
    struct Counter {
        pos: Pos,
        window: Rc<AnyView>,
    }

    fn counter() -> Result<Counter, String> {
        let service =
            NfeService::sandbox(BetaAck::i_understand_this_is_experimental()).map_err(|e| e.to_string())?;
        let (tef, simulator) = tef_simulado();
        let pos = Pos::com_tef(service, tef, Some(simulator));
        let window = pos.instalar(App::new(PAGE)).seed(seed).build().map_err(|why| why.0)?;
        if let Some(session) = pos.sessao() {
            session.open_view(VIEW_CAIXA);
        }
        Ok(Counter { pos, window: Rc::new(window) })
    }

    /// What screen the checkout is on, in words.
    fn screen_of(tela: &Tela) -> String {
        match tela {
            Tela::Vendendo => "Vendendo".to_owned(),
            Tela::Pagando => "Pagando".to_owned(),
            Tela::Cobrando { pergunta: Some(Pergunta::Menu { titulo, opcoes }), .. } => {
                format!("Cobrando: o provedor pergunta {titulo} ({} opções)", opcoes.len())
            }
            Tela::Cobrando { pergunta: Some(_), .. } => "Cobrando: o provedor pergunta".to_owned(),
            Tela::Cobrando { mensagem, .. } => format!("Cobrando: {mensagem}"),
            Tela::Enviando => "Enviando a nota".to_owned(),
            Tela::Resultado(fim) if fim.autorizada => format!("Nota {} autorizada", fim.numero),
            Tela::Resultado(fim) => format!("Nota recusada: {}", fim.motivo),
        }
    }

    /// The buttons over the checkout and what it holds now.
    struct PosScreen {
        counter: Counter,
    }

    impl View for PosScreen {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let narrow = cx.viewport().width < 720.0;
            let said = cx.use_state(|| "Passe um produto, como no leitor.".to_owned());
            let caixa = self.counter.pos.caixa().clone();
            let act = |caption: &str, run: fn(&ravenrs_app_pos::Caixa, &Session) -> String| {
                let (pos, said) = (self.counter.pos.clone(), said.clone());
                button(caption, move || {
                    let text = match pos.sessao() {
                        Some(session) => run(pos.caixa(), &session),
                        None => "O caixa ainda não abriu.".to_owned(),
                    };
                    said.set(text);
                })
            };
            let buttons = vec![
                act("Passar arroz", |caixa, session| {
                    caixa.ler(session, RICE);
                    "Leu o arroz.".to_owned()
                }),
                act("Passar 3 leites", |caixa, session| {
                    caixa.ler(session, MILK);
                    "Leu 3 * leite.".to_owned()
                }),
                act("Passar código errado", |caixa, session| {
                    caixa.ler(session, "999");
                    "Leu 999.".to_owned()
                }),
                act("Cancelar último", |caixa, _| {
                    caixa.cancelar_ultimo();
                    "Tirou a última linha.".to_owned()
                }),
                act("Cancelar venda", |caixa, _| {
                    caixa.cancelar_venda();
                    "Venda jogada fora.".to_owned()
                }),
                act("Guardar venda", |caixa, session| match caixa.guardar(session) {
                    Ok(()) => "Venda guardada; o caixa está livre.".to_owned(),
                    Err(why) => why,
                }),
                act("Retomar a guardada", |caixa, session| match caixa.pendentes(session).first() {
                    Some((id, title)) => match caixa.retomar(session, id) {
                        Ok(()) => format!("Retomou: {title}."),
                        Err(why) => why,
                    },
                    None => "Nenhuma venda guardada.".to_owned(),
                }),
                act("Avançar (F9)", |caixa, session| {
                    caixa.avancar(session);
                    "Avançou.".to_owned()
                }),
                act("Voltar (Esc)", |caixa, _| {
                    if caixa.voltar() { "Voltou.".to_owned() } else { "Nada para voltar.".to_owned() }
                }),
                act("Cobrar no débito com pergunta", |caixa, session| {
                    if let Some(simulator) = &caixa.simulador {
                        simulator.use_once(iniciar(cenarios::menu_debito_credito));
                    }
                    if *caixa.tela.peek() == Tela::Vendendo {
                        caixa.avancar(session);
                    }
                    caixa.forma.set(Forma::Debito);
                    caixa.recebido.set(valor_em_texto(caixa.carrinho.peek().total()));
                    caixa.avancar(session);
                    "Mandou cobrar no débito.".to_owned()
                }),
                act("Responder a primeira opção", |caixa, session| {
                    caixa.responder(session, EntradaOperador::Opcao(0));
                    "Respondeu ao provedor.".to_owned()
                }),
                act("Imprimir o cupom", |caixa, session| match caixa.tela.get() {
                    Tela::Resultado(fim) => {
                        caixa.imprimir_cupom(session, &fim.chave);
                        "Cupom na tela, pronto para sair em PDF.".to_owned()
                    }
                    _ => "Termine uma venda antes.".to_owned(),
                }),
                act("Trocar por nota grande", |caixa, session| {
                    caixa.carrinho.update(|carrinho| carrinho.cliente = Some(CUSTOMER.to_owned()));
                    caixa.trocar_por_nota_grande(session);
                    "Pediu a troca por NF-e.".to_owned()
                }),
                act("Conferir pagamentos pendentes", |caixa, _| {
                    if let (Some(simulator), Ok(id)) =
                        (&caixa.simulador, IdPagamento::new("pdv-sessao-anterior"))
                    {
                        simulator.semear_pendente_de_sessao_anterior(&id, 499);
                    }
                    let found = caixa.recuperar_pagamentos();
                    if found.is_empty() { "Nenhum pagamento pendente.".to_owned() } else { found.join(" ") }
                }),
            ];
            let cart = caixa.carrinho.get();
            let kept = self.counter.pos.sessao().map_or(0, |session| caixa.pendentes(&session).len());
            let lines = [
                format!("Tela: {}", screen_of(&caixa.tela.get())),
                format!("Itens: {} · Total: R$ {}", cart.linhas.len(), valor_em_texto(cart.total())),
                format!("Vendas guardadas: {kept}"),
                caixa
                    .problema
                    .get()
                    .map_or_else(|| "Sem problema".to_owned(), |why| format!("Problema: {why}")),
            ];
            let status: Vec<AnyView> =
                lines.into_iter().map(|line| Box::new(Text::new(line)) as AnyView).collect();
            let intro = "Um ponto de venda inteiro, sobre o ambiente de testes da SEFAZ e um provedor de cartão simulado. \
                         Os botões fazem o que as teclas do caixa fazem; o caixa embaixo mostra o mesmo.";
            let window = Kept(self.counter.window.clone());
            cx.child(
                &VStack::new((
                    Text::new(intro).foreground(Semantic::InkMuted),
                    HStack::new(buttons).spacing(8.0).wrap(),
                    VStack::new(status).spacing(4.0).align(Align::Start),
                    Text::new(said.get()),
                    window.height(if narrow { 480.0 } else { 620.0 }),
                ))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
            )
        }
    }

    /// The window of the point of sale, built once and shown again on each rebuild.
    #[derive(Clone)]
    struct Kept(Rc<AnyView>);

    impl View for Kept {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            cx.child(self.0.as_ref().as_ref())
        }
    }

    pub fn register(session: &Session) {
        let made: Rc<std::cell::OnceCell<Result<Counter, String>>> = Rc::default();
        session.register_control(ID, move || match made.get_or_init(counter) {
            Ok(counter) => Box::new(PosScreen { counter: counter.clone() }) as AnyView,
            Err(why) => Box::new(Alert::new(PAGE, why.clone())) as AnyView,
        });
    }
}
