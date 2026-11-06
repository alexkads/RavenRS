//! What arrives from suppliers: an invoice goes into stock, what is new gets a product and a barcode,
//! and whatever goes back on the shelf gets its label suggested.

use std::collections::BTreeMap;

use ravenrs_app::{FieldValue, Object, ObjectSpace, Query, SpaceError, Value, next_number};
use ravenrs_app_nfe::engine::entrada::{ItemRecebido, NotaRecebida, ler_nota_recebida};
use ravenrs_app_ui::Session;
use ravenrs_layout::Align;
use ravenrs_pos::{
    Composicao, LinhaCompra, arredondar_preco, codigo_interno, entrar_no_estoque, ler_valor,
    sugerir_etiqueta, valor_em_texto,
};
use ravenrs_reactive::{Binding, State};
use ravenrs_report::barcode::is_valid_gtin;
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Card, Checkbox, FileRequest, HStack, NodeId, ScrollView,
    Spacer, Text, TextField, Tone, VStack, View, ViewExt,
};
use rust_decimal::Decimal;

use crate::caixa::{configuracao_decimal, texto};
use crate::codigos::{cadastrar_codigo, produto_por_codigo};
use crate::modelo::{MOVIMENTO, PRODUTO, VINCULO};
use crate::modulo::VIEW_ETIQUETAS;

/// How big an invoice can be.
const MAIOR_XML: u64 = 10 * 1024 * 1024;

/// What is on the way into stock. A clone is the same.
#[derive(Clone, Debug)]
pub struct Entradas {
    /// The invoice being looked at.
    pub nota: State<Option<NotaRecebida>>,
    /// How many of our units come in one of theirs, as typed, by line.
    pub fatores: State<BTreeMap<u32, String>>,
    /// A barcode of the store's choice for a line, as typed, by line.
    pub codigos: State<BTreeMap<u32, String>>,
    /// How many units that barcode counts as, as typed, by line.
    pub quantidades_dos_codigos: State<BTreeMap<u32, String>>,
    /// Whether the prices calculated also go on the products.
    pub atualizar_precos: State<bool>,
    /// What went wrong, or how it ended.
    pub aviso: State<Option<(Tone, String)>>,
    /// Purchases found at the SEFAZ, by what tells them apart, with their XML.
    pub encontradas: State<Vec<(String, String)>>,
    /// The labels suggested by what arrived, by the product's code, for the screen of labels.
    pub etiquetas: State<BTreeMap<String, String>>,
}

impl Default for Entradas {
    fn default() -> Self {
        Self {
            nota: State::new(None),
            fatores: State::new(BTreeMap::new()),
            codigos: State::new(BTreeMap::new()),
            quantidades_dos_codigos: State::new(BTreeMap::new()),
            atualizar_precos: State::new(true),
            aviso: State::new(None),
            etiquetas: State::new(BTreeMap::new()),
            encontradas: State::new(Vec::new()),
        }
    }
}

/// A line of the invoice, as it would go into stock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Previa {
    /// The line.
    pub numero: u32,
    /// What the supplier calls it.
    pub descricao: String,
    /// The product it goes into, by its code; none means a new one.
    pub produto: Option<String>,
    /// How many of ours in one of theirs.
    pub fator: Decimal,
    /// How many of ours arrive.
    pub unidades: Decimal,
    /// What one of ours cost.
    pub custo: Decimal,
    /// The price on the shelf now, if it is on one.
    pub preco_atual: Option<Decimal>,
    /// The price that covers it all.
    pub preco_novo: Option<Decimal>,
}

/// The product a line of this supplier goes into, by what it was linked to or by its barcode.
fn produto_da_linha(space: &dyn ObjectSpace, fornecedor: &str, item: &ItemRecebido) -> Option<Object> {
    let produtos = space.query(PRODUTO, &Query::all()).ok()?;
    let vinculos = space.query(VINCULO, &Query::all()).ok()?;
    let vinculado = vinculos.iter().find(|vinculo| {
        texto(vinculo, "fornecedor") == fornecedor && texto(vinculo, "codigo_fornecedor") == item.codigo
    });
    if let Some(vinculo) = vinculado {
        let codigo = texto(vinculo, "produto");
        if let Some(produto) = produtos.iter().find(|produto| texto(produto, "codigo") == codigo) {
            return Some(produto.clone());
        }
    }
    let gtin = item.gtin.as_deref()?;
    produto_por_codigo(space, gtin).map(|achado| achado.produto)
}

/// The factor kept for a line of this supplier.
fn fator_salvo(space: &dyn ObjectSpace, fornecedor: &str, item: &ItemRecebido) -> Option<Decimal> {
    let vinculos = space.query(VINCULO, &Query::all()).ok()?;
    vinculos
        .iter()
        .find(|vinculo| {
            texto(vinculo, "fornecedor") == fornecedor && texto(vinculo, "codigo_fornecedor") == item.codigo
        })
        .map(|vinculo| Decimal::from_value(vinculo.get("fator")))
        .filter(|fator| *fator > Decimal::ZERO)
}

fn preco_em_reais(produto: &Object) -> Decimal {
    match produto.get("preco") {
        Value::Number(centavos) => Decimal::new(centavos.round() as i64, 2),
        _ => Decimal::ZERO,
    }
}

/// The price a cost asks for, with what is set on the checkout.
fn preco_para(session: &Session, custo: Decimal) -> Option<Decimal> {
    let composicao = Composicao {
        custo,
        impostos: configuracao_decimal(session, "impostos"),
        custos_fixos: configuracao_decimal(session, "custos_fixos"),
        comissao: configuracao_decimal(session, "comissao"),
        margem: configuracao_decimal(session, "margem"),
        ..Composicao::default()
    };
    let formacao = composicao.calcular().ok()?;
    let centavos =
        u8::try_from(configuracao_decimal(session, "final_de_preco").trunc().mantissa()).unwrap_or(90);
    Some(arredondar_preco(formacao.preco, centavos))
}

fn linha_de_compra(item: &ItemRecebido, fator: Decimal) -> LinhaCompra {
    LinhaCompra {
        quantidade: item.quantidade,
        valor_total: item.valor_total,
        desconto: item.desconto,
        acrescimos: item.frete + item.seguro + item.outros,
        impostos_no_custo: item.valor_ipi + item.valor_icms_st,
        fator,
    }
}

impl Entradas {
    /// Whether the invoice with this key is already in stock.
    pub fn ja_entrou(&self, session: &Session, chave: &str) -> bool {
        let space = session.frame().space();
        let space = space.borrow();
        space
            .query(MOVIMENTO, &Query::all())
            .unwrap_or_default()
            .iter()
            .any(|movimento| texto(movimento, "documento") == chave)
    }

    /// Takes in what the SEFAZ sent, leaving out what is in stock, listed or open already: after
    /// the acknowledgement the national environment sends the full invoice again, under a new NSU.
    /// Says how many are new.
    pub fn receber_da_sefaz(&self, session: &Session, xmls: Vec<String>) -> usize {
        let mut vistas: Vec<String> = self
            .encontradas
            .peek()
            .iter()
            .filter_map(|(_, xml)| ler_nota_recebida(xml).ok().map(|nota| nota.chave))
            .collect();
        if let Some(aberta) = self.nota.peek().as_ref() {
            vistas.push(aberta.chave.clone());
        }
        let mut novas = Vec::new();
        for xml in xmls {
            let Ok(nota) = ler_nota_recebida(&xml) else { continue };
            if vistas.contains(&nota.chave) || self.ja_entrou(session, &nota.chave) {
                continue;
            }
            vistas.push(nota.chave.clone());
            let titulo =
                format!("{}, nota {}, R$ {}", nota.emitente_nome, nota.numero, valor_em_texto(nota.total));
            novas.push((titulo, xml));
        }
        let quantas = novas.len();
        self.encontradas.update(|todas| todas.extend(novas));
        self.aviso.set(Some((
            Tone::Accent,
            match quantas {
                0 => "Nenhuma nota nova na SEFAZ.".to_owned(),
                1 => "1 nota de compra encontrada na SEFAZ.".to_owned(),
                _ => format!("{quantas} notas de compra encontradas na SEFAZ."),
            },
        )));
        quantas
    }

    /// Reads an invoice from its XML, refusing one already in stock or addressed to someone else.
    pub fn carregar(&self, session: &Session, xml: &str) -> Result<(), String> {
        let nota = ler_nota_recebida(xml).map_err(|error| error.to_string())?;
        if self.ja_entrou(session, &nota.chave) {
            return Err(format!("A nota {} já entrou no estoque.", nota.numero));
        }
        let space = session.frame().space();
        let space = space.borrow();
        if let Some(emitente) = ravenrs_app_nfe::emitente_salvo_de(&*space)
            && !nota.destinada_a(&texto(&emitente, "cnpj"))
        {
            return Err("Essa nota não foi emitida para o CNPJ desta loja.".to_owned());
        }
        drop(space);
        self.aviso.set(None);
        self.fatores.set(BTreeMap::new());
        self.codigos.set(BTreeMap::new());
        self.quantidades_dos_codigos.set(BTreeMap::new());
        self.nota.set(Some(nota));
        Ok(())
    }

    fn fator_de(&self, space: &dyn ObjectSpace, fornecedor: &str, item: &ItemRecebido) -> Decimal {
        self.fatores
            .with(|all| all.get(&item.numero).and_then(|typed| ler_valor(typed)))
            .filter(|fator| *fator > Decimal::ZERO)
            .or_else(|| fator_salvo(space, fornecedor, item))
            .unwrap_or(Decimal::ONE)
    }

    /// How the invoice would go into stock.
    pub fn previa(&self, session: &Session) -> Vec<Previa> {
        let Some(nota) = self.nota.get() else { return Vec::new() };
        let space = session.frame().space();
        let space = space.borrow();
        nota.itens
            .iter()
            .map(|item| {
                let produto = produto_da_linha(&*space, &nota.emitente_cnpj, item);
                let fator = self.fator_de(&*space, &nota.emitente_cnpj, item);
                let compra = linha_de_compra(item, fator);
                let custo = compra.custo_unitario().unwrap_or_default();
                Previa {
                    numero: item.numero,
                    descricao: item.descricao.clone(),
                    produto: produto.as_ref().map(|produto| texto(produto, "codigo")),
                    fator,
                    unidades: compra.unidades(),
                    custo,
                    preco_atual: produto.as_ref().map(preco_em_reais),
                    preco_novo: preco_para(session, custo),
                }
            })
            .collect()
    }

    /// Puts the invoice into stock, all of it or none, and suggests labels for what goes on the shelf.
    pub fn confirmar(&self, session: &Session) -> Result<usize, String> {
        let Some(nota) = self.nota.get() else { return Err("Não há nota para dar entrada.".to_owned()) };
        let atualizar = *self.atualizar_precos.peek();
        // Read before the space is taken for writing: it reads the settings through the same space.
        let precos: BTreeMap<u32, Option<Decimal>> =
            self.previa(session).iter().map(|previa| (previa.numero, previa.preco_novo)).collect();
        let shared = session.frame().space();
        let mut space = shared.borrow_mut();
        let result = (|| -> Result<BTreeMap<String, String>, SpaceError> {
            let mut etiquetas = BTreeMap::new();
            for item in &nota.itens {
                let fator = self.fator_de(&*space, &nota.emitente_cnpj, item);
                let compra = linha_de_compra(item, fator);
                let custo = compra.custo_unitario().map_err(SpaceError::Conflict)?;
                let unidades = compra.unidades();
                let preco_novo = precos.get(&item.numero).copied().flatten();
                let existente = produto_da_linha(&*space, &nota.emitente_cnpj, item);
                let mut produto = match existente {
                    Some(produto) => produto,
                    None => novo_produto(&mut *space, item)?,
                };
                let estoque = Decimal::from_value(produto.get("estoque"));
                let medio = Decimal::from_value(produto.get("custo"));
                let depois = entrar_no_estoque(estoque, medio, unidades, custo);
                produto.set("estoque", Decimal::to_value(&depois.quantidade));
                produto.set("custo", Decimal::to_value(&depois.custo_medio));
                let preco_atual = preco_em_reais(&produto);
                let preco_final = match preco_novo {
                    Some(preco) if atualizar || preco_atual.is_zero() => preco,
                    _ => preco_atual,
                };
                let sugestao = sugerir_etiqueta(preco_atual, preco_final);
                let centavos = (preco_final * Decimal::ONE_HUNDRED).round();
                produto.set("preco", Decimal::to_value(&centavos));
                let codigo = texto(&produto, "codigo");
                space.save(produto)?;
                guardar_vinculo(&mut *space, &nota.emitente_cnpj, item, &codigo, fator)?;
                // What the supplier prints on this pack is one more code of this product.
                if let Some(gtin) = item.gtin.as_deref().filter(|gtin| is_valid_gtin(gtin)) {
                    cadastrar_codigo(&mut *space, &codigo, gtin, "Código do fornecedor", Decimal::ONE)?;
                }
                let personalizado =
                    self.codigos.with(|all| all.get(&item.numero).cloned()).unwrap_or_default();
                if !personalizado.trim().is_empty() {
                    let digitado = self.quantidades_dos_codigos.with(|all| all.get(&item.numero).cloned());
                    let vale = match digitado.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
                        Some(text) => ler_valor(text).ok_or_else(|| {
                            SpaceError::Conflict(format!(
                                "A quantidade do código {personalizado} não é um número."
                            ))
                        })?,
                        None => Decimal::ONE,
                    };
                    cadastrar_codigo(&mut *space, &codigo, &personalizado, "Personalizado", vale)?;
                }
                let mut movimento = space.new_object(MOVIMENTO)?;
                movimento.set("produto", codigo.clone());
                movimento.set("origem", format!("Compra de {}", nota.emitente_nome));
                movimento.set("documento", nota.chave.clone());
                movimento.set("quantidade", Decimal::to_value(&unidades));
                movimento.set("custo", Decimal::to_value(&custo));
                space.save(movimento)?;
                etiquetas.insert(codigo, sugestao.copias.to_string());
            }
            space.commit()?;
            Ok(etiquetas)
        })();
        let etiquetas = match result {
            Ok(etiquetas) => etiquetas,
            Err(error) => {
                space.rollback();
                return Err(error.to_string());
            }
        };
        drop(space);
        let quantas = etiquetas.len();
        self.etiquetas.set(etiquetas);
        self.nota.set(None);
        self.aviso
            .set(Some((Tone::Success, format!("Entrada feita. {quantas} etiquetas de gôndola sugeridas."))));
        session.records_changed();
        Ok(quantas)
    }
}

/// A product made from a line of an invoice, with a barcode of the store if it comes with none.
fn novo_produto(space: &mut dyn ObjectSpace, item: &ItemRecebido) -> Result<Object, SpaceError> {
    let numero = next_number(space, "pos-produto")?;
    let gtin = match item.gtin.as_deref().filter(|gtin| is_valid_gtin(gtin)) {
        Some(gtin) => gtin.to_owned(),
        None => {
            let sequencia = next_number(space, "pos-codigo-interno")?;
            codigo_interno(sequencia.unsigned_abs()).map_err(SpaceError::Conflict)?
        }
    };
    let mut produto = space.new_object(PRODUTO)?;
    produto.set("codigo", format!("C{numero:06}"));
    produto.set("gtin", gtin);
    produto.set("descricao", item.descricao.clone());
    produto.set("ncm", item.ncm.clone());
    produto.set("unidade", "UN".to_owned());
    Ok(produto)
}

/// Remembers what this supplier calls the product, and how many of ours come in one of theirs.
fn guardar_vinculo(
    space: &mut dyn ObjectSpace,
    fornecedor: &str,
    item: &ItemRecebido,
    produto: &str,
    fator: Decimal,
) -> Result<(), SpaceError> {
    let vinculos = space.query(VINCULO, &Query::all())?;
    let existente = vinculos.into_iter().find(|vinculo| {
        texto(vinculo, "fornecedor") == fornecedor && texto(vinculo, "codigo_fornecedor") == item.codigo
    });
    let mut vinculo = match existente {
        Some(vinculo) => vinculo,
        None => space.new_object(VINCULO)?,
    };
    vinculo.set("fornecedor", fornecedor.to_owned());
    vinculo.set("codigo_fornecedor", item.codigo.clone());
    vinculo.set("produto", produto.to_owned());
    vinculo.set("fator", Decimal::to_value(&fator));
    space.save(vinculo)
}

/// A barcode of the store for a product that has none, kept with it.
pub fn criar_codigo_de_barras(session: &Session, id: &Value) -> Result<String, String> {
    let shared = session.frame().space();
    let mut space = shared.borrow_mut();
    let result = (|| -> Result<String, SpaceError> {
        let mut produto = space
            .find(PRODUTO, id)?
            .ok_or_else(|| SpaceError::Conflict("Esse produto não existe mais.".to_owned()))?;
        let atual = texto(&produto, "gtin");
        if !atual.is_empty() {
            return Err(SpaceError::Conflict(format!("Esse produto já tem o código {atual}.")));
        }
        let sequencia = next_number(&mut *space, "pos-codigo-interno")?;
        let codigo = codigo_interno(sequencia.unsigned_abs()).map_err(SpaceError::Conflict)?;
        produto.set("gtin", codigo.clone());
        space.save(produto)?;
        space.commit()?;
        Ok(codigo)
    })();
    if result.is_err() {
        space.rollback();
    }
    drop(space);
    session.records_changed();
    result.map_err(|error| error.to_string())
}

/// The screen of what arrives.
pub(crate) struct TelaEntrada {
    pub(crate) entradas: Entradas,
    pub(crate) session: Session,
    pub(crate) service: ravenrs_app_nfe::NfeService,
}

impl View for TelaEntrada {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let entradas = &self.entradas;
        let session = &self.session;
        let picker = cx.file_picker();
        let waker = cx.ui_waker();
        let importar = {
            let (entradas, session) = (entradas.clone(), session.clone());
            Button::new("Importar XML da nota").icon(ravenrs_widgets::IconName::Upload).on_click(move || {
                let (entradas, session, waker) = (entradas.clone(), session.clone(), waker.clone());
                let mut pedido = FileRequest::default();
                pedido.extensions = vec!["xml".to_owned()];
                pedido.multiple = false;
                pedido.most_bytes = Some(MAIOR_XML);
                picker.pick(&pedido, move |files| {
                    if let Some(file) = files.first() {
                        let lido = String::from_utf8_lossy(&file.bytes).into_owned();
                        if let Err(why) = entradas.carregar(&session, &lido) {
                            entradas.aviso.set(Some((Tone::Danger, why)));
                        }
                    }
                    waker.wake();
                });
            })
        };
        let buscar = {
            let (entradas, session, service) = (entradas.clone(), session.clone(), self.service.clone());
            Button::new("Buscar na SEFAZ").style(ButtonStyle::Secondary).on_click(move || {
                let entradas = entradas.clone();
                service.buscar_compras(&session, move |session, buscado| match buscado {
                    Ok(xmls) => {
                        entradas.receber_da_sefaz(session, xmls);
                    }
                    Err(why) => entradas.aviso.set(Some((Tone::Danger, why))),
                });
            })
        };
        let topo: Vec<AnyView> = vec![
            Box::new(Text::new("Entrada de mercadoria").style(TextStyle::Title)),
            Box::new(Spacer::new()),
            Box::new(buscar),
            Box::new(importar),
        ];
        let mut corpo: Vec<AnyView> = vec![Box::new(HStack::new(topo).spacing(12.0).align(Align::Center))];
        if let Some((tone, texto)) = entradas.aviso.get() {
            corpo.push(Box::new(Alert::new("Entrada", texto).tone(tone)));
        }
        if entradas.aviso.get().is_some_and(|(tone, _)| tone == Tone::Success) {
            let session = session.clone();
            corpo.push(Box::new(
                Button::new("Imprimir as etiquetas de gôndola sugeridas")
                    .style(ButtonStyle::Primary)
                    .on_click(move || session.open_view(VIEW_ETIQUETAS)),
            ));
        }
        for (titulo, xml) in entradas.encontradas.get() {
            let (e, s) = (entradas.clone(), session.clone());
            corpo.push(Box::new(
                Button::new(format!("Abrir: {titulo}")).style(ButtonStyle::Secondary).on_click(move || {
                    if let Err(why) = e.carregar(&s, &xml) {
                        e.aviso.set(Some((Tone::Danger, why)));
                    }
                    e.encontradas.update(|all| all.retain(|(_, outro)| *outro != xml));
                }),
            ));
        }
        match entradas.nota.get() {
            None => corpo.push(Box::new(
                Text::new("Importe o XML da nota de compra para ver o que entra no estoque.")
                    .foreground(Semantic::InkMuted),
            )),
            Some(nota) => {
                corpo.push(Box::new(
                    Text::new(format!(
                        "{}, nota {} série {}, total R$ {}",
                        nota.emitente_nome,
                        nota.numero,
                        nota.serie,
                        valor_em_texto(nota.total)
                    ))
                    .style(TextStyle::Headline),
                ));
                let mut linhas: Vec<AnyView> = Vec::new();
                for previa in entradas.previa(session) {
                    let numero = previa.numero;
                    let (read, write) = (entradas.fatores.clone(), entradas.fatores.clone());
                    let inicial = previa.fator.normalize().to_string().replace('.', ",");
                    let fator = TextField::new(Binding::new(
                        move || read.with(|all| all.get(&numero).cloned().unwrap_or_else(|| inicial.clone())),
                        move |value: String| {
                            write.update(|all| {
                                all.insert(numero, value);
                            });
                        },
                    ))
                    .compact()
                    .end_aligned()
                    .width(64.0);
                    let (read, write) = (entradas.codigos.clone(), entradas.codigos.clone());
                    let codigo_proprio = TextField::new(Binding::new(
                        move || read.with(|all| all.get(&numero).cloned().unwrap_or_default()),
                        move |value: String| {
                            write.update(|all| {
                                all.insert(numero, value);
                            });
                        },
                    ))
                    .placeholder("Código personalizado")
                    .compact()
                    .width(170.0);
                    let (read, write) =
                        (entradas.quantidades_dos_codigos.clone(), entradas.quantidades_dos_codigos.clone());
                    let vale_quanto = TextField::new(Binding::new(
                        move || read.with(|all| all.get(&numero).cloned().unwrap_or_default()),
                        move |value: String| {
                            write.update(|all| {
                                all.insert(numero, value);
                            });
                        },
                    ))
                    .placeholder("Vale 1 un")
                    .compact()
                    .end_aligned()
                    .width(84.0);
                    let situacao = previa.produto.as_ref().map_or_else(
                        || "Produto novo, com código criado".to_owned(),
                        |codigo| format!("Recompra do {codigo}"),
                    );
                    let preco = match (previa.preco_atual, previa.preco_novo) {
                        (Some(atual), Some(novo)) if atual != novo => {
                            format!("R$ {} para R$ {}", valor_em_texto(atual), valor_em_texto(novo))
                        }
                        (_, Some(novo)) => format!("R$ {}", valor_em_texto(novo)),
                        _ => "sem preço calculado".to_owned(),
                    };
                    linhas.push(Box::new(Card::new(
                        HStack::new(vec![
                            Box::new(
                                VStack::new(vec![
                                    Box::new(Text::new(previa.descricao.clone()).style(TextStyle::Headline))
                                        as AnyView,
                                    Box::new(
                                        Text::new(situacao)
                                            .style(TextStyle::Callout)
                                            .foreground(Semantic::InkMuted),
                                    ),
                                ])
                                .spacing(2.0)
                                .align(Align::Stretch)
                                .grow(1.0),
                            ) as AnyView,
                            Box::new(codigo_proprio),
                            Box::new(vale_quanto),
                            Box::new(fator),
                            Box::new(Text::new(format!(
                                "{} un, custo R$ {}",
                                previa.unidades.normalize().to_string().replace('.', ","),
                                valor_em_texto(previa.custo)
                            ))),
                            Box::new(Text::new(preco).font_weight(ravenrs_style::Weight::SEMIBOLD)),
                        ])
                        .spacing(16.0)
                        .align(Align::Center),
                    )));
                }
                corpo.push(Box::new(
                    ScrollView::new(vec![
                        Box::new(VStack::new(linhas).spacing(8.0).align(Align::Stretch)) as AnyView
                    ])
                    .grow(1.0),
                ));
                let (read, write) = (entradas.atualizar_precos.clone(), entradas.atualizar_precos.clone());
                corpo.push(Box::new(Checkbox::new(
                    "Aplicar os preços calculados com a margem da configuração",
                    Binding::new(move || read.get(), move |on: bool| write.set(on)),
                )));
                let (e, s) = (entradas.clone(), session.clone());
                corpo.push(Box::new(
                    Button::new("Confirmar a entrada").style(ButtonStyle::Primary).on_click(move || {
                        if let Err(why) = e.confirmar(&s) {
                            e.aviso.set(Some((Tone::Danger, why)));
                        }
                    }),
                ));
            }
        }
        cx.child(&VStack::new(corpo).spacing(12.0).align(Align::Stretch).padding(16.0).grow(1.0))
    }
}
