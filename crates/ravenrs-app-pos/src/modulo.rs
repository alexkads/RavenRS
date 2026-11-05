//! The point of sale as a module: what it adds to an application, and its keys.

use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, AppRecordsModule, DashboardInfo, Message, MessageKind, Module, ViewController,
    WindowController,
};
use ravenrs_app_nfe::{CUPOM_LINHA, NfeModule, NfeService, modelos_de_cupom, modelos_de_danfe};
use ravenrs_app_report::{ReportWords, StoredReportsModule};
use ravenrs_app_ui::{App, Language, Session};
use ravenrs_report::barcode::Symbology;
use ravenrs_report::labels::{LabelStock, ShelfLabelFields, shelf_label};
use ravenrs_style::{TextStyle, Theme};
use ravenrs_widgets::{AnyView, Button, Text, TextField, VStack, ViewExt};

use crate::caixa::{Caixa, configuracao};
use crate::entrada::{Entradas, TelaEntrada, criar_codigo_de_barras};
use crate::etiquetas::TelaEtiquetas;
use crate::modelo::{
    PRODUTO, cliente_entity, codigo_entity, configuracao_entity, movimento_entity, produto_entity,
    vinculo_entity,
};
use crate::preco::TelaPreco;
use crate::telas::{TelaCaixa, TelaInicio};

/// The view of the first screen.
pub const VIEW_INICIO: &str = "PosInicio_DashboardView";
/// The view of the checkout.
pub const VIEW_CAIXA: &str = "PosCaixa_DashboardView";
/// The view of the shelf labels.
pub const VIEW_ETIQUETAS: &str = "PosEtiquetas_DashboardView";
/// The view of what arrives from suppliers.
pub const VIEW_ENTRADA: &str = "PosEntrada_DashboardView";
/// The view of pricing.
pub const VIEW_PRECO: &str = "PosPreco_DashboardView";

fn sessao(cx: &ravenrs_app::ActionContext) -> Result<Session, String> {
    Session::of(cx).ok_or_else(|| "esta ação precisa das telas".to_owned())
}

/// A question in a window, answered with a line of text.
fn perguntar(
    session: &Session,
    titulo: &'static str,
    atual: String,
    responder: impl Fn(String) + Clone + 'static,
) {
    let fechar = session.clone();
    let valor = ravenrs_reactive::State::new(atual);
    session.present(titulo, move || {
        let (read, write) = (valor.clone(), valor.clone());
        let (responder, fechar) = (responder.clone(), fechar.clone());
        Box::new(
            TextField::new(ravenrs_reactive::Binding::new(
                move || read.get(),
                move |text: String| write.set(text),
            ))
            .autofocus()
            .on_submit(move |text: String| {
                responder(text.trim().to_owned());
                fechar.close_presented();
            }),
        ) as AnyView
    });
}

/// The window with the rest of what can be done.
pub(crate) fn abrir_outras_funcoes(session: &Session) {
    let opcoes: [(&str, &str); 5] = [
        ("Notas emitidas", "NfeDocumento_ListView"),
        ("Emitente", "NfeEmitente_ListView"),
        ("Modelos de cupom e etiqueta", "ReportDefinition_ListView"),
        ("Clientes", "PosCliente_ListView"),
        ("Configuração do caixa", "PosConfiguracao_ListView"),
    ];
    let aberta = session.clone();
    session.present("Outras funções", move || {
        let botoes: Vec<AnyView> = opcoes
            .iter()
            .map(|(nome, view)| {
                let (session, view) = (aberta.clone(), *view);
                Box::new(Button::new(*nome).full_width().on_click(move || {
                    session.close_presented();
                    session.open_view(view);
                })) as AnyView
            })
            .collect();
        Box::new(VStack::new(botoes).spacing(8.0))
    });
}

/// The keys that work everywhere: the way back, the list of keys and locking.
struct Sempre {
    caixa: Caixa,
}

impl WindowController for Sempre {
    fn id(&self) -> &str {
        "PosSempre"
    }

    fn actions(&self) -> Vec<Action> {
        let caixa = self.caixa.clone();
        vec![
            Action::simple("Pos.Atalhos", "Atalhos").hotkey("f1").category(ActionCategory::View).on_execute(
                |cx| {
                    sessao(cx)?.show_shortcuts();
                    Ok(())
                },
            ),
            Action::simple("Pos.Voltar", "Início")
                .hotkey("esc")
                .category(ActionCategory::Navigation)
                .on_execute(move |cx| {
                    if !caixa.voltar() {
                        sessao(cx)?.open_view(VIEW_INICIO);
                    }
                    Ok(())
                }),
            Action::simple("Pos.Bloquear", "Bloquear")
                .hotkey("f12")
                .category(ActionCategory::View)
                .on_execute(|cx| {
                    let session = sessao(cx)?;
                    let pin = configuracao(&session, "pin");
                    session.lock("Caixa bloqueado. Digite o PIN.", move |typed| typed == pin);
                    Ok(())
                }),
        ]
    }
}

/// The keys of the first screen.
struct NoInicio;

impl ViewController for NoInicio {
    fn id(&self) -> &str {
        "PosInicio"
    }

    fn actions(&self) -> Vec<Action> {
        let ir = |id: &str, nome: &str, tecla: &str, view: &'static str| {
            Action::simple(id, nome).hotkey(tecla).target_view(VIEW_INICIO).on_execute(move |cx| {
                sessao(cx)?.open_view(view);
                Ok(())
            })
        };
        vec![
            ir("Pos.Vender", "Lançar vendas", "f2", VIEW_CAIXA),
            ir("Pos.Produtos", "Produtos", "f3", "PosProduto_ListView"),
            ir("Pos.Etiquetas", "Etiquetas", "f4", VIEW_ETIQUETAS),
            ir("Pos.Entrada", "Entrada", "f6", VIEW_ENTRADA),
            ir("Pos.Preco", "Preços", "f7", VIEW_PRECO),
            Action::simple("Pos.Outras", "Outras funções").hotkey("f5").target_view(VIEW_INICIO).on_execute(
                |cx| {
                    abrir_outras_funcoes(&sessao(cx)?);
                    Ok(())
                },
            ),
        ]
    }
}

/// What a product can do beyond its form.
struct NosProdutos;

impl ViewController for NosProdutos {
    fn id(&self) -> &str {
        "PosProdutos"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple("Pos.CriarCodigo", "Criar código de barras")
                .icon("scan-line")
                .target_entity(PRODUTO)
                .on_execute(|cx| {
                    let session = sessao(cx)?;
                    let produto = cx.selected.first().ok_or("Selecione um produto.")?;
                    let codigo = criar_codigo_de_barras(&session, &produto.get("id"))?;
                    session
                        .show_message(Message::new(format!("Código {codigo} criado."), MessageKind::Success));
                    Ok(())
                }),
        ]
    }
}

/// The keys of the checkout.
struct NoCaixa {
    caixa: Caixa,
}

impl ViewController for NoCaixa {
    fn id(&self) -> &str {
        "PosCaixa"
    }

    fn actions(&self) -> Vec<Action> {
        let grande = self.caixa.clone();
        let (item, venda, cliente, nota, pendentes, finalizar) = (
            self.caixa.clone(),
            self.caixa.clone(),
            self.caixa.clone(),
            self.caixa.clone(),
            self.caixa.clone(),
            self.caixa.clone(),
        );
        let no_caixa = |id: &str, nome: &str, tecla: &str| {
            Action::simple(id, nome).hotkey(tecla).target_view(VIEW_CAIXA).category(ActionCategory::Edit)
        };
        vec![
            no_caixa("Pos.CancelarItem", "Cancelar item", "f6").on_execute(move |_| {
                item.cancelar_ultimo();
                Ok(())
            }),
            no_caixa("Pos.Cliente", "Cliente", "alt+c").on_execute(move |cx| {
                let session = sessao(cx)?;
                let atual = cliente.carrinho.peek().cliente.clone().unwrap_or_default();
                let caixa = cliente.clone();
                perguntar(&session, "CPF ou CNPJ do cliente", atual, move |documento| {
                    let digitos: String = documento.chars().filter(char::is_ascii_alphanumeric).collect();
                    caixa.carrinho.update(|carrinho| {
                        carrinho.cliente = (!digitos.is_empty()).then_some(digitos);
                    });
                });
                Ok(())
            }),
            no_caixa("Pos.NotaGrande", "Nota grande", "f8").on_execute(move |_| {
                grande.nota_grande.update(|ligada| *ligada = !*ligada);
                Ok(())
            }),
            no_caixa("Pos.Observacao", "Observação", "ctrl+o").on_execute(move |cx| {
                let session = sessao(cx)?;
                let atual = nota.carrinho.peek().observacao.clone().unwrap_or_default();
                let caixa = nota.clone();
                perguntar(&session, "Observação do documento", atual, move |texto| {
                    caixa
                        .carrinho
                        .update(|carrinho| carrinho.observacao = (!texto.is_empty()).then_some(texto));
                });
                Ok(())
            }),
            no_caixa("Pos.Pendentes", "Pendentes", "f10").on_execute(move |cx| {
                let session = sessao(cx)?;
                abrir_pendentes(&session, &pendentes);
                Ok(())
            }),
            no_caixa("Pos.CancelarVenda", "Cancelar venda", "f11").on_execute(move |_| {
                venda.cancelar_venda();
                Ok(())
            }),
            Action::simple("Pos.Finalizar", "Finalizar")
                .hotkey("f9")
                .target_view(VIEW_CAIXA)
                .category(ActionCategory::Save)
                .on_execute(move |cx| {
                    finalizar.avancar(&sessao(cx)?);
                    Ok(())
                }),
        ]
    }
}

/// The sales kept: guard this one, or pick one up.
fn abrir_pendentes(session: &Session, caixa: &Caixa) {
    let (aberta, caixa) = (session.clone(), caixa.clone());
    session.present("Vendas pendentes", move || {
        let mut linhas: Vec<AnyView> = Vec::new();
        if !caixa.carrinho.peek().vazio() {
            let (c, s) = (caixa.clone(), aberta.clone());
            linhas.push(Box::new(Button::new("Guardar a venda aberta").full_width().on_click(move || {
                s.close_presented();
                if let Err(why) = c.guardar(&s) {
                    s.show_message(Message::new(why, MessageKind::Warning));
                }
            })));
        }
        let kept = caixa.pendentes(&aberta);
        if kept.is_empty() {
            linhas.push(Box::new(Text::new("Nenhuma venda guardada.").style(TextStyle::Callout)));
        }
        for (id, titulo) in kept {
            let (c, s) = (caixa.clone(), aberta.clone());
            linhas.push(Box::new(Button::new(titulo).full_width().on_click(move || {
                s.close_presented();
                if let Err(why) = c.retomar(&s, &id) {
                    s.show_message(Message::new(why, MessageKind::Warning));
                }
            })));
        }
        Box::new(VStack::new(linhas).spacing(8.0))
    });
}

struct PosModule {
    caixa: Caixa,
}

impl Module for PosModule {
    fn name(&self) -> &str {
        "Pos"
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["Nfe".to_owned(), "AppRecords".to_owned()]
    }

    fn entities(&self) -> Vec<ravenrs_app::EntityInfo> {
        vec![
            produto_entity(),
            cliente_entity(),
            configuracao_entity(),
            vinculo_entity(),
            codigo_entity(),
            movimento_entity(),
        ]
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(NoInicio), Box::new(NosProdutos), Box::new(NoCaixa { caixa: self.caixa.clone() })]
    }

    fn window_controllers(&self) -> Vec<Box<dyn WindowController>> {
        vec![Box::new(Sempre { caixa: self.caixa.clone() })]
    }
}

/// The label that comes with it, for a roll of 60 x 40 mm.
fn etiqueta_padrao() -> ravenrs_report::Report {
    let mut etiqueta = shelf_label(
        &LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0),
        "Gôndola 60 x 40 mm",
        &ShelfLabelFields {
            description: "[descricao]".to_owned(),
            price: "[preco]".to_owned(),
            price_format: "C2".to_owned(),
            barcode: "[gtin]".to_owned(),
            symbology: Symbology::Ean13,
        },
    );
    etiqueta.culture = "pt-BR".to_owned();
    etiqueta
}

/// A point of sale, to add to an application.
#[derive(Clone, Debug)]
pub struct Pos {
    caixa: Caixa,
    entradas: Entradas,
    sessao: Rc<std::cell::RefCell<Option<Session>>>,
}

impl Pos {
    /// A point of sale issuing through this service.
    pub fn new(service: NfeService) -> Self {
        Self { caixa: Caixa::new(service), entradas: Entradas::default(), sessao: Rc::default() }
    }

    /// The session of the window, once it is put together.
    pub fn sessao(&self) -> Option<Session> {
        self.sessao.borrow().clone()
    }

    /// What arrives from suppliers.
    pub fn entradas(&self) -> &Entradas {
        &self.entradas
    }

    /// Its checkout.
    pub fn caixa(&self) -> &Caixa {
        &self.caixa
    }

    /// Adds it all to an application: the records, the screens, the look of a counter and a window
    /// that opens on its first screen.
    pub fn instalar(&self, app: App) -> App {
        let mut relatorios = StoredReportsModule::new(ReportWords::pt_br()).predefined(
            "Gôndola 60 x 40 mm",
            PRODUTO,
            etiqueta_padrao(),
        );
        for (nome, modelo) in modelos_de_cupom().into_iter().chain(modelos_de_danfe()) {
            relatorios = relatorios.predefined(nome, CUPOM_LINHA, modelo);
        }
        let (caixa, palavras) = (self.caixa.clone(), Rc::new(ReportWords::pt_br()));
        let entradas = self.entradas.clone();
        let sessao_guardada = self.sessao.clone();
        app.language(Language::BrazilianPortuguese)
            .theme(Theme::raven_light())
            .counter()
            .module(AppRecordsModule)
            .module(relatorios)
            .module(NfeModule::new(self.caixa.service.clone()))
            .module(PosModule { caixa: self.caixa.clone() })
            .dashboard(DashboardInfo::new("PosInicio", "Início").control("Início", "pos.inicio"))
            .dashboard(DashboardInfo::new("PosCaixa", "Caixa").control("Caixa", "pos.caixa"))
            .dashboard(DashboardInfo::new("PosEtiquetas", "Etiquetas").control("Etiquetas", "pos.etiquetas"))
            .dashboard(DashboardInfo::new("PosEntrada", "Entrada").control("Entrada", "pos.entrada"))
            .dashboard(DashboardInfo::new("PosPreco", "Preços").control("Preços", "pos.preco"))
            .with_session(move |session| {
                *sessao_guardada.borrow_mut() = Some(session.clone());
                let aqui = session.clone();
                session.register_control("pos.inicio", move || {
                    Box::new(TelaInicio { session: aqui.clone() }.grow(1.0)) as AnyView
                });
                let (aqui, caixa_aqui) = (session.clone(), caixa.clone());
                session.register_control("pos.caixa", move || {
                    Box::new(TelaCaixa { caixa: caixa_aqui.clone(), session: aqui.clone() }.grow(1.0))
                        as AnyView
                });
                let (aqui, copias) = (session.clone(), entradas.etiquetas.clone());
                session.register_control("pos.etiquetas", move || {
                    Box::new(
                        TelaEtiquetas {
                            session: aqui.clone(),
                            palavras: palavras.clone(),
                            copias: copias.clone(),
                        }
                        .grow(1.0),
                    ) as AnyView
                });
                let (aqui, entradas_aqui) = (session.clone(), entradas.clone());
                session.register_control("pos.entrada", move || {
                    Box::new(TelaEntrada { entradas: entradas_aqui.clone(), session: aqui.clone() }.grow(1.0))
                        as AnyView
                });
                let aqui = session.clone();
                session.register_control("pos.preco", move || {
                    Box::new(TelaPreco { session: aqui.clone() }.grow(1.0)) as AnyView
                });
                session.open_view(VIEW_INICIO);
            })
    }
}
