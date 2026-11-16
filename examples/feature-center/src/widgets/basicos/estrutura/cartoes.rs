//! `Card`, `GlassCard`, `Section`, `PageHeader`, `PropertyRow`, `Divider` and `Footer`, every
//! feature: surfaces side by side, cards chosen with a click, the head of a page in its two sizes,
//! label and value rows, lines both ways and a footer that stacks when narrow. A line says what
//! was clicked.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "CardsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Cartões e seções";

/// The plans the cards offer, in order.
pub const PLANS: [&str; 3] = ["Básico", "Equipe", "Empresa"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn body(title: &str, text: &str) -> VStack<(Text, Text)> {
    VStack::new((
        Text::new(title.to_owned()).style(TextStyle::Headline),
        Text::new(text.to_owned()).foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
    .align(Align::Start)
}

/// The page: surfaces, the plan picked by card, headers, rows, lines and footers.
#[derive(Debug, Default)]
pub struct CardsPage;

impl View for CardsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let plan = cx.use_state(|| 1_usize);
        let event = cx.use_state(|| String::from("Nada clicado ainda"));

        // The same content on each kind of surface.
        let surfaces = Grid::new(
            if narrow { 1 } else { 3 },
            (
                Card::new(body("Padrão", "Sombra, borda e cantos do tema.")),
                Card::new(body("Sem sombra", "Plano, para dentro de outro painel.")).no_shadow(),
                Card::new(body("Sem borda", "Só a sombra levanta o cartão.")).border(false),
                Card::new(body("Cantos retos", "Raio 4, mais justo.")).corner_radius(4.0).padding(24.0),
                Card::new(body("Sombra alta", "Para o que flutua sobre a página.")).shadow(Shadow::raised()),
                Card::new(body("Preenchido", "Fundo da cor de controle.")).fill(Semantic::Control),
            ),
        )
        .spacing(12.0);

        let glass = HStack::new((
            GlassCard::new(body("Vidro", "Translúcido, sobre o fundo.")),
            GlassCard::new(body("Vidro justo", "Cantos 8 e menos respiro."))
                .corner_radius(8.0)
                .padding(10.0)
                .shadow(Shadow::soft()),
        ))
        .spacing(12.0);

        // Cards as a choice: clicking one picks it, and the chosen one shows it.
        let chosen = plan.get();
        let plan_card = |index: usize, price: &str| {
            let (pick, said) = (plan.clone(), event.clone());
            Card::new(body(PLANS[index], price)).selected(index == chosen).on_click(move || {
                pick.set(index);
                said.set(format!("Plano escolhido: {}", PLANS[index]));
            })
        };
        let plans = Grid::new(
            if narrow { 1 } else { 3 },
            (plan_card(0, "R$ 0 por mês"), plan_card(1, "R$ 49 por mês"), plan_card(2, "R$ 199 por mês")),
        )
        .spacing(12.0);

        // The head of a page, full and compact.
        let said_new = event.clone();
        let header = PageHeader::new("Pedidos")
            .description("Tudo o que entrou nesta semana, do mais novo ao mais antigo.")
            .badge(Badge::new("12 novos").tone(Tone::Accent))
            .action(Button::new("Novo pedido").icon(IconName::Plus).on_click(move || {
                said_new.set(String::from("Novo pedido aberto"));
            }));
        let compact = PageHeader::new("Estoque")
            .description("Visão de ferramenta, numa linha só")
            .badge(Badge::new("3 em falta").tone(Tone::Warning))
            .compact();

        // A section with a description and its own action, holding label and value rows.
        let said_edit = event.clone();
        let section = Section::new(
            "Conta",
            (
                PropertyRow::new("Nome", "Ana Souza"),
                PropertyRow::new("E-mail", "ana@exemplo.com.br"),
                PropertyRow::with("Plano", Badge::new(PLANS[chosen]).tone(Tone::Accent)),
                PropertyRow::with("Situação", Badge::new("Em dia").tone(Tone::Success)),
            ),
        )
        .description("Os dados que aparecem na nota fiscal.")
        .action(
            Button::new("Editar")
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || said_edit.set(String::from("Editar a conta"))),
        );

        // Lines: across the stack, and upright between items in a row.
        let lines = VStack::new((
            Text::new("Acima da linha"),
            Divider::horizontal(),
            Text::new("Abaixo da linha"),
            HStack::new((
                Text::new("Arquivo"),
                Divider::vertical(),
                Text::new("Editar"),
                Divider::vertical(),
                Text::new("Exibir"),
            ))
            .spacing(12.0)
            .height(24.0)
            .align(Align::Stretch),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // Footers: three areas that stack below the breakpoint, and one that always stacks.
        let said_privacy = event.clone();
        let said_terms = event.clone();
        let footer = Footer::new()
            .left((Footer::text("© 2026 Raven"),))
            .center((Footer::link("Privacidade").on_click(move || {
                said_privacy.set(String::from("Link do rodapé: Privacidade"));
            }),))
            .right((Footer::link("Termos").on_click(move || {
                said_terms.set(String::from("Link do rodapé: Termos"));
            }),))
            .breakpoint(900.0);
        let stacked = Footer::new()
            .left((Footer::text("Painel estreito"),))
            .right((Footer::text("versão 0.1"),))
            .divider(false)
            .stacked();

        cx.child(
            &VStack::new((
                VStack::new((heading("Superfícies"), surfaces, glass)).spacing(8.0),
                VStack::new((heading("Cartões que se escolhem"), plans)).spacing(8.0),
                VStack::new((heading("Cabeçalho de página"), header, compact)).spacing(12.0),
                section,
                VStack::new((heading("Linhas"), lines)).spacing(8.0),
                VStack::new((heading("Rodapés"), footer, stacked)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
