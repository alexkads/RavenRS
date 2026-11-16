//! `DataGrid`, every feature (#598): sales to select, group, filter, edit and act on row by row;
//! accounts that open their entries below the row and export what shows; a grid dressed its own
//! way next to one with no data; and rows that stay on a server. A line says what happened last.

use std::collections::BTreeSet;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DataGridDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Grade de dados";

/// The uses, in the order of the tabs.
pub const USES: [&str; 4] = ["Vendas", "Detalhes e arquivos", "Aparência", "Servidor"];

/// How many customers the sales start with.
pub const CUSTOMERS: usize = 48;
/// How many rows the made-up server holds.
pub const SERVER_ROWS: usize = 5_000_000;

const REGIONS: [&str; 5] = ["Norte", "Nordeste", "Centro-Oeste", "Sudeste", "Sul"];
const PLANS: [&str; 4] = ["Grátis", "Básico", "Pro", "Empresa"];
const SELLERS: [&str; 4] = ["Ana", "Bia", "Caio", "Davi"];
/// The column of the licenses, which cannot go below zero.
const LICENSES: usize = 5;
/// The column that says whether the customer is active.
const ACTIVE: usize = 7;

/// A customer, the same for the same number.
pub fn customer(n: usize) -> Vec<Value> {
    vec![
        Value::Number(n as f64),
        Value::Text(format!("Cliente {n:03}")),
        Value::Text(REGIONS[n * 7 % REGIONS.len()].to_owned()),
        Value::Text(PLANS[n * 3 % PLANS.len()].to_owned()),
        Value::Text(SELLERS[n % SELLERS.len()].to_owned()),
        Value::Number((n * 37 % 50) as f64 + 1.0),
        Value::Number(((n * 97 % 9_000) as f64 + 1_000.0) / 10.0),
        Value::Bool(!n.is_multiple_of(5)),
    ]
}

fn sales_columns() -> Vec<GridColumn> {
    vec![
        GridColumn::number("Id").width(70.0).groupable(false),
        GridColumn::text("Cliente").width(150.0).fill(),
        GridColumn::text("Região").width(120.0),
        GridColumn::text("Plano").width(110.0).template(|v| -> AnyView {
            let text = v.key_text();
            let tone = match text.as_str() {
                "Empresa" => Tone::Accent,
                "Pro" => Tone::Success,
                "Básico" => Tone::Warning,
                _ => Tone::Neutral,
            };
            Box::new(Badge::new(text).tone(tone))
        }),
        GridColumn::text("Vendedor").width(110.0),
        GridColumn::number("Licenças").width(100.0).summary(Summary::Sum).editable(),
        GridColumn::currency("Mensalidade", "R$ ", 2).width(140.0).summary(Summary::Sum),
        GridColumn::boolean("Ativo").width(80.0),
    ]
}

fn account_columns() -> Vec<GridColumn> {
    vec![
        GridColumn::text("Conta").fill(),
        GridColumn::currency("Saldo", "R$ ", 2).width(160.0).summary(Summary::Sum),
    ]
}

fn accounts() -> GridData {
    GridData::new(
        (1..=30).map(|i| vec![format!("Conta {i:02}").into(), (f64::from(i) * 37.5).into()]).collect(),
    )
}

fn entries(account: &[Value]) -> AnyView {
    Box::new(
        DataGrid::new(
            GridData::new(
                (1..=8)
                    .map(|i| vec![format!("Lançamento {i}").into(), (f64::from(i) * 2.5).into()])
                    .collect(),
            ),
            [
                GridColumn::text(account[0].key_text()).fill(),
                GridColumn::currency("Valor", "R$ ", 2).width(140.0),
            ],
        )
        .toolbar(false)
        .filter_row(false)
        .footer(false)
        .height(170.0),
    )
}

fn server_row(n: usize) -> Vec<Value> {
    vec![Value::Number(n as f64), Value::Text(format!("Pedido {n:07}"))]
}

fn server_columns() -> Vec<GridColumn> {
    vec![GridColumn::number("Id").width(120.0), GridColumn::text("Pedido").fill()]
}

/// The page: a tab per use, and a line with what happened last.
#[derive(Debug, Default)]
pub struct DataGridPage;

impl View for DataGridPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let said =
            cx.use_state(|| "Selecione, agrupe, filtre ou edite uma célula com duplo clique.".to_string());
        let sales = cx.use_state(|| GridData::new((1..=CUSTOMERS).map(customer).collect()));
        let next = cx.use_state(|| CUSTOMERS + 1);
        let selection = cx.use_state(BTreeSet::<usize>::new);
        let keys = cx.use_state(BTreeSet::<String>::new);
        let layout = cx.use_state(GridLayout::default);
        let scenarios = cx.use_state(Vec::<GridScenario>::new);
        let expanded = cx.use_state(BTreeSet::<usize>::new);
        let current = cx.use_state(GridCurrent::new);
        let asked = cx.use_state(|| 0_usize);
        let sent = cx.use_state(|| 0_usize);
        // The same server twice: scrolling with no total ever counted, and pages that know it.
        let server = cx.use_state(|| made_up_server(&asked, &sent, false).chunk_size(100).max_chunks(8));
        let paged = cx.use_state(|| made_up_server(&asked, &sent, true));
        let narrow = cx.viewport().width < 720.0;
        let height = if narrow { 420.0 } else { 520.0 };

        let body: AnyView = match tab.get() {
            0 => {
                let data = sales.get();
                let (adding, add_line, counter) = (data.clone(), said.clone(), next.clone());
                let (deleting, delete_line) = (data.clone(), said.clone());
                let (edit_line, activate_line, open_line) = (said.clone(), said.clone(), said.clone());
                let (call_line, block_line) = (said.clone(), said.clone());
                let picked = keys.get();
                Box::new(
                    VStack::new((
                        Text::new(if picked.is_empty() {
                            "Nenhum cliente selecionado".to_string()
                        } else {
                            format!("Selecionados: {}", picked.iter().cloned().collect::<Vec<_>>().join(", "))
                        })
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                        DataGrid::new(data, sales_columns())
                            .height(height)
                            .selection_mode(SelectionMode::Multiple)
                            .selection(selection.binding())
                            .key_column(0)
                            .selected_keys(keys.binding())
                            .name_column(1)
                            .sort_by(6, SortDirection::Descending)
                            .group_by(2)
                            .group_panel(true)
                            .filter_row(true)
                            .filter_builder(true)
                            .search_panel(true)
                            .toolbar(true)
                            .footer(true)
                            .layout(layout.binding())
                            .scenarios(scenarios.binding())
                            .cell_style(|row, column| match row.get(ACTIVE) {
                                Some(Value::Bool(false)) => {
                                    GridCellStyle::new(Some(Semantic::InkMuted), None, false)
                                }
                                _ if column == 6 && row[6].as_f64().is_some_and(|v| v > 800.0) => {
                                    GridCellStyle::new(Some(Semantic::Accent), None, true)
                                }
                                _ => GridCellStyle::default(),
                            })
                            .on_cell_edit(move |edit| match (edit.column, edit.value.as_f64()) {
                                (LICENSES, Some(v)) if v < 0.0 => {
                                    let why = "Licenças não podem ficar abaixo de zero".to_string();
                                    edit_line.set(format!("Recusado: {why}"));
                                    Err(why)
                                }
                                _ => {
                                    edit_line.set(format!("Linha {} mudou para {}", edit.row + 1, edit.value.key_text()));
                                    Ok(())
                                }
                            })
                            .row_action(
                                GridRowAction::new("Ligar para", IconName::Phone, move |row| {
                                    call_line.set(format!("Ligando para {}", row[1].key_text()))
                                })
                                .visible(|row| matches!(row.get(ACTIVE), Some(Value::Bool(true)))),
                            )
                            .row_action(
                                GridRowAction::new("Bloquear", IconName::Lock, move |row| {
                                    block_line.set(format!("{} bloqueado", row[1].key_text()))
                                })
                                .enabled(|row| matches!(row.get(ACTIVE), Some(Value::Bool(true)))),
                            )
                            .on_add_row(move || {
                                let n = counter.get();
                                counter.set(n + 1);
                                adding.push(customer(n));
                                add_line.set(format!("Cliente {n:03} adicionado: {} clientes", adding.len()));
                            })
                            .on_delete_rows(move |mut rows| {
                                rows.sort_unstable_by(|a, b| b.cmp(a));
                                for row in &rows {
                                    deleting.remove(*row);
                                }
                                delete_line.set(format!("{} excluídos: {} clientes", rows.len(), deleting.len()));
                            })
                            .on_row_activate(move |row| activate_line.set(format!("Linha {} ativada", row + 1)))
                            .on_row_open(move |row| open_line.set(format!("Abrindo {}", row[1].key_text()))),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
            1 => {
                let (open, export_line, file_line, count_line) = (expanded.clone(), said.clone(), said.clone(), said.clone());
                let showing = current.get();
                let counted = showing.clone();
                Box::new(
                    VStack::new((
                        HStack::new((
                            Button::new("Abrir a primeira conta").style(ButtonStyle::Secondary).on_click(move || {
                                open.update(|rows| {
                                    rows.insert(0);
                                })
                            }),
                            Button::new("Contar o que a grade mostra").style(ButtonStyle::Secondary).on_click(
                                move || match counted.table("Contas", "pt-BR") {
                                    Some(table) => count_line.set(format!("A grade mostra {} contas", table.rows.len())),
                                    None => count_line.set("A grade ainda não foi montada".to_string()),
                                },
                            ),
                        ))
                        .spacing(8.0)
                        .wrap(),
                        DataGrid::new(accounts(), account_columns())
                            .height(height)
                            .row_detail(|row, _| entries(row))
                            .detail_height(194.0)
                            .expanded_rows(expanded.binding())
                            .current(showing)
                            .export_title("Contas")
                            .on_export(move |csv| {
                                let lines = csv.lines().count().saturating_sub(1);
                                export_line.set(format!("CSV com {lines} contas"))
                            })
                            .on_export_file(move |format, table| {
                                file_line.set(format!("{format:?}: \"{}\" com {} contas", table.title, table.rows.len()))
                            }),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
            2 => Box::new(
                VStack::new((
                    DataGrid::new(accounts(), account_columns())
                        .height(if narrow { 260.0 } else { 320.0 })
                        .toolbar(false)
                        .row_height(28.0)
                        .striped(true)
                        .even_row(Color::hex(0x1E2A3A))
                        .odd_row(Color::hex(0x16202C))
                        .horizontal_lines(false)
                        .vertical_lines(true),
                    DataGrid::new(GridData::new(Vec::new()), account_columns())
                        .height(160.0)
                        .toolbar(false)
                        .empty_text("Nenhuma conta cadastrada ainda"),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
            ),
            _ => Box::new(
                VStack::new((
                    Text::new(format!(
                        "O servidor recebeu {} pedidos e mandou {} linhas; a grade guarda no máximo {} das {SERVER_ROWS}",
                        asked.get(),
                        sent.get(),
                        server.get().peak_rows().max(paged.get().peak_rows()),
                    ))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                    DataGrid::remote(server.get(), server_columns()).height(height / 2.0),
                    DataGrid::remote(paged.get(), server_columns()).page_size(25).height(height / 2.0 + 40.0),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            ),
        };
        let picker: AnyView = if narrow {
            Box::new(Select::new(tab.binding(), USES).label("Uso"))
        } else {
            Box::new(SegmentedControl::new(tab.binding(), USES))
        };
        cx.child(
            &VStack::new((picker, Keyed::new(tab.get() as u64, body), Text::new(said.get())))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}

/// A made-up server of [`SERVER_ROWS`] rows, which counts what it is asked and sends.
fn made_up_server(asked: &State<usize>, sent: &State<usize>, with_total: bool) -> RemoteRows {
    let (asked, sent) = (asked.clone(), sent.clone());
    RemoteRows::new(move |request, answer| {
        asked.update(|n| *n += 1);
        let end = (request.offset + request.limit).min(SERVER_ROWS);
        sent.update(|n| *n += end - request.offset);
        let page = GridPage::new((request.offset..end).map(server_row).collect(), end < SERVER_ROWS);
        answer.send(if with_total { page.total(SERVER_ROWS) } else { page });
    })
}
