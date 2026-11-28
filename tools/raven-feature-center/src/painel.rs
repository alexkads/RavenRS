//! A panel declared in code with levels: each number in good standing, needing attention or
//! critical by its thresholds, over the records the center already has (#864). A number its reader
//! may not see says it is restricted, never zero.
use ravenrs_app::Query;
use ravenrs_app_report::{AnalyticsPanel, IndicatorLevel, PanelChart, Thresholds};
use ravenrs_app_ui::Session;
use ravenrs_report::Document;

/// The records of the appearance rules page, read here by type: the struct's name is the
/// entity's, so the tiles below that take a type read the records the center already has.
#[derive(Debug, ravenrs_app::BusinessObject)]
#[allow(dead_code)]
pub struct AppearanceRules {
    /// What it is known by.
    #[field(key)]
    pub id: i64,
    /// What it is called.
    #[field(default_property)]
    pub name: String,
    /// Its state.
    pub state: String,
    /// How much, in cents.
    pub amount: i64,
}

/// The id its view starts with.
pub const NAME: &str = "LevelsPanel";
/// The page's navigation caption.
pub const PAGE: &str = "Painel com níveis";

/// Every feature of AnalyticsPanel: groups with a rule and a button, numbers and charts by type
/// and by name, one worked out in code, a filter, dates by month, levels by limits and by a rule.
pub fn panel() -> AnalyticsPanel {
    AnalyticsPanel::new(NAME, PAGE)
        .navigation("Aplicação")
        .icon("gauge")
        .group("Situações")
        .group_note("Em aberto pede atenção acima de R$ 300,00; qualquer atrasado é crítico.")
        .group_view("AppearanceRules_ListView", "Ver registros")
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
        // Worked out in code, from what it says it reads: the share that is late.
        .number_with("Parte atrasada (%)", &["AppearanceRules.state"], |space| {
            let all = space.query("AppearanceRules", &Query::all()).map_err(|e| e.to_string())?;
            let late = all.iter().filter(|record| record.text("state") == "late").count();
            Ok(late as f64 * 100.0 / all.len().max(1) as f64)
        })
        .level(Thresholds::at_most(10.0, 30.0))
        .group("Caixa")
        .number::<AppearanceRules>("Com observação", "")
        .filtered("Not IsNull([notes])")
        .level(Thresholds::level_with(|count| (count > 0.0).then_some(IndicatorLevel::Good)))
        .chart_in("AppearanceRules", PanelChart::Area, "Valor por situação", "name", "amount:Sum")
        .chart_in("EditorsDemo", PanelChart::Bar, "Por mês", "day", "money:Sum")
        .by("Month")
        .bar::<AppearanceRules>("Valor por registro", "name", "amount:Sum")
        .line::<AppearanceRules>("Linha de valores", "name", "amount:Sum")
        .pie::<AppearanceRules>("Divisão por situação", "state", "amount:Sum")
        .doughnut::<AppearanceRules>("Rosca por situação", "state", "amount:Sum")
}

/// The panel on paper, as the person signed in reads it.
pub fn document(session: &Session) -> Document {
    panel().document(session)
}
