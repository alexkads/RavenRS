//! `ParameterForm`, every feature: a schema with text, a number, yes or no, a choice, several
//! choices kept as a list (#932), a field
//! shown only in one mode, a field that takes an expression, a named group, a list of rows,
//! options loaded by the app that depend on another field (loading, failed, ready), and
//! Aplicar and Descartar, each written on a line the tour reads.

use ravenrs::prelude::*;
use serde_json::{Value, json};

/// The control's name, as the session knows it.
pub const ID: &str = "ParameterFormDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Formulário de parâmetros";

/// The parameters of a request to another system, as a node of a flow would ask them.
fn schema() -> Vec<ParamField> {
    vec![
        ParamField::new("url", "Endereço", ParamKind::Text).required().hint("Para onde vai o pedido"),
        ParamField::new(
            "method",
            "Método",
            ParamKind::Choice(vec![
                ("GET".into(), "Buscar (GET)".into()),
                ("POST".into(), "Enviar (POST)".into()),
            ]),
        )
        .default("GET"),
        ParamField::new("body", "Corpo", ParamKind::LongText).shown_when("method", "POST"),
        ParamField::new(
            "accept",
            "Aceita",
            ParamKind::MultiChoice(vec![
                ("json".into(), "JSON".into()),
                ("xml".into(), "XML".into()),
                ("text".into(), "Texto".into()),
            ]),
        )
        .default(json!(["json"])),
        ParamField::new("only_when", "Só quando", ParamKind::Text).expressions(),
        ParamField::new("timeout", "Tempo limite (s)", ParamKind::Number).default(30),
        ParamField::new("follow", "Seguir redirecionamentos", ParamKind::Boolean).default(true),
        ParamField::new(
            "auth",
            "Autenticação",
            ParamKind::Group(vec![
                ParamField::new("user", "Usuário", ParamKind::Text),
                ParamField::new("account", "Conta", ParamKind::Loaded).depends_on(["user"]),
            ]),
        ),
        ParamField::new(
            "headers",
            "Cabeçalhos",
            ParamKind::List(vec![
                ParamField::new("name", "Nome", ParamKind::Text),
                ParamField::new("value", "Valor", ParamKind::Text),
            ]),
        ),
    ]
}

/// The page.
#[derive(Debug, Default)]
pub struct ParametersPage;

impl View for ParametersPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let values = cx.use_state(ParamValues::new);
        // How the accounts of the user arrive, as a connector would answer.
        let answer = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let options = match answer.get() {
            0 => ParamOptions::Ready(vec![
                ("pessoal".into(), "Pessoal".into()),
                ("empresa".into(), "Empresa".into()),
            ]),
            1 => ParamOptions::Loading,
            _ => ParamOptions::Failed("O serviço não respondeu.".into()),
        };
        let (loaded, applied, discarded) = (said.clone(), said.clone(), said.clone());
        let form = ParameterForm::new(schema(), values.binding())
            .options("account", options)
            .on_load(move |field, now| {
                loaded.set(format!(
                    "Pediu as opções de {field}, para {}",
                    now.get("auth").map_or(json!(null), Clone::clone)
                ));
            })
            .on_apply(move |values| applied.set(format!("Aplicado: {}", Value::Object(values.0.clone()))))
            .on_discard(move || discarded.set("Descartado.".to_string()));
        // The same form inside a dialog's flow: each change goes back at once, with no buttons.
        let live_values = cx.use_state(ParamValues::new);
        let shown_live = live_values.get();
        let live_form = ParameterForm::new(
            vec![
                ParamField::new("branch", "Branch do disparo", ParamKind::Text).required(),
                ParamField::new("e2e", "Rodar os testes de ponta a ponta", ParamKind::Boolean).default(false),
            ],
            live_values.binding(),
        )
        .live(true);
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Ao vivo, dentro de um diálogo").style(TextStyle::Headline),
                Text::new(
                    "Sem Aplicar nem Descartar: cada mudança já vale, e quem decide é o botão do diálogo.",
                )
                .foreground(Semantic::InkMuted),
                VStack::new((live_form,)).width(560.0),
                Text::new(format!("Ao vivo: {}", Value::Object(shown_live.0.clone()))),
                Text::new("As opções da conta, como o serviço responde").style(TextStyle::Headline),
                SegmentedControl::new(answer.binding(), ["Chegaram", "Carregando", "Falharam"]),
                VStack::new((form,)).width(560.0),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .align(Align::Start),
        )
    }
}
