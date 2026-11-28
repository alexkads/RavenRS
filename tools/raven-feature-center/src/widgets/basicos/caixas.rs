//! Checkbox: on and off, mixed as a "select all" computed from its children, an answer that may
//! be unknown, labels lined up in a column, and the states that keep it from changing.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "CheckboxDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Caixas de seleção";

const EXTRAS: [&str; 3] = ["Seguro", "Bagagem extra", "Assento na janela"];

/// Every public feature of Checkbox on one page.
#[derive(Debug, Default)]
pub struct CheckboxesPage;

impl View for CheckboxesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let terms = cx.use_state(|| false);
        let extras = cx.use_state(|| vec![true, false, true]);
        let car = cx.use_state(|| None::<bool>);
        let kept = cx.use_state(|| true);
        let chosen = extras.get();
        let (some, every) = (chosen.iter().any(|on| *on), chosen.iter().all(|on| *on));
        // "Select all" is on when every child is; turning it on or off sets them all.
        let everything = extras.clone();
        let select_all = Binding::new(move || every, move |on: bool| everything.set(vec![on; EXTRAS.len()]));
        let children: Vec<AnyView> = EXTRAS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let extras = extras.clone();
                let on = chosen[i];
                let one = Binding::new(move || on, move |value: bool| extras.update(|all| all[i] = value));
                Box::new(Checkbox::new(*name, one).leading(40.0)) as AnyView
            })
            .collect();
        let answer = match car.get() {
            None => "não informado",
            Some(true) => "sim",
            Some(false) => "não",
        };
        let picked: Vec<&str> = EXTRAS.iter().zip(&chosen).filter(|(_, on)| **on).map(|(n, _)| *n).collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Ligada, desligada ou mista. Mista é uma seleção parcial calculada dos filhos, ou uma resposta ainda não dada; as duas são anunciadas como \"parcialmente marcada\".")
                    .foreground(Semantic::InkMuted),
                VStack::new((
Text::new("Simples").style(TextStyle::Headline),
                Checkbox::new("Aceito os termos", terms.binding()),
                Text::new(format!("Termos: {}", if terms.get() { "aceitos" } else { "não aceitos" })),
))
.spacing(12.0)
.align(Align::Start),
                VStack::new((
Text::new("Selecionar todos").style(TextStyle::Headline),
                Text::new("Com parte dos filhos marcada, a caixa fica mista. Clique ou Espaço marca todos; de novo, desmarca todos.")
                    .foreground(Semantic::InkMuted),
                Checkbox::new("Todos os adicionais", select_all).mixed(some && !every),
                VStack::new(children).spacing(8.0).align(Align::Start),
                Text::new(format!("Adicionais: {}", if picked.is_empty() { "nenhum".to_owned() } else { picked.join(", ") })),
))
.spacing(12.0)
.align(Align::Start),
                VStack::new((
Text::new("Resposta opcional").style(TextStyle::Headline),
                Text::new("Para um sim ou não que pode ficar sem resposta: cada clique ou Espaço vai de não informado para sim, para não, e volta.")
                    .foreground(Semantic::InkMuted),
                Checkbox::optional("Tem carro", car.binding()),
                Text::new(format!("Tem carro: {answer}")),
))
.spacing(12.0)
.align(Align::Start),
                VStack::new((
Text::new("Estados").style(TextStyle::Headline),
                Checkbox::new("Indisponível", kept.binding()).disabled(true),
                Checkbox::new("Só leitura", kept.binding()).read_only(true),
))
.spacing(12.0)
.align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(720.0)
            .align(Align::Start),
        )
    }
}
