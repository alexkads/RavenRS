//! `ExpressionEditor`, every feature: the fields and parameters it offers, one at a time and
//! many at once, the lines it shows, and the preview of what the expression works out to over a
//! sample order, or why it failed, worked out by `ravenrs-criteria` as the person types.

use ravenrs::prelude::*;
use ravenrs_criteria::{Criteria, Environment, Scope};

/// The control's name, as the session knows it.
pub const ID: &str = "ExpressionEditorDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Editor de expressões";

/// The order the preview works the expression out over.
fn sample() -> serde_json::Value {
    serde_json::json!({ "valor": 150, "cliente": "Ana", "pago": true })
}

/// The page.
#[derive(Debug, Default)]
pub struct ExpressionsPage;

impl View for ExpressionsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let text = cx.use_state(|| "[valor] * 2".to_owned());
        let env = Environment::new().parameter("Desconto", 10.0);
        let order = sample();
        let worked = Criteria::parse(&text.get())
            .map_err(|e| e.message)
            .and_then(|c| Scope::new(&env).row(&order).evaluate(&c).map_err(|e| e.to_string()));
        let editor = ExpressionEditor::new(text.binding())
            .field(FilterField::new("valor", "Valor", FilterFieldKind::Number))
            .fields([
                FilterField::new("cliente", "Cliente", FilterFieldKind::Text),
                FilterField::new("pago", "Pago", FilterFieldKind::Boolean),
            ])
            .parameter(FilterField::new("Desconto", "Desconto", FilterFieldKind::Number))
            .parameters([FilterField::new("Meta", "Meta do mês", FilterFieldKind::Number)])
            .rows(3);
        let editor = match &worked {
            Ok(value) => editor.preview(if value.is_null() { "vazio".to_owned() } else { value.to_text() }),
            Err(why) => editor.preview_error(why.clone()),
        };
        let (good, bad, empty) = (text.clone(), text.clone(), text.clone());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Sobre o pedido de exemplo: valor 150, cliente Ana, pago.")
                    .style(TextStyle::Callout),
                HStack::new((
                    Button::new("Usar um exemplo que dá certo")
                        .on_click(move || good.set("[valor] - ?Desconto".into())),
                    Button::new("Usar um exemplo que falha")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || bad.set("Nope(1)".into())),
                    Button::new("Limpar")
                        .style(ButtonStyle::Ghost)
                        .on_click(move || empty.set(String::new())),
                ))
                .spacing(8.0),
                VStack::new((editor,)).width(860.0),
            ))
            .spacing(12.0)
            .padding(16.0)
            .align(Align::Start),
        )
    }
}
