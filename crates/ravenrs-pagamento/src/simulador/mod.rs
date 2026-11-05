//! A provider in the style of MSW: the application does not know it was intercepted.
//!
//! With no handler of yours, it behaves as a small real provider with a ledger: it approves,
//! holds the approval until the third leg, undoes, refunds and answers queries about all that.
//! It is fully deterministic: no clock, no randomness, numbers from counters.
//!
//! | MSW | Here |
//! |---|---|
//! | `setupServer(...handlers)` | [`SimuladorProvedor::com_handlers`] |
//! | `http.post(url, resolver)` | [`iniciar`], [`continuar`], [`confirmar`], [`desfazer`], [`cancelar`], [`estornar`], [`consultar`], [`pendencias`] |
//! | `server.use(handler)` | [`SimuladorProvedor::use_handler`] |
//! | `{ once: true }` | [`SimuladorProvedor::use_once`] |
//! | `server.resetHandlers()` | [`SimuladorProvedor::reset_handlers`] |
//! | `onUnhandledRequest` | [`SimuladorProvedor::on_unhandled`] |
//!
//! ```
//! use ravenrs_pagamento::simulador::{SimuladorProvedor, cenarios, iniciar};
//!
//! let simulador = SimuladorProvedor::new();
//! // The next payment is refused; the one after it follows the default rules.
//! simulador.use_once(iniciar(cenarios::negado));
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::dominio::{
    Aprovacao, Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, ProvedorId,
    ReferenciaTransacao, Vias,
};
use crate::fiscal::DadosFiscaisPagamento;
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};

pub mod cenarios;

/// The start of every text and code this simulator invents, so a receipt never looks real.
pub const PREFIXO: &str = "[SIMULADOR] ";

/// Which operation of the provider a request is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operacao {
    /// [`ProvedorPagamento::iniciar`].
    Iniciar,
    /// [`ProvedorPagamento::continuar`].
    Continuar,
    /// [`ProvedorPagamento::confirmar`].
    Confirmar,
    /// [`ProvedorPagamento::desfazer`].
    Desfazer,
    /// [`ProvedorPagamento::cancelar`].
    Cancelar,
    /// [`ProvedorPagamento::estornar`].
    Estornar,
    /// [`ProvedorPagamento::consultar`].
    Consultar,
    /// [`ProvedorPagamento::pendencias`].
    Pendencias,
}

/// A request as it reached the simulator, also kept for [`SimuladorProvedor::requests`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequisicaoSimulada {
    /// The position among all requests, from 1.
    pub sequencia: u64,
    /// The operation.
    pub operacao: Operacao,
    /// The payment, when the operation names one.
    pub id: Option<IdPagamento>,
    /// The request, in [`Operacao::Iniciar`], or the one the ledger holds for this id.
    pub pedido: Option<PedidoPagamento>,
    /// What the operator sent, in [`Operacao::Continuar`].
    pub entrada: Option<EntradaOperador>,
    /// The reference, in the third leg and in refunds.
    pub referencia: Option<ReferenciaTransacao>,
}

impl RequisicaoSimulada {
    /// The approval this simulator gives by default for this request.
    #[must_use]
    pub fn aprovacao(&self) -> Aprovacao {
        aprovacao_padrao(self.sequencia, self.pedido.as_ref(), self.id.as_ref())
    }

    /// An answer that approves.
    #[must_use]
    pub fn aprovar(&self) -> Resposta {
        Resposta::Passo(Passo::Aprovado(Box::new(self.aprovacao())))
    }

    /// An answer that approves after changing the default approval.
    #[must_use]
    pub fn aprovar_com(&self, ajuste: impl FnOnce(&mut Aprovacao)) -> Resposta {
        let mut aprovacao = self.aprovacao();
        ajuste(&mut aprovacao);
        Resposta::Passo(Passo::Aprovado(Box::new(aprovacao)))
    }
}

/// What a handler decides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resposta {
    /// For `iniciar` and `continuar`: this step.
    Passo(Passo),
    /// For the third leg and refunds: done. The ledger follows.
    Feito,
    /// For `cancelar`: this outcome.
    Cancelamento(Cancelamento),
    /// For `consultar`: this situation.
    Situacao(SituacaoProvedor),
    /// For `pendencias`: this list.
    Pendencias(Vec<ReferenciaTransacao>),
    /// Fails, and nothing happened at the provider.
    Erro(ErroProvedor),
    /// Does what the default rules do, but the caller only sees this failure: the answer was lost.
    PerdeuResposta(ErroProvedor),
    /// Leaves it to the next handler, and in the end to the default rules.
    Proximo,
}

/// A rule that answers one kind of request.
#[derive(Clone)]
pub struct Handler {
    operacao: Operacao,
    responder: Arc<dyn Fn(&RequisicaoSimulada) -> Resposta + Send + Sync>,
}

impl fmt::Debug for Handler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handler").field("operacao", &self.operacao).finish_non_exhaustive()
    }
}

fn handler(
    operacao: Operacao,
    responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static,
) -> Handler {
    Handler { operacao, responder: Arc::new(responder) }
}

/// A handler for [`Operacao::Iniciar`].
pub fn iniciar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Iniciar, responder)
}
/// A handler for [`Operacao::Continuar`].
pub fn continuar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Continuar, responder)
}
/// A handler for [`Operacao::Confirmar`].
pub fn confirmar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Confirmar, responder)
}
/// A handler for [`Operacao::Desfazer`].
pub fn desfazer(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Desfazer, responder)
}
/// A handler for [`Operacao::Cancelar`].
pub fn cancelar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Cancelar, responder)
}
/// A handler for [`Operacao::Estornar`].
pub fn estornar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Estornar, responder)
}
/// A handler for [`Operacao::Consultar`].
pub fn consultar(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Consultar, responder)
}
/// A handler for [`Operacao::Pendencias`].
pub fn pendencias(responder: impl Fn(&RequisicaoSimulada) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Operacao::Pendencias, responder)
}

/// What to do with a request no handler took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnUnhandled {
    /// Apply the default rules.
    Padrao,
    /// Fail it, for tests that must state every step.
    Erro,
}

/// Where a payment is in the simulated ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SituacaoRazao {
    /// Started; the flow is still running.
    EmCurso,
    /// Approved; waiting for the third leg, or captured when there is none.
    Pendente,
    /// Refused.
    Negada,
    /// Abandoned.
    Cancelada,
    /// Confirmed.
    Confirmada,
    /// Undone.
    Desfeita,
    /// Paid back.
    Estornada,
}

#[derive(Clone, Debug)]
struct Lancado {
    situacao: SituacaoRazao,
    pedido: Option<PedidoPagamento>,
    aprovacao: Option<Aprovacao>,
}

#[derive(Debug)]
struct Interno {
    iniciais: Vec<Handler>,
    handlers: Vec<(Handler, bool)>,
    capacidades: Capacidades,
    on_unhandled: OnUnhandled,
    requests: Vec<RequisicaoSimulada>,
    razao: BTreeMap<IdPagamento, Lancado>,
    sequencia: u64,
}

/// A provider that lives in memory. Its clones share the same ledger and handlers.
#[derive(Clone, Debug)]
pub struct SimuladorProvedor(Arc<Mutex<Interno>>);

impl Default for SimuladorProvedor {
    fn default() -> Self {
        Self::new()
    }
}

/// The capabilities of a provider with a third leg, which is what most of the flow exercises.
pub const CAPACIDADES_PADRAO: Capacidades = Capacidades {
    terceira_perna: true,
    estorna_por_api: true,
    estorno_parcial: false,
    cancela_em_andamento: true,
    pergunta_ao_operador: true,
    imprime_no_terminal: false,
    exige_servidor: false,
    testavel_sem_hardware: true,
};

impl SimuladorProvedor {
    /// A simulator with no handlers.
    #[must_use]
    pub fn new() -> Self {
        Self::com_handlers(Vec::new())
    }

    /// A simulator that starts with these handlers; [`reset_handlers`](Self::reset_handlers) returns to them.
    #[must_use]
    pub fn com_handlers(handlers: Vec<Handler>) -> Self {
        Self(Arc::new(Mutex::new(Interno {
            handlers: handlers.iter().cloned().map(|h| (h, false)).collect(),
            iniciais: handlers,
            capacidades: CAPACIDADES_PADRAO,
            on_unhandled: OnUnhandled::Padrao,
            requests: Vec::new(),
            razao: BTreeMap::new(),
            sequencia: 0,
        })))
    }

    /// Behaves as a provider with these capabilities, for example with no third leg.
    #[must_use]
    pub fn com_capacidades(self, capacidades: Capacidades) -> Self {
        self.travar().capacidades = capacidades;
        self
    }

    /// Adds a handler in front of the others.
    pub fn use_handler(&self, handler: Handler) {
        self.travar().handlers.insert(0, (handler, false));
    }

    /// Adds a handler in front of the others that answers only one request.
    pub fn use_once(&self, handler: Handler) {
        self.travar().handlers.insert(0, (handler, true));
    }

    /// Back to the handlers it was created with.
    pub fn reset_handlers(&self) {
        let mut interno = self.travar();
        interno.handlers = interno.iniciais.iter().cloned().map(|h| (h, false)).collect();
    }

    /// What to do with a request no handler took.
    pub fn on_unhandled(&self, modo: OnUnhandled) {
        self.travar().on_unhandled = modo;
    }

    /// All requests so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<RequisicaoSimulada> {
        self.travar().requests.clone()
    }

    /// The requests of one operation, in order.
    #[must_use]
    pub fn requests_de(&self, operacao: Operacao) -> Vec<RequisicaoSimulada> {
        self.travar().requests.iter().filter(|r| r.operacao == operacao).cloned().collect()
    }

    /// Where a payment is in the ledger.
    #[must_use]
    pub fn situacao(&self, id: &IdPagamento) -> Option<SituacaoRazao> {
        self.travar().razao.get(id).map(|l| l.situacao.clone())
    }

    /// Plants an approval left open by an earlier session, unknown to any journal.
    pub fn semear_pendente_de_sessao_anterior(&self, id: &IdPagamento, valor_centavos: u64) {
        let mut interno = self.travar();
        interno.sequencia += 1;
        let mut aprovacao = aprovacao_padrao(interno.sequencia, None, Some(id));
        aprovacao.referencia.valor_centavos = valor_centavos;
        interno.razao.insert(
            id.clone(),
            Lancado { situacao: SituacaoRazao::Pendente, pedido: None, aprovacao: Some(aprovacao) },
        );
    }

    fn travar(&self) -> MutexGuard<'_, Interno> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn atender(
        &self,
        operacao: Operacao,
        id: Option<&IdPagamento>,
        pedido: Option<&PedidoPagamento>,
        entrada: Option<EntradaOperador>,
        referencia: Option<&ReferenciaTransacao>,
    ) -> Result<Resposta, ErroProvedor> {
        let (requisicao, escolhidos, modo) = {
            let mut interno = self.travar();
            interno.sequencia += 1;
            let id = id
                .cloned()
                .or_else(|| pedido.map(|p| p.id.clone()))
                .or_else(|| referencia.and_then(|r| r.id.clone()));
            let pedido = pedido
                .cloned()
                .or_else(|| id.as_ref().and_then(|id| interno.razao.get(id)).and_then(|l| l.pedido.clone()));
            let requisicao = RequisicaoSimulada {
                sequencia: interno.sequencia,
                operacao,
                id,
                pedido,
                entrada,
                referencia: referencia.cloned(),
            };
            interno.requests.push(requisicao.clone());
            let escolhidos: Vec<(usize, Handler)> = interno
                .handlers
                .iter()
                .enumerate()
                .filter(|(_, (h, _))| h.operacao == operacao)
                .map(|(indice, (h, _))| (indice, h.clone()))
                .collect();
            (requisicao, escolhidos, interno.on_unhandled)
        };
        // Handlers run with the lock released, so they may look at the simulator.
        for (indice, escolhido) in escolhidos {
            let resposta = (escolhido.responder)(&requisicao);
            if resposta == Resposta::Proximo {
                continue;
            }
            let mut interno = self.travar();
            if interno.handlers.get(indice).is_some_and(|(_, unico)| *unico) {
                interno.handlers.remove(indice);
            }
            drop(interno);
            return self.aplicar(&requisicao, resposta);
        }
        if modo == OnUnhandled::Erro {
            return Err(ErroProvedor::Protocolo(format!("{PREFIXO}sem handler para {operacao:?}")));
        }
        let padrao = self.padrao(&requisicao)?;
        self.aplicar(&requisicao, padrao)
    }

    /// The default answers, which read the ledger and do not change it.
    fn padrao(&self, requisicao: &RequisicaoSimulada) -> Result<Resposta, ErroProvedor> {
        let interno = self.travar();
        let lancado = requisicao.id.as_ref().and_then(|id| interno.razao.get(id));
        let resposta = match requisicao.operacao {
            Operacao::Iniciar => requisicao.aprovar(),
            Operacao::Continuar => match requisicao.entrada {
                Some(EntradaOperador::Cancelar) => {
                    Resposta::Passo(Passo::Cancelado { motivo: format!("{PREFIXO}cancelado pelo operador") })
                }
                _ => requisicao.aprovar(),
            },
            Operacao::Confirmar | Operacao::Desfazer | Operacao::Estornar => {
                match lancado.map(|l| &l.situacao) {
                    Some(SituacaoRazao::Negada | SituacaoRazao::Cancelada) => {
                        return Err(ErroProvedor::Recusado {
                            codigo: "SIM-ESTADO".into(),
                            motivo: format!("{PREFIXO}transação não aprovada"),
                        });
                    }
                    _ => Resposta::Feito,
                }
            }
            Operacao::Cancelar => Resposta::Cancelamento(Cancelamento::Feito),
            Operacao::Consultar => Resposta::Situacao(match lancado {
                None => SituacaoProvedor::Desconhecida,
                Some(l) => situacao_externa(l),
            }),
            Operacao::Pendencias => Resposta::Pendencias(
                interno
                    .razao
                    .iter()
                    .filter(|(_, l)| {
                        l.situacao == SituacaoRazao::Pendente && interno.capacidades.terceira_perna
                    })
                    .filter_map(|(id, l)| {
                        l.aprovacao.as_ref().map(|a| {
                            let mut referencia = a.referencia.clone();
                            referencia.id = Some(id.clone());
                            referencia
                        })
                    })
                    .collect(),
            ),
        };
        Ok(resposta)
    }

    /// Brings the ledger along with the answer and turns it into a result.
    fn aplicar(&self, requisicao: &RequisicaoSimulada, resposta: Resposta) -> Result<Resposta, ErroProvedor> {
        match resposta {
            // It did not arrive: the ledger stays as it was.
            Resposta::Erro(erro) => Err(erro),
            Resposta::PerdeuResposta(erro) => {
                let padrao = self.padrao(requisicao)?;
                self.lancar(requisicao, &padrao);
                Err(erro)
            }
            outra => {
                self.lancar(requisicao, &outra);
                Ok(outra)
            }
        }
    }

    fn lancar(&self, requisicao: &RequisicaoSimulada, resposta: &Resposta) {
        let Some(id) = requisicao.id.clone() else { return };
        let mut interno = self.travar();
        let entrada = interno.razao.entry(id).or_insert_with(|| Lancado {
            situacao: SituacaoRazao::EmCurso,
            pedido: requisicao.pedido.clone(),
            aprovacao: None,
        });
        match (requisicao.operacao, resposta) {
            (Operacao::Iniciar | Operacao::Continuar, Resposta::Passo(Passo::Aprovado(aprovacao))) => {
                entrada.situacao = SituacaoRazao::Pendente;
                entrada.aprovacao = Some((**aprovacao).clone());
            }
            (Operacao::Iniciar | Operacao::Continuar, Resposta::Passo(Passo::Negado { .. })) => {
                entrada.situacao = SituacaoRazao::Negada;
            }
            (Operacao::Iniciar | Operacao::Continuar, Resposta::Passo(Passo::Cancelado { .. })) => {
                entrada.situacao = SituacaoRazao::Cancelada;
            }
            (Operacao::Confirmar, Resposta::Feito) => entrada.situacao = SituacaoRazao::Confirmada,
            (Operacao::Desfazer, Resposta::Feito) => entrada.situacao = SituacaoRazao::Desfeita,
            (Operacao::Estornar, Resposta::Feito) => entrada.situacao = SituacaoRazao::Estornada,
            (Operacao::Cancelar, Resposta::Cancelamento(Cancelamento::Feito)) => {
                entrada.situacao = SituacaoRazao::Cancelada;
            }
            _ => {}
        }
    }
}

fn situacao_externa(lancado: &Lancado) -> SituacaoProvedor {
    let aprovacao = || Box::new(lancado.aprovacao.clone().unwrap_or_else(|| aprovacao_padrao(0, None, None)));
    match lancado.situacao {
        SituacaoRazao::EmCurso => SituacaoProvedor::EmAndamento,
        SituacaoRazao::Pendente => SituacaoProvedor::Aprovada(aprovacao()),
        SituacaoRazao::Negada => {
            SituacaoProvedor::Negada { codigo: "SIM-05".into(), motivo: format!("{PREFIXO}negada") }
        }
        SituacaoRazao::Cancelada => SituacaoProvedor::Cancelada,
        SituacaoRazao::Confirmada => SituacaoProvedor::Confirmada(aprovacao()),
        SituacaoRazao::Desfeita => SituacaoProvedor::Desfeita,
        SituacaoRazao::Estornada => SituacaoProvedor::Estornada,
    }
}

fn aprovacao_padrao(sequencia: u64, pedido: Option<&PedidoPagamento>, id: Option<&IdPagamento>) -> Aprovacao {
    let mut dados_fiscais = DadosFiscaisPagamento::integrado();
    // A valid CNPJ that belongs to no payment institution, used across the tests of this workspace.
    dados_fiscais.cnpj = Some("11222333000181".into());
    dados_fiscais.t_band = Some("01".into());
    dados_fiscais.c_aut = Some(format!("SIM{sequencia:06}"));
    dados_fiscais.id_term_pag = Some("SIMULADOR-01".into());
    Aprovacao {
        referencia: ReferenciaTransacao {
            id: id.cloned().or_else(|| pedido.map(|p| p.id.clone())),
            rede: Some("SIMULADOR".into()),
            nsu: Some(format!("{sequencia:012}")),
            controle: Some(format!("CTRL{sequencia:06}")),
            valor_centavos: pedido.map_or(0, |p| p.valor_centavos),
            extras: BTreeMap::new(),
        },
        dados_fiscais,
        vias: Vias {
            cliente: vec![format!("{PREFIXO}VIA DO CLIENTE"), "SEM VALOR FISCAL OU FINANCEIRO".into()],
            estabelecimento: vec![format!("{PREFIXO}VIA DA LOJA"), "SEM VALOR FISCAL OU FINANCEIRO".into()],
            unica: Vec::new(),
            impressas_no_terminal: false,
        },
        mensagem_operador: Some(format!("{PREFIXO}APROVADA")),
        tipo_efetivo: pedido.map(|p| p.tipo),
        avisos: Vec::new(),
    }
}

fn so_passo(resposta: Resposta) -> Result<Passo, ErroProvedor> {
    match resposta {
        Resposta::Passo(passo) => Ok(passo),
        outra => Err(ErroProvedor::Protocolo(format!("{PREFIXO}resposta sem passo: {outra:?}"))),
    }
}

fn so_feito(resposta: Resposta) -> Result<(), ErroProvedor> {
    match resposta {
        Resposta::Feito => Ok(()),
        outra => Err(ErroProvedor::Protocolo(format!("{PREFIXO}resposta inesperada: {outra:?}"))),
    }
}

impl ProvedorPagamento for SimuladorProvedor {
    fn id(&self) -> ProvedorId {
        ProvedorId::Simulador
    }

    fn capacidades(&self) -> Capacidades {
        self.travar().capacidades
    }

    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        so_passo(self.atender(Operacao::Iniciar, None, Some(pedido), None, None)?)
    }

    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        so_passo(self.atender(Operacao::Continuar, Some(id), None, Some(entrada), None)?)
    }

    fn confirmar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        so_feito(self.atender(Operacao::Confirmar, None, None, None, Some(referencia))?)
    }

    fn desfazer(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        so_feito(self.atender(Operacao::Desfazer, None, None, None, Some(referencia))?)
    }

    fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        match self.atender(Operacao::Cancelar, Some(id), None, None, None)? {
            Resposta::Cancelamento(cancelamento) => Ok(cancelamento),
            outra => Err(ErroProvedor::Protocolo(format!("{PREFIXO}resposta inesperada: {outra:?}"))),
        }
    }

    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        so_feito(self.atender(Operacao::Estornar, None, None, None, Some(referencia))?)
    }

    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        match self.atender(Operacao::Consultar, Some(id), None, None, None)? {
            Resposta::Situacao(situacao) => Ok(situacao),
            outra => Err(ErroProvedor::Protocolo(format!("{PREFIXO}resposta inesperada: {outra:?}"))),
        }
    }

    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        match self.atender(Operacao::Pendencias, None, None, None, None)? {
            Resposta::Pendencias(lista) => Ok(lista),
            outra => Err(ErroProvedor::Protocolo(format!("{PREFIXO}resposta inesperada: {outra:?}"))),
        }
    }
}
