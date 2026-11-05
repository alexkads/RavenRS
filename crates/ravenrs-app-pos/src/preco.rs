//! The screen of pricing: a product's cost and what the price has to cover, the markup that comes
//! out and, for a price typed in, what is left of it.

use ravenrs_app::{FieldValue, Message, MessageKind, Object, Query, Value};
use ravenrs_app_ui::Session;
use ravenrs_layout::Align;
use ravenrs_pos::{Composicao, Formacao, arredondar_preco, ler_valor, valor_em_texto};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Card, FormField, Grid, HStack, NodeId, ScrollView, Select, Spacer,
    Text, TextField, Tone, VStack, View, ViewExt,
};
use rust_decimal::Decimal;

use crate::caixa::{configuracao_decimal, texto};
use crate::modelo::PRODUTO;

/// Pricing.
pub(crate) struct TelaPreco {
    pub(crate) session: Session,
}

/// What was typed in each field, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Campos {
    produto: Option<String>,
    frete: String,
    outros: String,
    impostos: String,
    custos_fixos: String,
    comissao: String,
    margem: String,
    preco_digitado: String,
}

fn numero(texto: &str) -> Decimal {
    ler_valor(texto).unwrap_or_default()
}

fn campo(
    rotulo: &'static str,
    campos: &State<Campos>,
    get: fn(&Campos) -> &String,
    set: fn(&mut Campos, String),
) -> AnyView {
    let (read, write) = (campos.clone(), campos.clone());
    Box::new(FormField::new(
        rotulo,
        TextField::new(Binding::new(
            move || read.with(|all| get(all).clone()),
            move |value: String| write.update(|all| set(all, value)),
        ))
        .end_aligned(),
    ))
}

fn linha(rotulo: &str, valor: String, forte: bool) -> AnyView {
    let mut texto = Text::new(valor);
    if forte {
        texto = texto.font_weight(Weight::SEMIBOLD);
    }
    Box::new(
        HStack::new(vec![
            Box::new(Text::new(rotulo.to_owned()).foreground(Semantic::InkMuted)) as AnyView,
            Box::new(Spacer::new()),
            Box::new(texto),
        ])
        .align(Align::Center),
    )
}

fn composicao_de(custo: Decimal, campos: &Campos) -> Composicao {
    Composicao {
        custo,
        frete: numero(&campos.frete),
        outros_custos: numero(&campos.outros),
        impostos: numero(&campos.impostos),
        custos_fixos: numero(&campos.custos_fixos),
        comissao: numero(&campos.comissao),
        margem: numero(&campos.margem),
    }
}

fn quadro(titulo: &str, formacao: &Formacao) -> AnyView {
    Box::new(
        Card::new(
            VStack::new(vec![
                Box::new(Text::new(titulo.to_owned()).style(TextStyle::Headline)) as AnyView,
                Box::new(
                    Text::new(format!("R$ {}", valor_em_texto(formacao.preco)))
                        .font_size(44.0)
                        .font_weight(Weight::SEMIBOLD),
                ),
                linha("Markup sobre o custo", format!("{:.4}", formacao.markup).replace('.', ","), true),
                linha("Custo total", format!("R$ {}", valor_em_texto(formacao.custo_total)), false),
                linha("Impostos", format!("R$ {}", valor_em_texto(formacao.valor_impostos)), false),
                linha("Custos fixos", format!("R$ {}", valor_em_texto(formacao.valor_custos_fixos)), false),
                linha("Comissão e taxas", format!("R$ {}", valor_em_texto(formacao.valor_comissao)), false),
                linha(
                    "Lucro",
                    format!("R$ {} ({}%)", valor_em_texto(formacao.lucro), formacao.margem.normalize()),
                    true,
                ),
                linha(
                    "Ponto de equilíbrio",
                    format!("R$ {}", valor_em_texto(formacao.ponto_de_equilibrio)),
                    false,
                ),
            ])
            .spacing(8.0)
            .align(Align::Stretch),
        )
        .grow(1.0),
    )
}

impl View for TelaPreco {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let campos: State<Campos> = cx.use_state(|| Campos {
            frete: "0,00".to_owned(),
            outros: "0,00".to_owned(),
            impostos: configuracao_decimal(session, "impostos").normalize().to_string().replace('.', ","),
            custos_fixos: configuracao_decimal(session, "custos_fixos").normalize().to_string().replace('.', ","),
            comissao: configuracao_decimal(session, "comissao").normalize().to_string().replace('.', ","),
            margem: configuracao_decimal(session, "margem").normalize().to_string().replace('.', ","),
            ..Campos::default()
        });
        let space = session.frame().space();
        let mut produtos = space.borrow().query(PRODUTO, &Query::all()).unwrap_or_default();
        produtos.sort_by_key(|produto| texto(produto, "descricao"));
        let atual = campos.with(|all| all.produto.clone());
        let posicao = produtos.iter().position(|produto| Some(texto(produto, "codigo")) == atual).unwrap_or(0);
        let produto: Option<Object> = produtos.get(posicao).cloned();

        let (read, write) = (posicao, campos.clone());
        let codigos: Vec<String> = produtos.iter().map(|produto| texto(produto, "codigo")).collect();
        let seletor = Select::new(
            Binding::new(
                move || read,
                move |i: usize| {
                    let codigo = codigos.get(i).cloned();
                    write.update(|all| {
                        all.produto = codigo;
                        all.preco_digitado.clear();
                    });
                },
            ),
            produtos.iter().map(|produto| texto(produto, "descricao")).collect::<Vec<_>>(),
        )
        .label("Produto");
        let custo = produto.as_ref().map_or(Decimal::ZERO, |produto| Decimal::from_value(produto.get("custo")));
        let preco_atual = produto.as_ref().map_or(Decimal::ZERO, |produto| match produto.get("preco") {
            Value::Number(centavos) => Decimal::new(centavos.round() as i64, 2),
            _ => Decimal::ZERO,
        });
        let valores = campos.get();
        let composicao = composicao_de(custo, &valores);
        let esquerda: Vec<AnyView> = vec![
            Box::new(FormField::new("Produto", seletor)),
            linha("Custo médio", format!("R$ {}", valor_em_texto(custo)), true),
            linha("Preço atual", format!("R$ {}", valor_em_texto(preco_atual)), true),
            Box::new(Grid::new(
                2,
                vec![
                    campo("Frete por unidade (R$)", &campos, |c| &c.frete, |c, v| c.frete = v),
                    campo("Outros custos (R$)", &campos, |c| &c.outros, |c, v| c.outros = v),
                    campo("Impostos sobre a venda (%)", &campos, |c| &c.impostos, |c, v| c.impostos = v),
                    campo("Custos fixos (%)", &campos, |c| &c.custos_fixos, |c, v| c.custos_fixos = v),
                    campo("Comissão e taxas (%)", &campos, |c| &c.comissao, |c, v| c.comissao = v),
                    campo("Margem desejada (%)", &campos, |c| &c.margem, |c, v| c.margem = v),
                ],
            )),
            campo("Testar um preço (R$)", &campos, |c| &c.preco_digitado, |c, v| c.preco_digitado = v),
        ];

        let mut direita: Vec<AnyView> = Vec::new();
        let calculado = composicao.calcular();
        match &calculado {
            Ok(formacao) => {
                let final_de = configuracao_decimal(session, "final_de_preco").trunc();
                let centavos = u8::try_from(final_de.mantissa()).unwrap_or(90);
                let sugerido = arredondar_preco(formacao.preco, centavos);
                direita.push(quadro("Preço calculado", formacao));
                let gondola = composicao.para_o_preco(sugerido);
                direita.push(quadro("Com o final de gôndola", &gondola));
                let (session, codigo, campos_salvar) = (session.clone(), produto.as_ref().map(|p| p.get("id")), sugerido);
                direita.push(Box::new(
                    Button::new(format!("Aplicar R$ {} ao produto", valor_em_texto(campos_salvar)))
                        .style(ButtonStyle::Primary)
                        .full_width()
                        .disabled(codigo.is_none())
                        .on_click(move || {
                            let Some(id) = &codigo else { return };
                            aplicar_preco(&session, id, campos_salvar);
                        }),
                ));
            }
            Err(why) => direita.push(Box::new(Alert::new("Não dá para calcular", why.clone()).tone(Tone::Warning))),
        }
        if let Some(digitado) = ler_valor(&valores.preco_digitado).filter(|preco| *preco > Decimal::ZERO) {
            let formacao = composicao.para_o_preco(digitado);
            direita.push(quadro("No preço testado", &formacao));
            if formacao.lucro < Decimal::ZERO {
                direita.push(Box::new(
                    Alert::new("Prejuízo", "Esse preço não cobre os custos e os impostos.").tone(Tone::Danger),
                ));
            }
        }
        cx.child(
            &HStack::new(vec![
                Box::new(VStack::new(esquerda).spacing(12.0).align(Align::Stretch).width(460.0)) as AnyView,
                Box::new(
                    ScrollView::new(vec![
                        Box::new(VStack::new(direita).spacing(12.0).align(Align::Stretch)) as AnyView,
                    ])
                    .grow(1.0),
                ),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(16.0)
            .grow(1.0),
        )
    }
}

/// Keeps a new price on a product.
pub(crate) fn aplicar_preco(session: &Session, id: &Value, preco: Decimal) {
    let space = session.frame().space();
    let mut space = space.borrow_mut();
    let result = (|| {
        if let Some(mut produto) = space.find(PRODUTO, id)? {
            let centavos = (preco * Decimal::ONE_HUNDRED).round();
            produto.set("preco", Decimal::to_value(&centavos));
            space.save(produto)?;
        }
        space.commit()
    })();
    if result.is_err() {
        space.rollback();
    }
    drop(space);
    session.records_changed();
    session.show_message(match result {
        Ok(()) => Message::new(format!("Preço de R$ {} aplicado.", valor_em_texto(preco)), MessageKind::Success),
        Err(error) => Message::new(error.to_string(), MessageKind::Error),
    });
}
