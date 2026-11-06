//! Selling a service at the counter: the NFS-e of the national standard, not the NFC-e. A sale of
//! products and services together waits for the rule of an accountant, so each goes on its own.

use std::rc::Rc;

use ravenrs_app::{Message, MessageKind, Object, Query, Value};
use ravenrs_app_nfe::nfse::Tomador;
use ravenrs_app_nfe::{
    NFSE_DOCUMENTO, NfeService, NfseFinal, ServicoVendido, inscricao_de, mostrar_danfse, nfses_pendentes,
};
use ravenrs_app_report::ReportWords;
use ravenrs_app_ui::Session;
use ravenrs_layout::Align;
use ravenrs_pos::{ler_valor, valor_em_texto};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::TextStyle;
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, FormField, HStack, NodeId, ScrollView, Select, Text,
    TextField, Tone, VStack, View, ViewExt,
};
use rust_decimal::Decimal;

use crate::caixa::texto;
use crate::modelo::SERVICO;

/// What is typed in to sell a service, and how the last one ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VendaServico {
    /// The internal code of the service chosen.
    pub servico: Option<String>,
    /// The value, as typed.
    pub valor: String,
    /// What was done, as the NFS-e says it.
    pub descricao: String,
    /// The CPF or CNPJ of who takes it, if any.
    pub documento: String,
    /// The name of who takes it.
    pub nome: String,
    /// The DPS is on its way.
    pub enviando: bool,
    /// What is wrong before sending.
    pub problema: Option<String>,
    /// How the last one ended.
    pub fim: Option<NfseFinal>,
}

/// The services sold at a counter. A clone is the same.
#[derive(Clone, Debug, Default)]
pub struct Servicos {
    /// What is on the screen.
    pub venda: State<VendaServico>,
}

impl Servicos {
    fn escolher(&self, servico: &Object) {
        let preco = match servico.get("preco") {
            Value::Number(centavos) => Decimal::new(centavos.round() as i64, 2),
            _ => Decimal::ZERO,
        };
        self.venda.update(|venda| {
            venda.servico = Some(texto(servico, "codigo"));
            venda.valor = valor_em_texto(preco);
            venda.descricao = texto(servico, "descricao");
        });
    }

    /// Issues the NFS-e of what is typed in.
    pub fn emitir(&self, session: &Session, service: &NfeService) {
        let venda = self.venda.peek().clone();
        if venda.enviando {
            return;
        }
        let pedido = (|| -> Result<ServicoVendido, String> {
            let codigo = venda.servico.as_deref().ok_or("Escolha o serviço.")?;
            let servico = servicos(session)
                .into_iter()
                .find(|servico| texto(servico, "codigo") == codigo)
                .ok_or("Esse serviço não existe mais.")?;
            let valor =
                ler_valor(&venda.valor).filter(|valor| *valor > Decimal::ZERO).ok_or("Digite o valor.")?;
            let tomador = if venda.documento.trim().is_empty() {
                None
            } else {
                if venda.nome.trim().is_empty() {
                    return Err("Digite o nome de quem toma o serviço.".to_owned());
                }
                Some(Tomador {
                    inscricao: inscricao_de(&venda.documento)?,
                    nome: venda.nome.trim().to_owned(),
                    email: None,
                })
            };
            Ok(ServicoVendido {
                codigo_tributacao_nacional: texto(&servico, "codigo_tributacao_nacional"),
                descricao: if venda.descricao.trim().is_empty() {
                    texto(&servico, "descricao")
                } else {
                    venda.descricao.trim().to_owned()
                },
                valor,
                tomador,
            })
        })();
        let pedido = match pedido {
            Ok(pedido) => pedido,
            Err(why) => return self.venda.update(|venda| venda.problema = Some(why)),
        };
        self.venda.update(|venda| {
            venda.enviando = true;
            venda.problema = None;
            venda.fim = None;
        });
        let aqui = self.clone();
        service.emitir_nfse(session, pedido, move |_, fim| aqui.terminar(fim));
    }

    /// Sends a pending DPS again, the same one.
    pub fn reenviar(&self, session: &Session, service: &NfeService, registro: &Value) {
        if self.venda.peek().enviando {
            return;
        }
        self.venda.update(|venda| {
            venda.enviando = true;
            venda.fim = None;
        });
        let aqui = self.clone();
        service.reenviar_nfse(session, registro, move |_, fim| aqui.terminar(fim));
    }

    fn terminar(&self, fim: NfseFinal) {
        self.venda.update(|venda| {
            if fim.autorizada {
                venda.documento.clear();
                venda.nome.clear();
            }
            venda.enviando = false;
            venda.fim = Some(fim);
        });
    }
}

fn servicos(session: &Session) -> Vec<Object> {
    let space = session.frame().space();
    let mut found = space.borrow().query(SERVICO, &Query::all()).unwrap_or_default();
    found.sort_by_key(|servico| texto(servico, "descricao"));
    found
}

/// The screen of selling a service.
pub(crate) struct TelaServico {
    pub(crate) servicos: Servicos,
    pub(crate) service: NfeService,
    pub(crate) session: Session,
    pub(crate) palavras: Rc<ReportWords>,
}

fn campo(
    rotulo: &'static str,
    venda: &State<VendaServico>,
    get: fn(&VendaServico) -> &String,
    set: fn(&mut VendaServico, String),
) -> AnyView {
    let (read, write) = (venda.clone(), venda.clone());
    Box::new(FormField::new(
        rotulo,
        TextField::new(Binding::new(
            move || read.with(|all| get(all).clone()),
            move |value: String| write.update(|all| set(all, value)),
        )),
    ))
}

impl TelaServico {
    fn resultado(&self, fim: &NfseFinal) -> Vec<AnyView> {
        let mut partes: Vec<AnyView> = Vec::new();
        if fim.autorizada {
            let mut texto = format!("Chave {}", fim.chave);
            if fim.recuperada {
                texto = format!("{texto}. {}", fim.motivo);
            }
            partes.push(Box::new(
                Alert::new(format!("NFS-e {} autorizada", fim.numero_nfse), texto).tone(Tone::Success),
            ));
            let (session, registro, palavras) =
                (self.session.clone(), fim.registro.clone(), self.palavras.clone());
            partes.push(Box::new(Button::new("Imprimir DANFSe").style(ButtonStyle::Primary).on_click(
                move || {
                    let space = session.frame().space();
                    let found = space.borrow().find(NFSE_DOCUMENTO, &registro).ok().flatten();
                    let shown = found
                        .ok_or_else(|| "Essa NFS-e não existe mais.".to_owned())
                        .and_then(|record| mostrar_danfse(&session, &record, &palavras));
                    if let Err(why) = shown {
                        session.show_message(Message::new(why, MessageKind::Error));
                    }
                },
            )));
        } else if fim.pendente {
            partes.push(Box::new(
                Alert::new(
                    format!("DPS {} pendente", fim.numero_dps),
                    format!("{} Envie de novo: vai a mesma DPS, com o mesmo número.", fim.motivo),
                )
                .tone(Tone::Warning),
            ));
        } else {
            partes.push(Box::new(Alert::new("NFS-e não emitida", fim.motivo.clone()).tone(Tone::Danger)));
        }
        partes
    }
}

impl View for TelaServico {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let venda = &self.servicos.venda;
        let lista = servicos(&self.session);
        let atual = venda.with(|all| all.servico.clone());
        if atual.is_none()
            && let Some(primeiro) = lista.first()
        {
            self.servicos.escolher(primeiro);
        }
        let posicao = lista.iter().position(|servico| Some(texto(servico, "codigo")) == atual).unwrap_or(0);
        let (escolhidos, servicos_aqui) = (lista.clone(), self.servicos.clone());
        let seletor = Select::new(
            Binding::new(
                move || posicao,
                move |i: usize| {
                    if let Some(servico) = escolhidos.get(i) {
                        servicos_aqui.escolher(servico);
                    }
                },
            ),
            lista.iter().map(|servico| texto(servico, "descricao")).collect::<Vec<_>>(),
        )
        .label("Serviço");
        let estado = venda.get();
        let mut esquerda: Vec<AnyView> = vec![
            Box::new(Text::new("Venda de serviço").style(TextStyle::Title)),
            Box::new(
                Text::new("Emite a NFS-e do Padrão Nacional. Produtos vão pelo caixa, na NFC-e.")
                    .style(TextStyle::Callout),
            ),
            Box::new(FormField::new("Serviço", seletor)),
            campo("Valor (R$)", venda, |v| &v.valor, |v, t| v.valor = t),
            campo("Descrição na NFS-e", venda, |v| &v.descricao, |v, t| v.descricao = t),
            campo("CPF ou CNPJ do tomador (opcional)", venda, |v| &v.documento, |v, t| v.documento = t),
            campo("Nome do tomador", venda, |v| &v.nome, |v, t| v.nome = t),
        ];
        if lista.is_empty() {
            esquerda.push(Box::new(
                Alert::new("Nenhum serviço", "Cadastre um serviço com o código de tributação nacional.")
                    .tone(Tone::Accent),
            ));
        }
        if let Some(problema) = &estado.problema {
            esquerda.push(Box::new(Alert::new("Falta algo", problema.clone()).tone(Tone::Warning)));
        }
        let (servicos_aqui, session, service) =
            (self.servicos.clone(), self.session.clone(), self.service.clone());
        esquerda.push(Box::new(
            Button::new(if estado.enviando { "Enviando a DPS…" } else { "Emitir NFS-e" })
                .style(ButtonStyle::Primary)
                .full_width()
                .disabled(estado.enviando || lista.is_empty())
                .on_click(move || servicos_aqui.emitir(&session, &service)),
        ));

        let mut direita: Vec<AnyView> = Vec::new();
        if let Some(fim) = &estado.fim {
            direita.extend(self.resultado(fim));
        }
        let pendentes = nfses_pendentes(&*self.session.frame().space().borrow());
        if !pendentes.is_empty() {
            direita.push(Box::new(Text::new("DPS sem resposta certa").style(TextStyle::Headline)));
        }
        for pendente in pendentes {
            let (servicos_aqui, session, service, registro) =
                (self.servicos.clone(), self.session.clone(), self.service.clone(), pendente.get("id"));
            let numero = match pendente.get("numero_dps") {
                Value::Number(numero) => numero as u64,
                _ => 0,
            };
            direita.push(Box::new(
                Button::new(format!("Enviar de novo: DPS {numero}, R$ {}", texto(&pendente, "valor")))
                    .full_width()
                    .disabled(estado.enviando)
                    .on_click(move || servicos_aqui.reenviar(&session, &service, &registro)),
            ));
        }
        cx.child(
            &HStack::new(vec![
                Box::new(VStack::new(esquerda).spacing(12.0).align(Align::Stretch).width(460.0)) as AnyView,
                Box::new(
                    ScrollView::new(vec![
                        Box::new(VStack::new(direita).spacing(12.0).align(Align::Stretch)) as AnyView
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
