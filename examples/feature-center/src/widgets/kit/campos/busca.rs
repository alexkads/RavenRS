//! Search, tags, models and a scanner, every feature (#579): a city picked from memory and a
//! customer asked of a made-up server, tags chosen or made up to a limit, local models downloaded,
//! tried, used and removed, and codes read by a scanner. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "SearchFieldsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Busca e etiquetas";

/// The uses, in the order of the tabs.
pub const USES: [&str; 4] = ["Busca", "Etiquetas", "Modelos", "Leitor"];

/// How many customers the made-up server holds.
pub const CUSTOMERS: usize = 500;
/// The most customers it hands over at a time.
pub const LIMIT: usize = 8;
/// The most tags an order takes.
pub const MOST_TAGS: usize = 4;

fn cities() -> Vec<LookupItem> {
    [
        "São Paulo",
        "Rio de Janeiro",
        "Belo Horizonte",
        "Curitiba",
        "Porto Alegre",
        "Recife",
        "Salvador",
        "Manaus",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, city)| LookupItem::new(i.to_string(), city))
    .collect()
}

fn tags() -> Vec<LookupItem> {
    [
        ("urgente", "Urgente"),
        ("revisar", "Revisar"),
        ("pago", "Pago"),
        ("presente", "Presente"),
        ("fragil", "Frágil"),
    ]
    .into_iter()
    .map(|(key, label)| LookupItem::new(key, label))
    .collect()
}

/// The models offered, as they start.
pub fn models() -> Vec<ModelChoice> {
    let model = |name: &str, details: &str, status| ModelChoice {
        name: name.to_owned(),
        details: details.to_owned(),
        status,
        in_use: false,
        result: None,
        error: None,
    };
    vec![
        model("Pequeno", "1,1 GB · Apache 2.0 · roda em qualquer máquina", ModelStatus::Available),
        ModelChoice { in_use: true, ..model("Médio", "4,2 GB · MIT", ModelStatus::Installed) },
        model("Grande", "9,8 GB · precisa de 16 GB de memória", ModelStatus::Downloading(0.35)),
    ]
}

fn pick(cx: &BuildCx<'_>, tab: &State<usize>) -> AnyView {
    if cx.viewport().width < 720.0 {
        Box::new(Select::new(tab.binding(), USES).label("Uso"))
    } else {
        Box::new(SegmentedControl::new(tab.binding(), USES))
    }
}

/// The page: a tab per use, and a line with the last event.
#[derive(Debug, Default)]
pub struct SearchPage;

impl View for SearchPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let width = if narrow { 260.0 } else { 320.0 };
        let tab = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Procure uma cidade ou um cliente.".to_string());
        let city = cx.use_state(|| None::<LookupItem>);
        let customer = cx.use_state(|| None::<LookupItem>);
        let chosen = cx.use_state(|| vec!["urgente".to_string()]);
        let choices = cx.use_state(models);

        let body: AnyView = match tab.get() {
            0 => {
                let asked = said.clone();
                let server = LookupField::remote(customer.binding(), move |request, responder| {
                    let query = request.query.to_lowercase();
                    let found: Vec<LookupItem> = (1..=CUSTOMERS)
                        .map(|n| LookupItem::new(n.to_string(), format!("Cliente {n:03}")))
                        .filter(|item| item.label.to_lowercase().contains(&query))
                        .take(request.limit)
                        .collect();
                    asked.set_if_changed(format!(
                        "O servidor recebeu \"{}\" e mandou {} de até {}",
                        request.query,
                        found.len(),
                        request.limit
                    ));
                    responder.respond(found);
                })
                .limit(LIMIT)
                .label("Cliente")
                .placeholder("Procure entre 500 clientes")
                .width(width);
                let picked = |item: Option<LookupItem>| item.map_or("nenhum".to_string(), |i| i.label);
                Box::new(
                    VStack::new((
                        LookupField::new(city.binding(), cities())
                            .label("Cidade")
                            .placeholder("Procure uma cidade")
                            .width(width),
                        server,
                        LookupField::new(Binding::constant(cities().into_iter().next()), cities())
                            .label("Sede, só para ler")
                            .read_only(true),
                        LookupField::new(Binding::constant(None), cities())
                            .label("Filial, desligada")
                            .disabled(true),
                        Text::new(format!(
                            "Cidade: {} · cliente: {}",
                            picked(city.get()),
                            picked(customer.get())
                        )),
                    ))
                    .spacing(8.0)
                    .align(Align::Start),
                )
            }
            1 => Box::new(
                VStack::new((
                    TagBox::new(chosen.binding(), tags())
                        .label("Etiquetas do pedido")
                        .placeholder("Escolha ou escreva uma etiqueta")
                        .allow_new(true)
                        .most(MOST_TAGS)
                        .width(width),
                    FormField::new(
                        "Etiquetas do cliente",
                        TagBox::new(Binding::constant(vec!["pago".to_string()]), tags()).fill(),
                    ),
                    TagBox::new(
                        Binding::constant(vec!["presente".to_string(), "fragil".to_string()]),
                        tags(),
                    )
                    .label("Etiquetas da nota, só para ler")
                    .read_only(true),
                    TagBox::new(Binding::constant(Vec::new()), tags())
                        .label("Etiquetas, desligadas")
                        .disabled(true),
                    Text::new(format!(
                        "{} de {MOST_TAGS} etiquetas: {}",
                        chosen.with(Vec::len),
                        chosen.get().join(", ")
                    )),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ),
            2 => {
                let event = |verb: &'static str, change: fn(&mut ModelChoice)| {
                    let (choices, said) = (choices.clone(), said.clone());
                    move |index: usize| {
                        choices.update(|all| {
                            if let Some(choice) = all.get_mut(index) {
                                change(choice);
                                said.set(format!("{verb}: {}", choice.name));
                            }
                        })
                    }
                };
                Box::new(
                    ModelPicker::new(choices.get())
                        .labels(ModelPickerLabels::pt_br())
                        .on_download(event("Baixando", |c| c.status = ModelStatus::Downloading(0.1)))
                        .on_cancel(event("Download parado", |c| c.status = ModelStatus::Available))
                        .on_test(event("Testado", |c| {
                            c.result = Some("Respondeu em 0,8 s".to_string());
                        }))
                        .on_use(event("Em uso agora", |c| c.in_use = !c.in_use))
                        .on_remove(event("Removido", |c| {
                            c.status = ModelStatus::Available;
                            c.in_use = false;
                        })),
                )
            }
            _ => {
                let read = said.clone();
                Box::new(
                    VStack::new((
                        ScanField::new(move |scan: Scan| {
                            let by = if scan.from_reader { "pelo leitor" } else { "pelo teclado" };
                            read.set(format!("Lido {by}: {}", scan.text));
                        })
                        .placeholder("Passe o código de barras ou digite e tecle Enter"),
                        ScanField::new(|_| {}).placeholder("Leitor do caixa 2, desligado").disabled(true),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
        };
        cx.child(
            &VStack::new((pick(cx, &tab), Keyed::new(tab.get() as u64, body), Text::new(said.get())))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
