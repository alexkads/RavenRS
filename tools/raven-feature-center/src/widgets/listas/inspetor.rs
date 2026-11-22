//! `DataInspector`, every feature: orders as a table, a tree, JSON and a schema; the item and
//! the field chosen kept from one way to the next; a field one order lacks told from one that
//! is null; a whole number of 19 digits exact; a long list cut with a notice; and the path of
//! the field picked, written on a line the tour reads.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DataInspectorDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Inspetor de dados";

/// Three orders, as a node of a flow would give them.
const ORDERS: &str = r#"[
    { "pedido": 101, "cliente": { "nome": "Ana", "cidade": "Recife" }, "itens": [ { "produto": "Café", "preço": 18.5 } ], "nota": 1234567890123456789, "desconto": null },
    { "pedido": 102, "cliente": { "nome": "Bia", "cidade": "Natal" }, "itens": [], "nota": 98765 },
    { "pedido": 103, "cliente": { "nome": "Caio" }, "itens": [ { "produto": "Pão", "preço": 7 }, { "produto": "Leite", "preço": 6.2 } ] }
]"#;

/// The page.
#[derive(Debug, Default)]
pub struct InspectorPage;

impl View for InspectorPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mode = cx.use_state(|| 0_usize);
        let item = cx.use_state(|| 0_usize);
        let chosen = cx.use_state(|| None::<String>);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let long = cx.use_state(|| false);
        let heard = said.clone();
        let inspector = if long.get() {
            DataInspector::of((0..5000).map(|i| serde_json::json!({ "linha": i })).collect())
        } else {
            DataInspector::new(ORDERS)
        }
        .mode(mode.binding())
        .item(item.binding())
        .selected(chosen.binding())
        .on_pick(move |path| heard.set(format!("Escolheu {path}")))
        .limit(100)
        .label("Pedidos");
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Toggle::new(long.binding()).label("Cinco mil linhas"),
                VStack::new((inspector,)).width(720.0),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
                Text::new(format!("Modo {} · item {}", mode.get(), item.get() + 1)),
            ))
            .spacing(12.0)
            .padding(16.0)
            .align(Align::Start),
        )
    }
}
