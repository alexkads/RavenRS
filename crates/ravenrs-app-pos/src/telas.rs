//! The screens of the checkout: they show the state in [`Caixa`] and call it.

use ravenrs_app_ui::Session;
use ravenrs_layout::{Align, Justify};
use ravenrs_nfe::sandbox::{autorizacao, cenarios};
use ravenrs_pos::{Forma, valor_em_texto};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Card, Column, FormField, HStack, Hotkey,
    NodeId, ScanField, ScrollView, Spacer, Spinner, Table, Text, TextField, Tone, VStack, View, ViewExt,
};

use crate::caixa::{Caixa, Tela, configuracao};
use crate::modulo::{VIEW_CAIXA, VIEW_ENTRADA, VIEW_ETIQUETAS, VIEW_PRECO, abrir_outras_funcoes};

fn titulo(text: impl Into<String>) -> Text {
    Text::new(text).style(TextStyle::Caption).foreground(Semantic::InkMuted)
}

fn quantidade_em_texto(quantidade: rust_decimal::Decimal) -> String {
    format!("{:.3}", quantidade.round_dp(3)).replace('.', ",")
}

/// The checkout, on whichever of its screens is up.
pub(crate) struct TelaCaixa {
    pub(crate) caixa: Caixa,
    pub(crate) session: Session,
}

impl TelaCaixa {
    fn cabecalho(&self) -> AnyView {
        let carrinho = self.caixa.carrinho.get();
        let cliente = carrinho.cliente.clone().map_or_else(
            || "Consumidor não identificado".to_owned(),
            |cliente| format!("Consumidor {cliente}"),
        );
        let nota = if self.caixa.nota_grande.get() { "Nota grande (NF-e, A4)" } else { "NFC-e" };
        let operador = configuracao(&self.session, "operador");
        let numero = configuracao(&self.session, "caixa");
        Box::new(
            HStack::new(vec![
                Box::new(Text::new(format!("Caixa {numero}")).style(TextStyle::Headline)) as AnyView,
                Box::new(Text::new(format!("Operador: {operador}")).foreground(Semantic::InkMuted)),
                Box::new(Spacer::new()),
                Box::new(Text::new(nota).font_weight(Weight::SEMIBOLD)),
                Box::new(Text::new(cliente).foreground(Semantic::InkMuted)),
            ])
            .spacing(24.0)
            .align(Align::Center)
            .padding((16.0, 10.0))
            .background(Semantic::Surface),
        )
    }

    fn vendendo(&self) -> AnyView {
        let carrinho = self.caixa.carrinho.get();
        let mut tabela = Table::new([
            Column::new("Item").weight(0.4),
            Column::new("Código").weight(1.2),
            Column::new("Descrição").weight(3.0),
            Column::new("Qtd").weight(0.7).numeric(),
            Column::new("Unit.").weight(0.8).numeric(),
            Column::new("Total").weight(0.9).numeric(),
        ]);
        for (index, linha) in carrinho.linhas.iter().enumerate() {
            tabela = tabela.row([
                (index + 1).to_string(),
                linha.gtin.clone().unwrap_or_else(|| linha.codigo.clone()),
                linha.descricao.clone(),
                quantidade_em_texto(linha.quantidade),
                valor_em_texto(linha.preco),
                valor_em_texto(linha.total()),
            ]);
        }
        let lista =
            Card::new(ScrollView::new(vec![Box::new(tabela) as AnyView]).grow(1.0)).grow(1.0).stretch();

        let ultimo: AnyView = match carrinho.ultimo() {
            Some(linha) => Box::new(
                VStack::new(vec![
                    Box::new(titulo("ÚLTIMO ITEM")) as AnyView,
                    Box::new(Text::new(linha.descricao.clone()).style(TextStyle::Title)),
                    Box::new(
                        HStack::new(vec![
                            Box::new(
                                Text::new(format!(
                                    "{} x R$ {}",
                                    quantidade_em_texto(linha.quantidade),
                                    valor_em_texto(linha.preco)
                                ))
                                .foreground(Semantic::InkMuted),
                            ) as AnyView,
                            Box::new(Spacer::new()),
                            Box::new(
                                Text::new(format!("R$ {}", valor_em_texto(linha.total())))
                                    .style(TextStyle::Headline),
                            ),
                        ])
                        .align(Align::Center),
                    ),
                ])
                .spacing(8.0)
                .align(Align::Stretch),
            ),
            None => Box::new(titulo("PASSE O PRIMEIRO PRODUTO")),
        };
        let (caixa, session) = (self.caixa.clone(), self.session.clone());
        let leitura = ScanField::new(move |lido| caixa.ler(&session, &lido.text))
            .placeholder("Código de barras ou quantidade*código");
        let mut lado: Vec<AnyView> = vec![
            Box::new(Card::new(ultimo)),
            Box::new(total_escuro("TOTAL DA VENDA", carrinho.total(), Some(carrinho.linhas.len())).grow(1.0)),
        ];
        if let Some(problema) = self.caixa.problema.get() {
            lado.push(Box::new(Alert::new("Atenção", problema).tone(Tone::Danger)));
        }
        lado.push(Box::new(
            VStack::new(vec![Box::new(titulo("CÓDIGO DE BARRAS")) as AnyView, Box::new(leitura)])
                .spacing(4.0)
                .align(Align::Stretch),
        ));
        Box::new(
            HStack::new(vec![
                Box::new(lista) as AnyView,
                Box::new(VStack::new(lado).spacing(16.0).align(Align::Stretch).width(430.0)),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(16.0)
            .grow(1.0),
        )
    }

    fn pagando(&self, cx: &mut BuildCx<'_>) -> AnyView {
        let total = self.caixa.carrinho.peek().total();
        let forma_atual = self.caixa.forma.get();
        let mut formas: Vec<AnyView> = Vec::new();
        for (index, forma) in Forma::TODAS.into_iter().enumerate() {
            let (caixa, recebido) = (self.caixa.clone(), valor_em_texto(total));
            let escolher = std::rc::Rc::new(move || {
                caixa.forma.set(forma);
                if !forma.tem_troco() {
                    caixa.recebido.set(recebido.clone());
                }
            });
            let tecla = (index + 1).to_string();
            Hotkey::alt(&tecla).register(cx, escolher.clone());
            formas.push(Box::new(
                Button::new(format!("{}  {}", Hotkey::alt(&tecla).label(), forma.nome()))
                    .size(ButtonSize::Large)
                    .full_width()
                    .style(if forma == forma_atual { ButtonStyle::Primary } else { ButtonStyle::Secondary })
                    .on_click(move || escolher()),
            ));
        }
        let (read, write) = (self.caixa.recebido.clone(), self.caixa.recebido.clone());
        let (caixa, session) = (self.caixa.clone(), self.session.clone());
        let campo = TextField::new(Binding::new(move || read.get(), move |value: String| write.set(value)))
            .autofocus()
            .select_on_open()
            .end_aligned()
            .on_submit(move |_| caixa.avancar(&session));
        let recebido = ravenrs_pos::ler_valor(&self.caixa.recebido.get()).unwrap_or_default();
        let mut esquerda: Vec<AnyView> = vec![
            Box::new(Text::new("Forma de pagamento").style(TextStyle::Title)),
            Box::new(VStack::new(formas).spacing(12.0).align(Align::Stretch)),
            Box::new(FormField::new("Valor recebido", campo)),
        ];
        if let Some(problema) = self.caixa.problema.get() {
            esquerda.push(Box::new(Alert::new("Atenção", problema).tone(Tone::Danger)));
        }
        let troco = ravenrs_pos::troco(forma_atual, total, recebido).unwrap_or_default();
        let lado = VStack::new(vec![
            Box::new(total_escuro("TOTAL", total, None)) as AnyView,
            Box::new(total_escuro("RECEBIDO", recebido, None)),
            Box::new(total_escuro("TROCO", troco, None).grow(1.0)),
        ])
        .spacing(8.0)
        .align(Align::Stretch)
        .width(470.0);
        Box::new(
            HStack::new(vec![
                Box::new(VStack::new(esquerda).spacing(16.0).align(Align::Stretch).grow(1.0)) as AnyView,
                Box::new(lado),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(16.0)
            .grow(1.0),
        )
    }

    fn enviando(&self) -> AnyView {
        Box::new(
            VStack::new(vec![
                Box::new(Spinner::new()) as AnyView,
                Box::new(Text::new("Enviando a nota").style(TextStyle::Title)),
            ])
            .spacing(16.0)
            .align(Align::Center)
            .justify(Justify::Center)
            .grow(1.0),
        )
    }

    fn resultado(&self, fim: &ravenrs_app_nfe::EmissaoFinal) -> AnyView {
        let (nome, tone) = match (fim.autorizada, fim.tentar_de_novo) {
            (true, _) => ("Nota autorizada", Tone::Success),
            (false, true) => ("A nota não saiu: dá para tentar de novo", Tone::Warning),
            (false, false) => ("Nota rejeitada", Tone::Danger),
        };
        let mut linhas: Vec<AnyView> =
            vec![Box::new(Alert::new(nome, format!("Código {}: {}", fim.c_stat, fim.motivo)).tone(tone))];
        let dado = |rotulo: &str, valor: String| -> AnyView {
            Box::new(
                HStack::new(vec![
                    Box::new(Text::new(rotulo.to_owned()).foreground(Semantic::InkMuted).width(150.0))
                        as AnyView,
                    Box::new(Text::new(valor).font_weight(Weight::SEMIBOLD)),
                ])
                .align(Align::Center),
            )
        };
        linhas.push(dado("Número", format!("{:09}", fim.numero)));
        if !fim.chave.is_empty() {
            linhas.push(dado("Chave de acesso", fim.chave.clone()));
        }
        if !fim.protocolo.is_empty() {
            linhas.push(dado("Protocolo", fim.protocolo.clone()));
        }
        linhas.push(dado(
            "Total e troco",
            format!("R$ {}, troco R$ {}", valor_em_texto(fim.total), valor_em_texto(fim.troco)),
        ));
        if fim.autorizada {
            let (caixa, session, chave) = (self.caixa.clone(), self.session.clone(), fim.chave.clone());
            linhas.push(Box::new(
                Button::new(if self.caixa.nota_grande.get() {
                    "Ver o DANFE"
                } else {
                    "Imprimir ou ver o cupom"
                })
                .icon(ravenrs_widgets::IconName::Printer)
                .on_click(move || caixa.imprimir_cupom(&session, &chave)),
            ));
            if !self.caixa.nota_grande.get() {
                let (caixa, session) = (self.caixa.clone(), self.session.clone());
                linhas.push(Box::new(
                    Button::new("Trocar por nota grande (cancela a NFC-e)")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || caixa.trocar_por_nota_grande(&session)),
                ));
            }
            if let Some(problema) = self.caixa.problema.get() {
                linhas.push(Box::new(Alert::new("Atenção", problema).tone(Tone::Warning)));
            }
        }
        linhas.push(Box::new(
            Text::new(if fim.autorizada { "F9 começa a próxima venda." } else { "F9 volta ao pagamento." })
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
        ));
        let mut direita: Vec<AnyView> = Vec::new();
        if self.caixa.service.sandbox_ativo().is_some() {
            direita.push(Box::new(CenariosSandbox { caixa: self.caixa.clone() }));
        }
        Box::new(
            HStack::new(vec![
                Box::new(Card::new(VStack::new(linhas).spacing(12.0).align(Align::Stretch)).grow(1.0))
                    as AnyView,
                Box::new(VStack::new(direita).spacing(16.0).align(Align::Stretch).width(430.0)),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(16.0)
            .grow(1.0),
        )
    }
}

/// A sum in large letters on the darkest ground, as the operator reads it from afar.
fn total_escuro(rotulo: &str, valor: rust_decimal::Decimal, itens: Option<usize>) -> impl View + use<> {
    let mut linhas: Vec<AnyView> = vec![
        Box::new(Text::new(rotulo.to_owned()).style(TextStyle::Caption).foreground(Semantic::Canvas)),
        Box::new(
            Text::new(format!("R$ {}", valor_em_texto(valor)))
                .font_size(56.0)
                .font_weight(Weight::SEMIBOLD)
                .foreground(Semantic::Canvas),
        ),
    ];
    if let Some(itens) = itens {
        linhas.push(Box::new(Text::new(format!("{itens} itens")).foreground(Semantic::Canvas)));
    }
    VStack::new(linhas).spacing(4.0).justify(Justify::Center).padding(24.0).background(Semantic::Ink)
}

/// How the sandbox answers the next document, for practicing at the counter.
struct CenariosSandbox {
    caixa: Caixa,
}

impl View for CenariosSandbox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let Some(sandbox) = self.caixa.service.sandbox_ativo() else {
            return cx.child(&Spacer::new());
        };
        let mut lines: Vec<AnyView> = vec![
            Box::new(Text::new("Cenário do sandbox").style(TextStyle::Headline)),
            Box::new(
                Text::new("Vale para a próxima emissão, para treinar o caixa.")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ),
        ];
        type Cenario = fn(&ravenrs_nfe::sandbox::SandboxRequest) -> ravenrs_nfe::sandbox::Resposta;
        let opcoes: [(&str, Cenario); 4] = [
            ("Assinatura inválida (297)", cenarios::assinatura_297),
            ("Duplicidade (204)", cenarios::duplicidade_204),
            ("Serviço paralisado (108)", cenarios::paralisado_108),
            ("Sem resposta (tempo esgotado)", cenarios::timeout),
        ];
        for (nome, cenario) in opcoes {
            let sandbox = sandbox.clone();
            lines.push(Box::new(
                Button::new(nome)
                    .style(ButtonStyle::Secondary)
                    .full_width()
                    .on_click(move || sandbox.use_once(autorizacao(cenario))),
            ));
        }
        cx.child(&Card::new(VStack::new(lines).spacing(8.0).align(Align::Stretch)))
    }
}

impl View for TelaCaixa {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let corpo = match self.caixa.tela.get() {
            Tela::Vendendo => self.vendendo(),
            Tela::Pagando => self.pagando(cx),
            Tela::Enviando => self.enviando(),
            Tela::Resultado(fim) => self.resultado(&fim),
        };
        cx.child(&VStack::new(vec![self.cabecalho(), corpo]).spacing(0.0).align(Align::Stretch).grow(1.0))
    }
}

/// The screen everything starts from.
pub(crate) struct TelaInicio {
    pub(crate) session: Session,
}

fn cartao(titulo: &str, texto: &str, tecla: &str, on_click: impl Fn() + 'static) -> AnyView {
    Box::new(
        Card::new(
            VStack::new(vec![
                Box::new(Text::new(tecla.to_owned()).style(TextStyle::Callout).foreground(Semantic::InkMuted))
                    as AnyView,
                Box::new(Text::new(titulo.to_owned()).style(TextStyle::Title)),
                Box::new(Text::new(texto.to_owned()).foreground(Semantic::InkMuted)),
            ])
            .spacing(8.0)
            .align(Align::Stretch),
        )
        .on_click(on_click)
        .grow(1.0),
    )
}

impl View for TelaInicio {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let abrir = |view: &'static str| {
            let session = self.session.clone();
            move || session.open_view(view)
        };
        let outras = {
            let session = self.session.clone();
            move || abrir_outras_funcoes(&session)
        };
        let atalhos = {
            let session = self.session.clone();
            move || session.show_shortcuts()
        };
        let primeira = HStack::new(vec![
            cartao("Lançar vendas", "Abre o caixa com a leitura de código pronta.", "F2", abrir(VIEW_CAIXA)),
            cartao(
                "Produtos",
                "Cadastro, preço, código e dados fiscais.",
                "F3",
                abrir("PosProduto_ListView"),
            ),
            cartao(
                "Etiquetas",
                "Etiquetas de gôndola feitas no editor de relatórios.",
                "F4",
                abrir(VIEW_ETIQUETAS),
            ),
        ])
        .spacing(16.0)
        .align(Align::Stretch)
        .grow(1.0);
        let terceira = HStack::new(vec![
            cartao(
                "Entrada de mercadoria",
                "Importa a nota de compra para o estoque e sugere etiquetas.",
                "F6",
                abrir(VIEW_ENTRADA),
            ),
            cartao(
                "Preços e markup",
                "Custo, impostos, custos fixos e margem até o preço.",
                "F7",
                abrir(VIEW_PRECO),
            ),
        ])
        .spacing(16.0)
        .align(Align::Stretch)
        .grow(1.0);
        let segunda = HStack::new(vec![
            cartao(
                "Outras funções",
                "Notas emitidas, emitente, modelos de cupom e configuração.",
                "F5",
                outras,
            ),
            cartao("Consulta de atalhos", "Todas as teclas do que dá para fazer aqui.", "F1", atalhos),
        ])
        .spacing(16.0)
        .align(Align::Stretch)
        .grow(1.0);
        cx.child(
            &VStack::new(vec![
                Box::new(Text::new("Mini-PDV").style(TextStyle::Display)) as AnyView,
                Box::new(primeira),
                Box::new(terceira),
                Box::new(segunda),
            ])
            .spacing(16.0)
            .align(Align::Stretch)
            .padding(32.0)
            .grow(1.0),
        )
    }
}
