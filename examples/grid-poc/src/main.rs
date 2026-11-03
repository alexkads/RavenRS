//! A proof of concept of the data grid: `cargo run -p grid-poc --release`.
//!
//! The rows are made up, and so is the server, which counts what it is asked: the
//! numbers on the screen are the point of it.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs::prelude::*;

const LOCAL_ROWS: usize = 100_000;
const SERVER_ROWS: usize = 5_000_000;
const REGIONS: [&str; 5] = ["Norte", "Nordeste", "Centro-Oeste", "Sudeste", "Sul"];
const PLANS: [&str; 4] = ["Grátis", "Básico", "Pro", "Empresa"];
const OWNERS: [&str; 6] = ["Ana", "Bruno", "Carla", "Diego", "Elisa", "Fábio"];

/// The same made-up row every time, for a row number.
fn row(n: usize) -> Vec<Value> {
    vec![
        Value::Number(n as f64),
        Value::Text(format!("Cliente {n:07}")),
        Value::Text(REGIONS[n * 7 % REGIONS.len()].to_owned()),
        Value::Text(PLANS[n * 13 % PLANS.len()].to_owned()),
        Value::Text(OWNERS[n * 3 % OWNERS.len()].to_owned()),
        Value::Number((n * 37 % 500) as f64 + 1.0),
        Value::Number(((n * 97 % 90_000) as f64 + 1_000.0) / 10.0),
    ]
}

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::number("Id").width(90.0).groupable(false),
        GridColumn::text("Cliente").width(190.0).fill(),
        GridColumn::text("Região").width(140.0),
        GridColumn::text("Plano").width(120.0),
        GridColumn::text("Responsável").width(140.0),
        GridColumn::number("Licenças").width(110.0).summary(Summary::Sum),
        GridColumn::currency("Mensalidade", "R$ ", 2).width(150.0).summary(Summary::Sum),
    ]
}

/// What the made-up server has been asked.
#[derive(Clone, Default)]
struct Asked {
    questions: Rc<Cell<usize>>,
    rows_sent: Rc<Cell<usize>>,
}

fn server(asked: &Asked, with_total: bool) -> RemoteRows {
    let asked = asked.clone();
    RemoteRows::new(move |request, answer| {
        asked.questions.set(asked.questions.get() + 1);
        let end = (request.offset + request.limit).min(SERVER_ROWS);
        asked.rows_sent.set(asked.rows_sent.get() + (end - request.offset));
        let page = GridPage::new((request.offset..end).map(row).collect(), end < SERVER_ROWS);
        answer.send(if with_total { page.total(SERVER_ROWS) } else { page });
    })
}

struct Poc {
    data: GridData,
    view: State<usize>,
    scenarios: State<Vec<GridScenario>>,
    asked: Asked,
    infinite: RemoteRows,
    paged: RemoteRows,
}

const CHUNK: usize = 100;
const KEPT_CHUNKS: usize = 8;

impl Poc {
    fn new() -> Self {
        let asked = Asked::default();
        Self {
            data: GridData::new((0..LOCAL_ROWS).map(row).collect()),
            view: State::new(0),
            scenarios: State::new(Vec::new()),
            infinite: server(&asked, false).chunk_size(CHUNK).max_chunks(KEPT_CHUNKS),
            paged: server(&asked, true).chunk_size(CHUNK).max_chunks(KEPT_CHUNKS),
            asked,
        }
    }

    fn counters(&self, held: usize, kind: &str) -> impl View + use<> {
        let line = format!(
            "{kind} · o servidor recebeu {} pedidos e mandou {} linhas · no máximo {} de {} linhas ficaram em memória",
            self.asked.questions.get(),
            self.asked.rows_sent.get(),
            held,
            SERVER_ROWS,
        );
        Text::new(line).style(TextStyle::Callout).foreground(Semantic::InkMuted)
    }
}

impl View for Poc {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let view = self.view.get();
        let body: AnyView = match view {
            0 => Box::new(
                DataGrid::new(self.data.clone(), columns())
                    .group_panel(true)
                    .selection_mode(SelectionMode::Multiple)
                    .scenarios(self.scenarios.binding())
                    .group_by(2)
                    .on_export(|_| {}),
            ),
            1 => Box::new(
                DataGrid::new(self.data.clone(), columns())
                    .group_panel(true)
                    .scenarios(self.scenarios.binding())
                    .page_size(50)
                    .on_export(|_| {}),
            ),
            2 => Box::new(DataGrid::remote(self.infinite.clone(), columns())),
            _ => Box::new(DataGrid::remote(self.paged.clone(), columns()).page_size(50)),
        };
        let hint: AnyView = match view {
            0 => Box::new(
                Text::new(
                    "Arraste um cabeçalho para o espaço acima para agrupar; arraste os grupos para trocar a ordem, ou para fora para desagrupar. Botão direito em um cabeçalho abre o menu. Clique em um total no rodapé para trocar a soma ou escrever uma fórmula, como Sum([Mensalidade]) / Sum([Licenças]). Em Cenário, grave o arranjo com as fórmulas.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            ),
            1 => Box::new(
                Text::new("As mesmas 100 mil linhas em páginas: escolha quantas linhas por página no rodapé.")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ),
            2 => Box::new(self.counters(self.infinite.peak_rows(), "Rolagem infinita, sem total: nunca há COUNT(*)")),
            _ => Box::new(self.counters(self.paged.peak_rows(), "Páginas: só a página aberta é pedida")),
        };
        cx.child(
            &VStack::new((
                Text::new("Grade de dados").style(TextStyle::Title).font_weight(Weight::SEMIBOLD),
                SegmentedControl::new(
                    self.view.binding(),
                    [
                        "100 mil linhas: agrupar",
                        "Em páginas",
                        "Servidor: rolagem infinita",
                        "Servidor: páginas",
                    ],
                )
                .label("Demonstração"),
                hint,
                Keyed::new(view as u64, body).grow(1.0),
            ))
            .spacing(14.0)
            .padding(24.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

fn main() -> Result<(), ravenrs::native::PlatformError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("RavenRS: grade de dados").size(1360.0, 860.0);
    let app = Localized::new(Labels::pt_br(), Poc::new());
    ravenrs::native::run(app, options)
}
