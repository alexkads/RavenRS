//! `Chart`, `AreaChart`, `BarChart` and `ChartCard`, every feature: bars side by side, stacked
//! and lying down, lines and areas with each curve, pies and doughnuts, radars and radial bars,
//! a scatter and candles, and the small charts in cards. A line says what the cards were told.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ChartDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Gráficos";

/// The uses, in the order of the tabs.
pub const USES: [&str; 6] =
    ["Barras", "Linhas e áreas", "Pizza e rosca", "Radar e radial", "Dispersão e velas", "Cartões"];

/// How bars share a month, in the order of their switch.
pub const STACKS: [&str; 3] = ["Lado a lado", "Empilhadas", "Em 100%"];
/// How lines go from point to point, in the order of their switch.
pub const CURVES: [&str; 4] = ["Natural", "Monótona", "Reta", "Degraus"];

const MONTHS: [&str; 6] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho"];

/// The sales of each half of the year, so a change of period has something to animate.
fn sales(second_half: bool) -> [f64; 6] {
    if second_half { [52.0, 47.0, 61.0, 58.0, 66.0, 72.0] } else { [31.0, 38.0, 35.0, 44.0, 41.0, 49.0] }
}

fn thousands(v: f64) -> String {
    format!("{v:.0} mil")
}

/// The page: a tab per kind of chart, and a line with the last event.
#[derive(Debug, Default)]
pub struct ChartsPage;

impl View for ChartsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let stack = cx.use_state(|| 0_usize);
        let curve = cx.use_state(|| 0_usize);
        let second_half = cx.use_state(|| false);
        let card_tab = cx.use_state(|| 0_usize);
        let active = cx.use_state(|| 2_usize);
        let said = cx.use_state(|| "Escolha uma aba do cartão ou troque o período.".to_string());
        let narrow = cx.viewport().width < 720.0;
        let columns = if narrow { 1 } else { 2 };
        let half = second_half.get();
        let period = if half { "2º semestre" } else { "1º semestre" };

        let body: AnyView = match tab.get() {
            0 => {
                let stacked = [ChartStack::None, ChartStack::Normal, ChartStack::Expand][stack.get().min(2)];
                let picker: AnyView = if narrow {
                    Box::new(Select::new(stack.binding(), STACKS).label("Barras"))
                } else {
                    Box::new(SegmentedControl::new(stack.binding(), STACKS))
                };
                let next = active.clone();
                Box::new(
                    VStack::new((
                        HStack::new((
                            picker,
                            Button::new("Destacar o próximo mês")
                                .on_click(move || next.update(|i| *i = (*i + 1) % MONTHS.len())),
                        ))
                        .spacing(12.0)
                        .wrap(),
                        Grid::new(
                            columns,
                            (
                                // Two series, one in a color of its own and one with an icon.
                                Chart::new(ChartKind::Bar)
                                    .title("Vendas por mês")
                                    .categories(MONTHS)
                                    .series_color("Loja", Color::hex(0x2F9E6A), sales(false))
                                    .series("Site", [22.0, 25.0, 30.0, 28.0, 36.0, 40.0])
                                    .series_icon("Site", IconName::Globe)
                                    .stack(stacked)
                                    .bar_radius(4.0)
                                    .value_labels(ChartLabelAt::Outside)
                                    .reference(
                                        ChartReference::new(40.0).label("Meta").color(Semantic::Danger),
                                    )
                                    .legend_at(ChartLegend::Top)
                                    .format(thousands)
                                    .tick_format(|m| m.chars().take(3).collect())
                                    .show_value_axis()
                                    .value_ticks(5)
                                    .grid_dashed()
                                    .active(Some(active.get()))
                                    .height(260.0),
                                // Lying down, each product its own color, the names inside the bars.
                                Chart::new(ChartKind::Bar)
                                    .title("Produtos mais vendidos")
                                    .categories(["Café", "Pão de queijo", "Bolo", "Suco"])
                                    .series("Unidades", [320.0, 280.0, 2.0, 140.0])
                                    .horizontal()
                                    .vary_colors()
                                    .category_labels(ChartLabelAt::InsideStart)
                                    .hide_category_axis()
                                    .min_bar_length(6.0)
                                    .no_hover_focus()
                                    .hide_legend()
                                    .height(260.0),
                                // A result both ways from zero.
                                Chart::new(ChartKind::Bar)
                                    .title("Resultado mensal")
                                    .categories(MONTHS)
                                    .series("Resultado", [12.0, -8.0, 5.0, -3.0, 15.0, 9.0])
                                    .negative_color(Semantic::Danger)
                                    .labels_at_zero()
                                    .reference_line(10.0)
                                    .grid(ChartGrid::Both)
                                    .grid_columns(6)
                                    .height(240.0),
                                // From the right, in colors given one by one.
                                Chart::new(ChartKind::Bar)
                                    .title("Estoque por depósito")
                                    .categories(["Norte", "Centro", "Sul"])
                                    .series("Caixas", [48.0, 72.0, 35.0])
                                    .bars_from(ChartBarFrom::Right)
                                    .item_colors([Semantic::Accent, Semantic::Warning, Semantic::Success])
                                    .hide_legend()
                                    .height(240.0),
                            ),
                        )
                        .spacing(16.0),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                )
            }
            1 => {
                let shape = [ChartCurve::Natural, ChartCurve::Monotone, ChartCurve::Linear, ChartCurve::Step]
                    [curve.get().min(3)];
                let picker: AnyView = if narrow {
                    Box::new(Select::new(curve.binding(), CURVES).label("Curva"))
                } else {
                    Box::new(SegmentedControl::new(curve.binding(), CURVES))
                };
                let flip = second_half.clone();
                Box::new(
                    VStack::new((
                        HStack::new((
                            picker,
                            Button::new("Trocar o semestre").on_click(move || flip.update(|h| *h = !*h)),
                            Text::new(format!("Período: {period}")),
                        ))
                        .spacing(12.0)
                        .wrap(),
                        Grid::new(
                            columns,
                            (
                                Chart::new(ChartKind::Line)
                                    .title("Visitas")
                                    .categories(MONTHS)
                                    .series("Visitas", [120.0, 180.0, 150.0, 210.0, 190.0, 240.0])
                                    .curve(shape)
                                    .dots()
                                    .align_axis_ends()
                                    .tight_scale()
                                    .no_hover_motion()
                                    .height(240.0),
                                // Stacked areas that go to the other half's values, and draw in
                                // again on each period.
                                Chart::new(ChartKind::Area)
                                    .title("Receita acumulada")
                                    .categories(MONTHS)
                                    .series("Loja", sales(half))
                                    .series("Site", sales(!half))
                                    .stack(ChartStack::Normal)
                                    .curve(shape)
                                    .gradient()
                                    .fill_opacity(0.45)
                                    .value_domain(0.0, 160.0)
                                    .animate_changes()
                                    .appear_key(half)
                                    .height(240.0),
                                Chart::new(ChartKind::Line)
                                    .title("Avaliações")
                                    .categories(MONTHS)
                                    .series("Nota", [4.1, 4.3, 4.0, 4.6, 4.7, 4.8])
                                    .dot_icon(IconName::Star)
                                    .hide_tooltip()
                                    .height(220.0),
                            ),
                        )
                        .spacing(16.0),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                )
            }
            2 => Box::new(
                Grid::new(
                    columns,
                    (
                        Chart::new(ChartKind::Pie)
                            .title("Vendas por canal")
                            .categories(["Loja", "Site", "Aplicativo", "Telefone"])
                            .series("Vendas", [45.0, 30.0, 18.0, 7.0])
                            .slice_radii([1.0, 0.9, 0.8, 0.7])
                            .category_labels(ChartLabelAt::Outside)
                            .label_guides()
                            .active(Some(1))
                            .height(260.0),
                        Chart::new(ChartKind::Doughnut)
                            .title("Pedidos do dia")
                            .categories(["Entregues", "A caminho", "Atrasados"])
                            .series("Pedidos", [980.0, 210.0, 50.0])
                            .hole(0.62)
                            .center_text("1.240", "pedidos")
                            .no_separator()
                            .height(260.0),
                        // One ring per year.
                        Chart::new(ChartKind::Pie)
                            .title("Dois anos lado a lado")
                            .categories(["Loja", "Site", "Aplicativo"])
                            .series("2025", [50.0, 35.0, 15.0])
                            .series("2026", [40.0, 35.0, 25.0])
                            .concentric()
                            .hole(0.3)
                            .height(260.0),
                    ),
                )
                .spacing(16.0),
            ),
            3 => Box::new(
                Grid::new(
                    columns,
                    (
                        Chart::new(ChartKind::Radar)
                            .title("Avaliação da equipe")
                            .categories(["Prazo", "Qualidade", "Custo", "Atendimento", "Inovação"])
                            .series("Equipe A", [4.0, 5.0, 3.0, 4.0, 2.0])
                            .series("Equipe B", [3.0, 4.0, 4.0, 5.0, 4.0])
                            .polar_fill()
                            .rings(4)
                            .radius_axis()
                            .dots()
                            .height(280.0),
                        Chart::new(ChartKind::Radar)
                            .title("Só os contornos")
                            .categories(["Prazo", "Qualidade", "Custo", "Atendimento", "Inovação"])
                            .series("Meta", [5.0, 5.0, 4.0, 5.0, 4.0])
                            .lines_only()
                            .no_spokes()
                            .polar_grid(ChartPolarGrid::Circle)
                            .height(280.0),
                        Chart::new(ChartKind::RadialBar)
                            .title("Metas do trimestre")
                            .categories(["Vendas", "Clientes", "Satisfação"])
                            .series("Alcançado", [82.0, 64.0, 91.0])
                            .angles(0.0, 270.0)
                            .tracks()
                            .hole(0.3)
                            .center_text("79%", "média")
                            .height(280.0),
                    ),
                )
                .spacing(16.0),
            ),
            4 => Box::new(
                Grid::new(
                    columns,
                    (
                        // Each point where its distance puts it.
                        Chart::new(ChartKind::Scatter)
                            .title("Entrega por distância")
                            .positions([2.0, 4.5, 6.0, 8.0, 11.0, 14.0])
                            .categories(["2 km", "4,5 km", "6 km", "8 km", "11 km", "14 km"])
                            .series("Minutos", [18.0, 25.0, 31.0, 34.0, 45.0, 58.0])
                            .tooltip(ChartTooltip::new().indicator(ChartIndicator::Line).label("Entrega"))
                            .show_value_axis()
                            .height(260.0),
                        Chart::new(ChartKind::Candlestick)
                            .title("Cotação da semana")
                            .categories(["Seg", "Ter", "Qua", "Qui", "Sex"])
                            .candles([
                                [100.0, 112.0, 98.0, 110.0],
                                [110.0, 111.0, 101.0, 102.0],
                                [102.0, 108.0, 99.0, 107.0],
                                [107.0, 118.0, 105.0, 116.0],
                                [116.0, 117.0, 109.0, 111.0],
                            ])
                            .tight_scale()
                            .height(260.0),
                    ),
                )
                .spacing(16.0),
            ),
            _ => {
                let (told, chose) = (said.clone(), second_half.clone());
                let chosen = card_tab.get();
                let shown: AnyView = if chosen == 0 {
                    Box::new(
                        AreaChart::new([31.0, 38.0, 35.0, 44.0, 41.0, 49.0])
                            .height(120.0)
                            .tone(Semantic::Success),
                    )
                } else {
                    Box::new(
                        BarChart::new([("Seg", 180.0), ("Ter", 220.0), ("Qua", 160.0), ("Qui", 260.0)])
                            .height(120.0),
                    )
                };
                Box::new(
                    VStack::new((
                        Grid::new(
                            columns,
                            (
                                ChartCard::new(Keyed::new(chosen as u64, shown))
                                    .title("Loja central")
                                    .description(format!("Resultado do {period}"))
                                    .action(
                                        Button::new("Trocar período").style(ButtonStyle::Secondary).on_click(
                                            move || {
                                                chose.update(|h| *h = !*h);
                                                told.set(format!(
                                                    "Período: {}",
                                                    if chose.get() { "2º semestre" } else { "1º semestre" }
                                                ));
                                            },
                                        ),
                                    )
                                    .tabs(
                                        card_tab.binding(),
                                        [("Receita", "R$ 48 mil"), ("Pedidos", "1.240")],
                                    )
                                    .trend("Subiu 12% no mês", IconName::TrendingUp)
                                    .note("Valores em milhares de reais"),
                                ChartCard::new(
                                    Chart::new(ChartKind::Doughnut)
                                        .categories(["Novos", "Recorrentes"])
                                        .series("Clientes", [35.0, 65.0])
                                        .hole(0.6)
                                        .center_text("65%", "voltam")
                                        .no_appear()
                                        .height(200.0),
                                )
                                .title("Clientes")
                                .description("Quem volta a comprar")
                                .note("Últimos 30 dias")
                                .centered(),
                            ),
                        )
                        .spacing(16.0),
                        Text::new("Pequenos, para caber numa linha").style(TextStyle::Headline),
                        HStack::new((
                            Chart::new(ChartKind::Sparkline)
                                .series("Visitas", [3.0, 5.0, 4.0, 7.0, 6.0, 9.0, 8.0])
                                .alt("Visitas da semana, subindo")
                                .size(160.0, 40.0),
                            Chart::new(ChartKind::SparkArea)
                                .series("Vendas", [9.0, 7.0, 8.0, 5.0, 6.0, 4.0, 5.0])
                                .alt("Vendas da semana, caindo")
                                .size(160.0, 40.0),
                        ))
                        .spacing(16.0)
                        .wrap(),
                    ))
                    .spacing(16.0)
                    .align(Align::Stretch),
                )
            }
        };
        let picker: AnyView = if narrow {
            Box::new(Select::new(tab.binding(), USES).label("Tipo"))
        } else {
            Box::new(SegmentedControl::new(tab.binding(), USES))
        };
        let shown = ["Receita", "Pedidos"][card_tab.get().min(1)];
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                picker,
                Keyed::new(tab.get() as u64, body),
                Text::new(said.get()),
                Text::new(format!("O cartão mostra: {shown}")),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
