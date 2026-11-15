//! `List`, `Table`, `CardView` and `TileLayout`, every feature (#579): the best customers of a
//! shop, the items of an order, a catalog of cards to pick and open, and a panel of tiles that
//! can be moved, resized and locked. A line says what happened last.

use std::collections::BTreeSet;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "CollectionsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Listas e tabelas";

/// How cards are picked, in the order of `SelectionMode`.
pub const MODES: [&str; 3] = ["Nenhum", "Um", "Vários"];
/// The products of the catalog.
pub const PRODUCTS: [&str; 8] =
    ["Café 500 g", "Filtro de papel", "Caneca", "Moedor", "Chaleira", "Prensa francesa", "Balança", "Bule"];

/// A heading and what goes under it, closer together than the sections are.
fn section<C: ViewSeq>(children: C) -> VStack<C> {
    VStack::new(children).spacing(8.0).align(Align::Stretch)
}

/// The page: the collections and a line with the last event.
#[derive(Debug, Default)]
pub struct CollectionsPage;

impl View for CollectionsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Abra um cartão com Enter ou um clique duplo.".to_string());
        let mode = cx.use_state(|| 2_usize);
        let picked = cx.use_state(BTreeSet::<usize>::new);
        let locked = cx.use_state(|| false);
        let spots = cx.use_state(|| {
            vec![
                TileSpot::new("vendas", 2, 1),
                TileSpot::new("estoque", 1, 1),
                TileSpot::new("clientes", 1, 1),
            ]
        });

        let customer =
            |name: &'static str, orders: &'static str, total: &'static str, line: &State<String>| {
                let line = line.clone();
                ListItem::new(name)
                    .avatar(name)
                    .subtitle(orders)
                    .trailing(total)
                    .on_click(move || line.set(format!("Cliente: {name}")))
            };
        let customers = List::new((
            customer("Ana Lima", "12 pedidos", "R$ 1.920", &said).selected(true),
            customer("Bruno Reis", "8 pedidos", "R$ 1.104", &said),
            ListItem::new("Clientes novos").icon(IconName::Users).subtitle("Sem compra ainda").trailing("5"),
        ));

        let items = Table::new([
            Column::new("Produto").weight(2.0),
            Column::new("Qtd.").numeric(),
            Column::new("Total").numeric(),
        ])
        .row(["Café 500 g", "2", "R$ 59,80"])
        .row(["Filtro de papel", "1", "R$ 12,90"])
        .row(["Caneca", "4", "R$ 120,00"]);

        let opened = said.clone();
        let catalog = CardView::new(PRODUCTS.len(), |i| {
            Box::new(
                VStack::new((
                    Icon::new(IconName::Package).size(24.0),
                    Text::new(PRODUCTS[i]).style(TextStyle::Headline),
                    Text::new(format!("R$ {},90", 10 + i * 7)).foreground(Semantic::InkMuted),
                ))
                .spacing(6.0)
                .padding(12.0),
            ) as AnyView
        })
        .card_width(if narrow { 140.0 } else { 180.0 })
        .card_height(if narrow { 100.0 } else { 120.0 })
        .height(if narrow { 240.0 } else { 280.0 })
        .selection_mode(match mode.get() {
            0 => SelectionMode::None,
            1 => SelectionMode::Single,
            _ => SelectionMode::Multiple,
        })
        .selection(picked.binding())
        .on_open(move |i| opened.set(format!("Aberto: {}", PRODUCTS[i])))
        .card_name(|i| format!("{} por R$ {},90", PRODUCTS[i], 10 + i * 7))
        .label("Catálogo");

        let help = said.clone();
        let tiles = TileLayout::new(spots.binding())
            .tile(
                Tile::new("vendas", "Vendas", Text::new("R$ 12.480 hoje · 64 pedidos")).accessory(
                    IconButton::new(IconName::Info)
                        .label("Sobre as vendas")
                        .on_click(move || help.set("Vendas: soma dos pedidos pagos".to_string())),
                ),
            )
            .tile(Tile::new("estoque", "Estoque", Text::new("42 itens abaixo do mínimo")))
            .tile(Tile::new("clientes", "Clientes", Text::new("5 novos esta semana")))
            .columns(if narrow { 2 } else { 3 })
            .min_column_width(if narrow { 120.0 } else { 160.0 })
            .row_height(if narrow { 110.0 } else { 130.0 })
            .locked(locked.get());
        let order = spots.with(|s| {
            s.iter().map(|t| format!("{} {}×{}", t.key, t.columns, t.rows)).collect::<Vec<_>>().join(", ")
        });

        let modes: AnyView = if narrow {
            Box::new(Select::new(mode.binding(), MODES).label("Seleção"))
        } else {
            Box::new(SegmentedControl::new(mode.binding(), MODES))
        };
        let chosen = picked.with(|p| p.iter().map(|&i| PRODUCTS[i]).collect::<Vec<_>>().join(", "));

        cx.child(
            &VStack::new((
                section((Text::new("Melhores clientes").style(TextStyle::Headline), customers)),
                section((Text::new("Itens do pedido").style(TextStyle::Headline), items)),
                section((
                    Text::new("Catálogo").style(TextStyle::Headline),
                    modes,
                    catalog,
                    Text::new(if chosen.is_empty() {
                        "Nenhum produto selecionado".to_string()
                    } else {
                        format!("Selecionados: {chosen}")
                    }),
                )),
                section((
                    Text::new("Painel").style(TextStyle::Headline),
                    Checkbox::new("Travar os quadros", locked.binding()),
                    tiles,
                    Text::new(format!("Quadros: {order}")),
                )),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
