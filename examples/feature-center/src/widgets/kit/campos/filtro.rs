//! `FilterBuilder`, every feature (#579): a filter of products over text, numbers, yes or no and
//! dates, with values to pick for "is any of", read and written in the expression language. The
//! expression it makes shows under it, and buttons put a ready one in or clear it.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "FilterBuilderDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Construtor de filtros";

/// The filter the page starts with.
pub const START: &str = "[Preco] > 10 And Contains([Nome], 'pro')";
/// A ready filter a button puts in.
pub const ACTIVE_ONLY: &str = "[Ativo] = True And [Categoria] In ('livros', 'jogos')";

fn fields() -> Vec<FilterField> {
    vec![
        FilterField::new("Nome", "Nome", FilterFieldKind::Text),
        FilterField::new("Preco", "Preço", FilterFieldKind::Number),
        FilterField::new("Ativo", "Ativo", FilterFieldKind::Boolean),
        FilterField::new("Cadastro", "Cadastro", FilterFieldKind::Date),
        FilterField::new("Categoria", "Categoria", FilterFieldKind::Text).values([
            LookupItem::new("livros", "Livros"),
            LookupItem::new("jogos", "Jogos"),
            LookupItem::new("musica", "Música"),
        ]),
    ]
}

/// The page: the builder, the expression it writes and buttons that change it from outside.
#[derive(Debug, Default)]
pub struct FilterPage;

impl View for FilterPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let filter = cx.use_state(|| START.to_string());
        let (ready, clear) = (filter.clone(), filter.clone());
        let expression = filter.get();
        let shown = if expression.trim().is_empty() { "nenhum filtro".to_string() } else { expression };
        cx.child(
            &VStack::new((
                Text::new("Filtro de produtos").style(TextStyle::Headline),
                FilterBuilder::new(filter.binding(), fields()),
                HStack::new((
                    Button::new("Só ativos de livros e jogos")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || ready.set(ACTIVE_ONLY.to_string())),
                    Button::new("Limpar o filtro")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || clear.set(String::new())),
                ))
                .spacing(8.0)
                .wrap(),
                Text::new(format!("Expressão: {shown}")),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
