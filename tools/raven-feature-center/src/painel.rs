//! A panel declared in code with levels: each number in good standing, needing attention or
//! critical by its thresholds, over the records the center already has (#864). A number its reader
//! may not see says it is restricted, never zero.
use ravenrs_app_report::{AnalyticsPanel, IndicatorLevel, PanelChart, Thresholds};

use crate::model::CounterProduct;

/// The id its view starts with.
pub const NAME: &str = "LevelsPanel";
/// The page's navigation caption.
pub const PAGE: &str = "Painel com níveis";

/// Every feature of AnalyticsPanel: numbers and charts by type and by name, a filter, dates by
/// month, levels by limits and by a rule.
pub fn panel() -> AnalyticsPanel {
    AnalyticsPanel::new(NAME, PAGE)
        .navigation("Aplicação")
        .icon("gauge")
        // 410 in all, between 300 and 500: attention.
        .number_in("AppearanceRules", "Valor em aberto", "amount:Sum")
        .level(Thresholds::at_most(300.0, 500.0))
        // One late: anything above zero is critical.
        .number_in("AppearanceRules", "Atrasados", "")
        .filtered("[state] = 'late'")
        .level(Thresholds::at_most(0.0, 0.0))
        // Four records, from three up in good standing.
        .number_in("AppearanceRules", "Registros", "")
        .level(Thresholds::at_least(3.0, 1.0))
        .number::<CounterProduct>("Produtos do caixa", "")
        .level(Thresholds::level_with(|count| (count > 0.0).then_some(IndicatorLevel::Good)))
        .chart_in("AppearanceRules", PanelChart::Area, "Valor por situação", "name", "amount:Sum")
        .chart_in("EditorsDemo", PanelChart::Bar, "Por mês", "day", "money:Sum")
        .by("Month")
        .bar::<CounterProduct>("Preço por produto", "name", "price:Sum")
        .line::<CounterProduct>("Linha de preços", "name", "price:Sum")
        .pie::<CounterProduct>("Divisão de preços", "name", "price:Sum")
        .doughnut::<CounterProduct>("Rosca de preços", "name", "price:Sum")
}
