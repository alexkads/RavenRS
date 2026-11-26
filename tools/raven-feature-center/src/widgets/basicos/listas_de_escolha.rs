//! Select: one option of a list that opens under the field, by position or by value, with the name
//! screen readers announce, four sizes, a search box, a fixed width or the width of the column, an
//! icon before the value, turned off and read only.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "SelectDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Listas de escolha";

const ESTUDIOS: [&str; 3] = ["Todos os estúdios", "Estúdio", "Externo"];
const MOEDAS: [&str; 6] = ["Real", "Euro", "Dólar", "Libra", "Iene", "Peso"];
const PRAZOS: [&str; 3] = ["Hoje", "Esta semana", "Este mês"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Qualidade {
    Rascunho,
    Final,
}

impl Qualidade {
    fn nome(self) -> &'static str {
        match self {
            Self::Rascunho => "Rascunho",
            Self::Final => "Final",
        }
    }
}

/// Every public feature of Select on one page.
#[derive(Debug, Default)]
pub struct SelectsPage;

impl View for SelectsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let estudio = cx.use_state(|| 0_usize);
        let qualidade = cx.use_state(|| Qualidade::Final);
        let prazo = cx.use_state(|| 0_usize);
        let moeda = cx.use_state(|| 0_usize);
        let tamanho = cx.use_state(|| 0_usize);
        let travado = cx.use_state(|| 1_usize);
        let (estudio_now, qualidade_now, moeda_now) = (estudio.get(), qualidade.get(), moeda.get());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma opção de uma lista que abre sob o campo, pela posição ou pelo valor.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Pela posição e pelo valor").style(TextStyle::Headline),
                    HStack::new((
                        Select::new(estudio.binding(), ESTUDIOS).label("Estúdio").icon(IconName::MapPin),
                        Select::of(
                            qualidade.binding(),
                            [(Qualidade::Rascunho, "Rascunho"), (Qualidade::Final, "Final")],
                        )
                        .label("Qualidade"),
                    ))
                    .spacing(16.0),
                    Text::new(format!("Estúdio escolhido: {}", ESTUDIOS[estudio_now])),
                    Text::new(format!("Qualidade escolhida: {}", qualidade_now.nome())),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Ícone antes do valor").style(TextStyle::Headline),
                    HStack::new((
                        Select::new(prazo.binding(), PRAZOS).label("Prazo").icon(IconName::Calendar),
                        Select::new(prazo.binding(), ["Ana Lima", "Bruno Reis"])
                            .label("Responsável")
                            .icon(IconName::User),
                        Select::new(prazo.binding(), ["Português", "English"])
                            .label("Idioma")
                            .icon(IconName::Globe),
                    ))
                    .spacing(16.0),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Tamanhos").style(TextStyle::Headline),
                    HStack::new((
                        Select::new(tamanho.binding(), ["Mini"])
                            .label("Tamanho Mini")
                            .size(ButtonSize::Mini)
                            .width(120.0),
                        Select::new(tamanho.binding(), ["Small"])
                            .label("Tamanho Small")
                            .size(ButtonSize::Small)
                            .width(120.0),
                        Select::new(tamanho.binding(), ["Medium"]).label("Tamanho Medium").width(120.0),
                        Select::new(tamanho.binding(), ["Large"])
                            .label("Tamanho Large")
                            .size(ButtonSize::Large)
                            .width(120.0),
                    ))
                    .spacing(16.0)
                    .align(Align::Center),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Busca").style(TextStyle::Headline),
                    Select::new(moeda.binding(), MOEDAS).label("Moeda").searchable(),
                    Text::new(format!("Moeda escolhida: {}", MOEDAS[moeda_now])),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Largura").style(TextStyle::Headline),
                    Select::new(tamanho.binding(), ["Curta"]).label("Largura fixa").width(140.0),
                    Select::new(tamanho.binding(), ["Toda a largura da coluna"])
                        .label("Largura da coluna")
                        .fill(),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
                VStack::new((
                    Text::new("Desligada e só leitura").style(TextStyle::Headline),
                    HStack::new((
                        Select::new(travado.binding(), ["Aberto", "Fechado"])
                            .label("Desligada")
                            .disabled(true),
                        Select::new(travado.binding(), ["Aberto", "Fechado"])
                            .label("Só leitura")
                            .read_only(true),
                    ))
                    .spacing(16.0),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(960.0)
            .align(Align::Stretch),
        )
    }
}
