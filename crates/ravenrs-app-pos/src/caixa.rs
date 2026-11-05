//! The state of the checkout and what can be done to it. The screens only show this and call it.

use std::rc::Rc;

use ravenrs_app::{Message, MessageKind, Object, Query, Value, drafts, save_draft, take_draft};
use ravenrs_app_nfe::engine::{
    Csosn, Destinatario, DocumentoDestinatario, Endereco, FormaPagamento, Item, Modelo, Rascunho,
};
use ravenrs_app_nfe::{
    CUPOM_LINHA, EmissaoFinal, MODELO_DANFE, NfeService, TextoVenda, linhas_do_documento, renderizar_cupom, uf_de,
};
use ravenrs_app_report::{ReportWords, label_bytes, send_to_printer, stored_reports_for};
use ravenrs_app_ui::Session;
use ravenrs_pos::{Carrinho, Forma, Linha, ler_codigo, ler_valor, troco, valor_em_texto};
use ravenrs_reactive::State;
use ravenrs_report::barcode::is_valid_gtin;
use ravenrs_report_label::{LabelJob, Language, output_for};
use ravenrs_report_viewer::{DocumentViewer, ViewerState};
use ravenrs_widgets::AnyView;
use rust_decimal::Decimal;

use crate::modelo::{CLIENTE, CONFIGURACAO, PRODUTO};

const TIPO_RASCUNHO: &str = "venda";

/// Which screen of the checkout is up.
#[derive(Clone, Debug, PartialEq)]
pub enum Tela {
    /// Scanning products.
    Vendendo,
    /// Taking the payment.
    Pagando,
    /// The document is on its way.
    Enviando,
    /// How it ended.
    Resultado(EmissaoFinal),
}

pub(crate) fn texto(record: &Object, field: &str) -> String {
    match record.get(field) {
        Value::Text(text) => text.trim().to_owned(),
        _ => String::new(),
    }
}

/// A product as a line of a sale.
fn linha_de(produto: &Object, quantidade: Decimal) -> Linha {
    let centavos = match produto.get("preco") {
        Value::Number(centavos) => centavos.round() as i64,
        _ => 0,
    };
    let gtin = texto(produto, "gtin");
    let cest = texto(produto, "cest");
    Linha {
        codigo: texto(produto, "codigo"),
        gtin: (!gtin.is_empty()).then_some(gtin),
        descricao: texto(produto, "descricao"),
        unidade: texto(produto, "unidade"),
        preco: Decimal::new(centavos, 2),
        quantidade,
        ncm: texto(produto, "ncm"),
        cfop: texto(produto, "cfop"),
        cest: (!cest.is_empty()).then_some(cest),
        csosn: texto(produto, "csosn"),
    }
}

fn forma_fiscal(forma: Forma) -> FormaPagamento {
    match forma {
        Forma::Dinheiro => FormaPagamento::Dinheiro,
        Forma::Debito => FormaPagamento::CartaoDebito,
        Forma::Credito => FormaPagamento::CartaoCredito,
        Forma::Pix => FormaPagamento::Pix,
    }
}

/// The document of a sale.
fn rascunho_de(
    carrinho: &Carrinho,
    destinatario: Option<&Destinatario>,
    forma: Forma,
    recebido: Decimal,
    numero: u32,
    serie: u16,
) -> Rascunho {
    let modelo = if destinatario.is_some() { Modelo::Nfe55 } else { Modelo::Nfce65 };
    let mut rascunho = Rascunho::novo(modelo, serie, numero, "VENDA DE MERCADORIA", String::new());
    for linha in &carrinho.linhas {
        rascunho = rascunho.com_item(Item {
            codigo: linha.codigo.clone(),
            gtin: linha.gtin.clone().filter(|gtin| is_valid_gtin(gtin)),
            descricao: linha.descricao.clone(),
            ncm: linha.ncm.clone(),
            cest: linha.cest.clone(),
            cfop: linha.cfop.clone(),
            unidade: linha.unidade.clone(),
            quantidade: linha.quantidade,
            valor_unitario: linha.preco,
            desconto: Decimal::ZERO,
            csosn: if linha.csosn == "500" { Csosn::S500 } else { Csosn::S102 },
            origem: 0,
        });
    }
    rascunho = rascunho.com_pagamento(forma_fiscal(forma), recebido);
    if let Some(destinatario) = destinatario {
        rascunho = rascunho.com_destinatario(destinatario.clone());
    } else if let Some(cliente) = &carrinho.cliente {
        let documento = if cliente.len() == 11 {
            DocumentoDestinatario::Cpf(cliente.clone())
        } else {
            DocumentoDestinatario::Cnpj(cliente.clone())
        };
        rascunho = rascunho.com_destinatario(Destinatario { documento, nome: None, endereco: None });
    }
    if let Some(observacao) = &carrinho.observacao {
        rascunho = rascunho.com_observacao(observacao.clone());
    }
    rascunho
}

/// A checkout. A clone is the same checkout.
#[derive(Clone, Debug)]
pub struct Caixa {
    /// The sale being made.
    pub carrinho: State<Carrinho>,
    /// The screen that is up.
    pub tela: State<Tela>,
    /// What went wrong last, to read at the counter.
    pub problema: State<Option<String>>,
    /// How it is being paid.
    pub forma: State<Forma>,
    /// What was typed as handed over.
    pub recebido: State<String>,
    /// The sale goes out as a NF-e, with a DANFE on a sheet, for a customer on record.
    pub nota_grande: State<bool>,
    /// Issuing.
    pub service: NfeService,
    /// The words of the stored reports.
    pub palavras: Rc<ReportWords>,
}

impl Caixa {
    /// A checkout issuing through this service.
    pub fn new(service: NfeService) -> Self {
        Self {
            carrinho: State::new(Carrinho::default()),
            tela: State::new(Tela::Vendendo),
            problema: State::new(None),
            forma: State::new(Forma::Dinheiro),
            recebido: State::new(String::new()),
            nota_grande: State::new(false),
            service,
            palavras: Rc::new(ReportWords::pt_br()),
        }
    }

    /// Takes in what the scanner or the operator gave.
    pub fn ler(&self, session: &Session, lido: &str) {
        let leitura = match ler_codigo(lido) {
            Ok(leitura) => leitura,
            Err(why) => return self.problema.set(Some(why)),
        };
        let space = session.frame().space();
        let produtos = space.borrow().query(PRODUTO, &Query::all()).unwrap_or_default();
        let achado = produtos.iter().find(|produto| {
            texto(produto, "gtin") == leitura.codigo || texto(produto, "codigo") == leitura.codigo
        });
        match achado {
            Some(produto) => {
                self.problema.set(None);
                let linha = linha_de(produto, leitura.quantidade);
                self.carrinho.update(|carrinho| carrinho.adicionar(linha));
            }
            None => self.problema.set(Some(format!("Produto {} não encontrado.", leitura.codigo))),
        }
    }

    /// What F9 does, by the screen that is up.
    pub fn avancar(&self, session: &Session) {
        match self.tela.get() {
            Tela::Vendendo => {
                if self.carrinho.peek().vazio() {
                    self.problema.set(Some("Passe ao menos um produto.".to_owned()));
                    return;
                }
                self.problema.set(None);
                self.forma.set(Forma::Dinheiro);
                self.recebido.set(valor_em_texto(self.carrinho.peek().total()));
                self.tela.set(Tela::Pagando);
            }
            Tela::Pagando => self.confirmar(session),
            Tela::Enviando => {}
            Tela::Resultado(fim) => {
                if fim.autorizada {
                    self.nova_venda();
                } else {
                    self.tela.set(Tela::Pagando);
                }
            }
        }
    }

    /// Who the note of this sale goes to, for a note of the large kind, or what is missing.
    fn destinatario(&self, session: &Session, carrinho: &Carrinho) -> Result<Destinatario, String> {
        let documento = carrinho.cliente.clone().ok_or("A nota grande precisa de um cliente: use Alt+C.")?;
        let space = session.frame().space();
        let clientes = space.borrow().query(CLIENTE, &Query::all()).unwrap_or_default();
        let cliente = clientes
            .iter()
            .find(|cliente| {
                texto(cliente, "documento").chars().filter(char::is_ascii_alphanumeric).collect::<String>() == documento
            })
            .ok_or_else(|| format!("O cliente {documento} não está cadastrado. A nota grande precisa do endereço."))?;
        Ok(Destinatario {
            documento: if documento.len() == 11 {
                DocumentoDestinatario::Cpf(documento)
            } else {
                DocumentoDestinatario::Cnpj(documento)
            },
            nome: Some(texto(cliente, "nome")),
            endereco: Some(Endereco {
                logradouro: texto(cliente, "logradouro"),
                numero: texto(cliente, "numero"),
                bairro: texto(cliente, "bairro"),
                codigo_municipio: texto(cliente, "codigo_municipio"),
                municipio: texto(cliente, "municipio"),
                uf: uf_de(&texto(cliente, "uf"))?,
                cep: texto(cliente, "cep"),
            }),
        })
    }

    /// Swaps an authorized NFC-e for a NF-e to a customer on record: a note cannot change its model,
    /// so the first one is canceled and the sale issued again. What the law allows for this, and how
    /// long after the sale, is for an accountant to confirm.
    pub fn trocar_por_nota_grande(&self, session: &Session) {
        let Tela::Resultado(fim) = self.tela.get() else { return };
        if !fim.autorizada {
            return;
        }
        let carrinho = self.carrinho.peek().clone();
        if let Err(why) = self.destinatario(session, &carrinho) {
            return self.problema.set(Some(why));
        }
        self.problema.set(None);
        self.tela.set(Tela::Enviando);
        let caixa = self.clone();
        self.service.cancelar(
            session,
            &fim.chave.clone(),
            &fim.protocolo.clone(),
            "Troca a pedido do cliente por NF-e com destinatario identificado",
            move |session, ended| match ended {
                Ok(()) => {
                    caixa.nota_grande.set(true);
                    caixa.confirmar(session);
                }
                Err(why) => {
                    caixa.problema.set(Some(format!("A NFC-e não foi cancelada: {why}")));
                    caixa.tela.set(Tela::Resultado(fim));
                }
            },
        );
    }

    /// What Esc does, by the screen that is up. False when there is nothing to go back from.
    pub fn voltar(&self) -> bool {
        match self.tela.get() {
            Tela::Pagando => {
                self.tela.set(Tela::Vendendo);
                true
            }
            Tela::Enviando => true,
            Tela::Resultado(fim) => {
                if fim.autorizada {
                    self.nova_venda();
                } else {
                    self.tela.set(Tela::Vendendo);
                }
                true
            }
            Tela::Vendendo => false,
        }
    }

    fn nova_venda(&self) {
        self.carrinho.set(Carrinho::default());
        self.problema.set(None);
        self.nota_grande.set(false);
        self.tela.set(Tela::Vendendo);
    }

    /// Takes the payment and issues the document.
    fn confirmar(&self, session: &Session) {
        let carrinho = self.carrinho.peek().clone();
        let forma = *self.forma.peek();
        let total = carrinho.total();
        let Some(recebido) = ler_valor(&self.recebido.peek()) else {
            return self.problema.set(Some("Digite o valor recebido.".to_owned()));
        };
        if let Err(why) = troco(forma, total, recebido) {
            return self.problema.set(Some(why));
        }
        let destinatario = if *self.nota_grande.peek() {
            match self.destinatario(session, &carrinho) {
                Ok(destinatario) => Some(destinatario),
                Err(why) => {
                    self.problema.set(Some(why));
                    self.tela.set(Tela::Pagando);
                    return;
                }
            }
        } else {
            None
        };
        self.problema.set(None);
        self.tela.set(Tela::Enviando);
        let venda = TextoVenda {
            pagamento: forma.nome().to_owned(),
            recebido,
            consumidor: carrinho.cliente.as_ref().filter(|_| destinatario.is_none()).map_or_else(
                || "CONSUMIDOR NÃO IDENTIFICADO".to_owned(),
                |cliente| format!("CONSUMIDOR {cliente}"),
            ),
        };
        let caixa = self.clone();
        self.service.emitir(
            session,
            move |numero, serie| rascunho_de(&carrinho, destinatario.as_ref(), forma, recebido, numero, serie),
            venda,
            move |session, fim| {
                let autorizada = fim.autorizada;
                let chave = fim.chave.clone();
                caixa.tela.set(Tela::Resultado(fim));
                if autorizada && !*caixa.nota_grande.peek() && !configuracao(session, "impressora_cupom").is_empty() {
                    caixa.imprimir_cupom(session, &chave);
                }
            },
        );
    }

    /// Takes out the last line.
    pub fn cancelar_ultimo(&self) {
        if *self.tela.peek() == Tela::Vendendo {
            self.carrinho.update(|carrinho| {
                carrinho.cancelar_ultimo();
            });
        }
    }

    /// Throws the sale away.
    pub fn cancelar_venda(&self) {
        self.nova_venda();
    }

    /// Keeps the sale to pick up later, and starts another.
    pub fn guardar(&self, session: &Session) -> Result<(), String> {
        let carrinho = self.carrinho.peek().clone();
        if carrinho.vazio() {
            return Err("Não há venda para guardar.".to_owned());
        }
        let titulo = format!("{} itens, R$ {}", carrinho.linhas.len(), valor_em_texto(carrinho.total()));
        let space = session.frame().space();
        let mut space = space.borrow_mut();
        save_draft(&mut *space, TIPO_RASCUNHO, &titulo, &carrinho.para_texto()).map_err(|e| e.to_string())?;
        space.commit().map_err(|e| e.to_string())?;
        drop(space);
        self.nova_venda();
        Ok(())
    }

    /// The sales kept, with what tells them apart.
    pub fn pendentes(&self, session: &Session) -> Vec<(Value, String)> {
        let space = session.frame().space();
        let kept = drafts(&*space.borrow(), TIPO_RASCUNHO).unwrap_or_default();
        kept.into_iter().map(|draft| (draft.id, draft.title)).collect()
    }

    /// Picks a kept sale up again.
    pub fn retomar(&self, session: &Session, id: &Value) -> Result<(), String> {
        if !self.carrinho.peek().vazio() {
            return Err("Termine ou guarde a venda aberta antes.".to_owned());
        }
        let space = session.frame().space();
        let mut space = space.borrow_mut();
        let draft = take_draft(&mut *space, id).map_err(|e| e.to_string())?.ok_or("Essa venda já saiu.")?;
        let carrinho = Carrinho::de_texto(&draft.payload)?;
        space.commit().map_err(|e| e.to_string())?;
        drop(space);
        self.carrinho.set(carrinho);
        self.tela.set(Tela::Vendendo);
        Ok(())
    }

    /// The receipt of a document: to the printer that is set, or on the screen, where it also goes
    /// out as a PDF.
    pub fn imprimir_cupom(&self, session: &Session, chave: &str) {
        let tell = |text: String| session.show_message(Message::new(text, MessageKind::Error));
        let linhas = linhas_do_documento(session, chave);
        if linhas.is_empty() {
            return tell("Essa nota não tem cupom: ela não foi autorizada.".to_owned());
        }
        // The note decides the paper: a sheet for a NF-e, the roll for a NFC-e.
        let grande = linhas.first().is_some_and(|linha| texto(linha, "modelo") == "55");
        let modelos = stored_reports_for(session, CUPOM_LINHA);
        let modelo = modelos.iter().find(|modelo| (texto(modelo, "name") == MODELO_DANFE) == grande);
        let Some(modelo) = modelo else {
            return tell("Não há modelo para esse documento.".to_owned());
        };
        let documento = match renderizar_cupom(session, modelo, &linhas, &self.palavras) {
            Ok(documento) => documento,
            Err(why) => return tell(why),
        };
        let conexao = configuracao(session, "impressora_cupom");
        if grande || conexao.is_empty() {
            let shown = Rc::new(documento);
            let state = ViewerState::new();
            session.present("Cupom", move || Box::new(DocumentViewer::new(shown.clone(), &state)) as AnyView);
            return;
        }
        let job = LabelJob::new(Language::EscPos, 203);
        let bytes = match label_bytes(&documento, &job) {
            Ok(bytes) => bytes,
            Err(why) => return tell(why),
        };
        match output_for(&conexao) {
            Ok(output) => send_to_printer(session, "Imprimindo o cupom", bytes, output, |session, sent| {
                if let Err(why) = sent {
                    session.show_message(Message::new(why, MessageKind::Error));
                }
            }),
            Err(why) => tell(why.to_string()),
        }
    }
}

/// A setting of the checkout, or empty.
pub(crate) fn configuracao(session: &Session, campo: &str) -> String {
    let space = session.frame().space();
    let found = space.borrow().query(CONFIGURACAO, &Query::all()).unwrap_or_default();
    found.first().map_or_else(String::new, |record| texto(record, campo))
}
