//! A panel declared over a Rust struct: `AnalyticsPanel::bar::<Sale>` and the rest take the entity
//! from the type, so a renamed struct fails to compile instead of showing an empty tile.

use ravenrs_app::{BusinessObject, EntityInfo, FieldValue, Module, ObjectSpace, Query};
use ravenrs_app_report::AnalyticsPanel;
use ravenrs_criteria::DateTime;
use rust_decimal::Decimal;

/// What the panel adds up: one sale of a shop, with its day and its kind of product.
#[derive(Debug, BusinessObject)]
#[object(caption = "Vendas da loja", navigation = "Listas", icon = "dollar-sign")]
pub struct Sale {
    /// What it is known by.
    #[field(key)]
    pub id: i64,
    /// What was sold.
    #[field(caption = "Produto", default_property)]
    pub product: String,
    /// The kind of product, for the pie.
    #[field(caption = "Categoria")]
    pub category: String,
    /// When, for the line by month.
    #[field(caption = "Dia")]
    pub day: DateTime,
    /// How much, exact to the cent.
    #[field(caption = "Valor", decimals = 2)]
    pub amount: Decimal,
}

/// The panel's name, as the navigation shows it.
pub const PAGE: &str = "Vendas por mês";

/// The panel: a number, bars by category, a line by month, a pie and a doughnut, all over [`Sale`].
pub fn panel() -> AnalyticsPanel {
    AnalyticsPanel::new("SalesPanel", PAGE)
        .navigation("Listas")
        .icon("chart-pie")
        .number::<Sale>("Total vendido", "amount:Sum")
        .bar::<Sale>("Por categoria", "category", "amount:Sum")
        .line::<Sale>("Por mês", "day", "amount:Sum")
        .by("Month")
        .pie::<Sale>("Vendas por categoria", "category", "")
        .doughnut::<Sale>("Por produto", "product", "amount:Sum")
}

/// A sale as the seed gives it: product, category, year, month and day, and cents.
type SeedSale = (&'static str, &'static str, (i32, u32, u32), i64);

/// The sales of the panel, the first time the center opens.
const SALES: [SeedSale; 6] = [
    ("Arroz 5 kg", "Mercearia", (2026, 9, 3), 2790),
    ("Feijão 1 kg", "Mercearia", (2026, 9, 18), 879),
    ("Detergente", "Limpeza", (2026, 10, 2), 349),
    ("Arroz 5 kg", "Mercearia", (2026, 10, 21), 2790),
    ("Sabão em pó", "Limpeza", (2026, 11, 4), 1890),
    ("Café 500 g", "Bebidas", (2026, 11, 9), 1849),
];

/// Brings [`Sale`] and puts its records in place once, as a module of the application would.
#[derive(Debug)]
pub struct SalesModule;

impl Module for SalesModule {
    fn name(&self) -> &str {
        "CenterSales"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![Sale::entity_info()]
    }

    fn initial_records(&self, space: &mut dyn ObjectSpace) -> Result<(), String> {
        let entity = Sale::entity_info().name;
        if !space.query(&entity, &Query::all()).map_err(|e| e.to_string())?.is_empty() {
            return Ok(());
        }
        for (index, (product, category, (year, month, day), cents)) in SALES.into_iter().enumerate() {
            let mut sale = space.new_object(&entity).map_err(|e| e.to_string())?;
            sale.set("id", (index + 1) as f64);
            sale.set("product", product);
            sale.set("category", category);
            sale.set("day", DateTime::from_ymd(year, month, day).unwrap_or_else(DateTime::now));
            sale.set("amount", Decimal::new(cents, 2).to_value());
            space.save(sale).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
