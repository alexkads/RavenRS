//! The one door for an application: it holds a provider and a journal and keeps them honest.

use std::collections::BTreeMap;
use std::fmt;

use crate::diario::{Acao, Conteudo, Diario, ErroDiario, Lancamento, Registro, aplicar};
use crate::dominio::{
    Aprovacao, Capacidades, EntradaOperador, ErroDominio, Estado, Evento, IdPagamento, Passo,
    PedidoPagamento, ReferenciaTransacao, transitar,
};
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};

/// Why an operation did not happen, or did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroPagamento {
    /// The journal failed. Whatever came after it was not attempted.
    Diario(ErroDiario),
    /// The provider failed. If [`ErroProvedor::incerto`], the outcome is unknown.
    Provedor(ErroProvedor),
    /// The state machine refused it.
    Dominio(ErroDominio),
    /// This id is already in the journal, and charging twice is what this refuses.
    JaExiste(IdPagamento),
    /// This id is not in the journal.
    NaoEncontrado(IdPagamento),
    /// This provider only refunds on its own terminal or portal. Do it there and call
    /// [`Pagamentos::registrar_estorno_externo`].
    EstornoManual,
}

impl fmt::Display for ErroPagamento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diario(erro) => erro.fmt(f),
            Self::Provedor(erro) => erro.fmt(f),
            Self::Dominio(erro) => erro.fmt(f),
            Self::JaExiste(id) => write!(f, "o pagamento {id} já existe"),
            Self::NaoEncontrado(id) => write!(f, "o pagamento {id} não está no diário"),
            Self::EstornoManual => f.write_str("este provedor só estorna no terminal ou no portal dele"),
        }
    }
}

impl std::error::Error for ErroPagamento {}

impl From<ErroDiario> for ErroPagamento {
    fn from(erro: ErroDiario) -> Self {
        Self::Diario(erro)
    }
}

impl From<ErroDominio> for ErroPagamento {
    fn from(erro: ErroDominio) -> Self {
        Self::Dominio(erro)
    }
}

impl From<ErroProvedor> for ErroPagamento {
    fn from(erro: ErroProvedor) -> Self {
        Self::Provedor(erro)
    }
}

/// How the receipt and the fiscal document went, which decides the fate of an approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impressao {
    /// The sale finished: fiscal document issued, receipt delivered.
    Concluida,
    /// There is no paper, or the fiscal document failed: the sale did not finish.
    Falhou,
}

/// What one line of [`Pagamentos::recuperar`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Desfecho {
    /// It never reached the provider, so it was closed here.
    CanceladoSemEnvio,
    /// The decision to confirm was already written, so it was confirmed.
    Confirmado,
    /// The sale had not finished, so the approval was undone.
    Desfeito,
    /// Money was captured, there is no third leg, and the sale did not finish. The application
    /// decides: finish the sale with `concluir`, or pay back with `estornar`.
    CobradoSemConclusao,
    /// The provider still has it in progress and it could not be stopped. Ask again later.
    EmAndamento,
    /// The provider cannot say what happened, and money may be moving. Someone must look at the
    /// terminal, and then call [`Pagamentos::abandonar`] or settle it with `consultar`.
    Indeterminado,
    /// The provider already knew the end of the story, and the journal now agrees.
    Reconciliado,
    /// The provider held something open that this journal does not explain, and it was undone.
    PendenciaDesfeita,
    /// The provider held a confirmed sale open, and it was confirmed again.
    PendenciaConfirmada,
    /// It could not be solved now. Call `recuperar` again.
    Falhou(String),
}

/// One thing that [`Pagamentos::recuperar`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recuperacao {
    /// The payment, when the provider's leftover names one.
    pub id: Option<IdPagamento>,
    /// The state in the journal before, if it knew this payment.
    pub antes: Option<Estado>,
    /// The state in the journal after, if it knows this payment.
    pub depois: Option<Estado>,
    /// What was done.
    pub desfecho: Desfecho,
}

/// Payments through one provider and one journal.
///
/// Each line goes to the journal before its effect. An intention is written before the provider
/// is called, and a transition before the state changes in memory. If the journal fails, the call
/// stops there with an error. Use one journal per provider.
#[derive(Debug)]
pub struct Pagamentos<P, D> {
    provedor: P,
    diario: D,
    registros: BTreeMap<IdPagamento, Registro>,
    proximo_seq: u64,
}

impl<P: ProvedorPagamento, D: Diario> Pagamentos<P, D> {
    /// Reads the journal back. Call [`recuperar`](Self::recuperar) before taking new payments.
    pub fn new(mut provedor: P, diario: D) -> Result<Self, ErroDiario> {
        let mut registros = BTreeMap::new();
        let mut ultimo = 0;
        for lancamento in diario.ler()? {
            aplicar(&mut registros, &lancamento)?;
            ultimo = ultimo.max(lancamento.seq);
        }
        for (id, registro) in &registros {
            if let Some(rastro) = &registro.rastro {
                provedor.restaurar_rastro(id, rastro);
            }
        }
        Ok(Self { provedor, diario, registros, proximo_seq: ultimo + 1 })
    }

    /// What the provider can do.
    #[must_use]
    pub fn capacidades(&self) -> Capacidades {
        self.provedor.capacidades()
    }

    /// What the journal knows about a payment.
    #[must_use]
    pub fn registro(&self, id: &IdPagamento) -> Option<&Registro> {
        self.registros.get(id)
    }

    /// All payments in the journal, by id.
    pub fn registros(&self) -> impl Iterator<Item = (&IdPagamento, &Registro)> {
        self.registros.iter()
    }

    /// The provider, for what is specific to it.
    pub fn provedor_mut(&mut self) -> &mut P {
        &mut self.provedor
    }

    /// Starts a payment. An id seen before is refused, even if that payment failed.
    pub fn iniciar(&mut self, pedido: PedidoPagamento) -> Result<Passo, ErroPagamento> {
        let id = pedido.id.clone();
        if self.registros.contains_key(&id) {
            return Err(ErroPagamento::JaExiste(id));
        }
        let provedor = self.provedor.id();
        self.lancar(&id, Conteudo::Aberto { pedido: pedido.clone(), provedor })?;
        self.lancar(&id, Conteudo::Intencao { acao: Acao::Iniciar })?;
        self.transicao(&id, Evento::Enviar, None, None)?;
        let resultado = self.provedor.iniciar(&pedido);
        self.guardar_rastro(&id)?;
        self.absorver(&id, resultado)
    }

    /// Goes on with a payment in progress. What the operator typed never reaches the journal.
    pub fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroPagamento> {
        let estado = self.estado(id)?;
        if estado != Estado::Aguardando {
            return Err(ErroDominio::TransicaoInvalida { estado, evento: Evento::Aprovar }.into());
        }
        let resultado = self.provedor.continuar(id, entrada);
        self.guardar_rastro(id)?;
        self.absorver(id, resultado)
    }

    /// Ends an approved payment: confirms if the sale finished, and otherwise undoes it.
    ///
    /// With no third leg, a failure cannot be undone, so the state stays [`Estado::Aprovado`]. The
    /// application then tries again or calls [`estornar`](Self::estornar). Check the returned state.
    pub fn concluir(&mut self, id: &IdPagamento, impressao: Impressao) -> Result<Estado, ErroPagamento> {
        match impressao {
            Impressao::Concluida => self.confirmar(id),
            Impressao::Falhou if self.provedor.capacidades().terceira_perna => self.desfazer(id),
            Impressao::Falhou => self.estado(id),
        }
    }

    /// Confirms an approved payment. If the provider is unreachable, the decision stays written,
    /// so calling this again or [`recuperar`](Self::recuperar) finishes it.
    pub fn confirmar(&mut self, id: &IdPagamento) -> Result<Estado, ErroPagamento> {
        let referencia = self.aprovada(id)?;
        self.lancar(id, Conteudo::Intencao { acao: Acao::Confirmar })?;
        if self.provedor.capacidades().terceira_perna {
            self.provedor.confirmar(&referencia)?;
        }
        self.transicao(id, Evento::Confirmar, None, None)
    }

    /// Undoes an approved payment. With no third leg this is refused: use a refund.
    pub fn desfazer(&mut self, id: &IdPagamento) -> Result<Estado, ErroPagamento> {
        let referencia = self.aprovada(id)?;
        if !self.provedor.capacidades().terceira_perna {
            return Err(ErroProvedor::SemSuporte("sem terceira perna, só resta o estorno".into()).into());
        }
        self.lancar(id, Conteudo::Intencao { acao: Acao::Desfazer })?;
        self.provedor.desfazer(&referencia)?;
        self.transicao(id, Evento::Desfazer, None, None)
    }

    /// Abandons a payment in progress. With [`Cancelamento::Solicitado`], the state only changes
    /// when `continuar` or `consultar` brings the answer, and it may even be an approval.
    pub fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroPagamento> {
        match self.estado(id)? {
            Estado::Criado => {
                self.transicao(id, Evento::Cancelar, None, None)?;
                Ok(Cancelamento::Feito)
            }
            Estado::Aguardando => {
                self.lancar(id, Conteudo::Intencao { acao: Acao::Cancelar })?;
                let cancelamento = self.provedor.cancelar(id)?;
                if cancelamento == Cancelamento::Feito {
                    self.transicao(id, Evento::Cancelar, None, None)?;
                }
                Ok(cancelamento)
            }
            estado => Err(ErroDominio::TransicaoInvalida { estado, evento: Evento::Cancelar }.into()),
        }
    }

    /// Pays back a confirmed sale, or an approval with no third leg.
    pub fn estornar(&mut self, id: &IdPagamento) -> Result<Estado, ErroPagamento> {
        let estado = self.estado(id)?;
        transitar(estado, Evento::Estornar)?;
        let capacidades = self.provedor.capacidades();
        if estado == Estado::Aprovado && capacidades.terceira_perna {
            return Err(ErroDominio::TransicaoInvalida { estado, evento: Evento::Estornar }.into());
        }
        if !capacidades.estorna_por_api {
            return Err(ErroPagamento::EstornoManual);
        }
        let referencia = self.referencia(id)?;
        self.lancar(id, Conteudo::Intencao { acao: Acao::Estornar })?;
        self.provedor.estornar(&referencia)?;
        self.transicao(id, Evento::Estornar, None, None)
    }

    /// Closes a payment in progress here, after someone checked on the terminal that it went
    /// nowhere. It tells the provider nothing, so it is for what [`Desfecho::Indeterminado`] leaves.
    pub fn abandonar(&mut self, id: &IdPagamento, motivo: &str) -> Result<Estado, ErroPagamento> {
        self.transicao(id, Evento::Cancelar, None, Some(motivo.to_owned()))
    }

    /// Writes down a refund done outside the integration, on the provider's terminal or portal.
    pub fn registrar_estorno_externo(&mut self, id: &IdPagamento) -> Result<Estado, ErroPagamento> {
        self.estado(id)?;
        self.transicao(id, Evento::Estornar, None, Some("estorno fora da integração".into()))
    }

    /// Asks the provider about a payment and brings the journal up to date. Nothing is undone here.
    pub fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroPagamento> {
        self.estado(id)?;
        let situacao = self.provedor.consultar(id)?;
        self.reconciliar(id, &situacao)?;
        Ok(situacao)
    }

    /// Settles everything left open by a power loss, a crash or an earlier session.
    ///
    /// The rule: a written decision is carried out, and without one an approval whose sale did not
    /// finish is undone. With no third leg nothing can be undone, so it is reported as
    /// [`Desfecho::CobradoSemConclusao`] for the application to decide. It is safe to call this
    /// again at any moment, also after a new power loss halfway through it.
    pub fn recuperar(&mut self) -> Result<Vec<Recuperacao>, ErroPagamento> {
        let meu = self.provedor.id();
        let abertos: Vec<IdPagamento> = self
            .registros
            .iter()
            .filter(|(_, registro)| {
                registro.provedor == meu
                    && (registro.estado.pendente()
                        || (registro.estado == Estado::Confirmado && registro.intencao.is_some()))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut relatorio = Vec::new();
        for id in abertos {
            let antes = self.estado(&id)?;
            let desfecho = self.recuperar_um(&id)?;
            relatorio.push(Recuperacao {
                id: Some(id.clone()),
                antes: Some(antes),
                depois: Some(self.estado(&id)?),
                desfecho,
            });
        }
        self.recuperar_pendencias(&mut relatorio)?;
        Ok(relatorio)
    }

    fn recuperar_um(&mut self, id: &IdPagamento) -> Result<Desfecho, ErroPagamento> {
        let registro = self.registros.get(id).ok_or_else(|| ErroPagamento::NaoEncontrado(id.clone()))?;
        let (estado, intencao) = (registro.estado, registro.intencao);
        match (estado, intencao) {
            // Sending is written before the call, so in this state the provider was never called.
            (Estado::Criado, _) => {
                self.transicao(id, Evento::Cancelar, None, Some("nunca enviado".into()))?;
                Ok(Desfecho::CanceladoSemEnvio)
            }
            (Estado::Aguardando, _) => self.recuperar_em_curso(id),
            (Estado::Aprovado, _) => self.recuperar_aprovado(id, intencao),
            (Estado::Confirmado, Some(Acao::Estornar)) => self.recuperar_estorno(id),
            _ => Ok(Desfecho::Reconciliado),
        }
    }

    fn recuperar_em_curso(&mut self, id: &IdPagamento) -> Result<Desfecho, ErroPagamento> {
        let situacao = match self.provedor.consultar(id) {
            Ok(situacao) => situacao,
            Err(erro) => return Ok(Desfecho::Falhou(erro.to_string())),
        };
        match situacao {
            SituacaoProvedor::Desconhecida => {
                self.transicao(id, Evento::Cancelar, None, Some("o provedor não conhece".into()))?;
                Ok(Desfecho::CanceladoSemEnvio)
            }
            SituacaoProvedor::Indeterminada => Ok(Desfecho::Indeterminado),
            SituacaoProvedor::EmAndamento => {
                self.lancar(id, Conteudo::Intencao { acao: Acao::Cancelar })?;
                match self.provedor.cancelar(id) {
                    Ok(Cancelamento::Feito) => {
                        self.transicao(id, Evento::Cancelar, None, Some("interrompido ao religar".into()))?;
                        Ok(Desfecho::Reconciliado)
                    }
                    Ok(Cancelamento::Solicitado) | Err(_) => Ok(Desfecho::EmAndamento),
                }
            }
            SituacaoProvedor::Aprovada(_) => {
                self.reconciliar(id, &situacao)?;
                self.recuperar_aprovado(id, None)
            }
            outra => {
                self.reconciliar(id, &outra)?;
                Ok(Desfecho::Reconciliado)
            }
        }
    }

    fn recuperar_aprovado(
        &mut self,
        id: &IdPagamento,
        intencao: Option<Acao>,
    ) -> Result<Desfecho, ErroPagamento> {
        let terceira_perna = self.provedor.capacidades().terceira_perna;
        let (acao, desfecho_ok) = match (intencao, terceira_perna) {
            (Some(Acao::Confirmar), _) => (Acao::Confirmar, Desfecho::Confirmado),
            (Some(Acao::Estornar), _) => return self.recuperar_estorno(id),
            (_, true) => (Acao::Desfazer, Desfecho::Desfeito),
            (_, false) => return Ok(Desfecho::CobradoSemConclusao),
        };
        let resultado = match acao {
            Acao::Confirmar => self.confirmar(id),
            _ => self.desfazer(id),
        };
        match resultado {
            Ok(_) => Ok(desfecho_ok),
            Err(ErroPagamento::Provedor(erro)) => Ok(Desfecho::Falhou(erro.to_string())),
            Err(erro) => Err(erro),
        }
    }

    /// A refund has no safe blind retry, so the provider is asked first.
    fn recuperar_estorno(&mut self, id: &IdPagamento) -> Result<Desfecho, ErroPagamento> {
        match self.provedor.consultar(id) {
            Ok(SituacaoProvedor::Estornada) => {
                self.transicao(id, Evento::Estornar, None, None)?;
                Ok(Desfecho::Reconciliado)
            }
            Ok(_) => match self.estornar(id) {
                Ok(_) => Ok(Desfecho::Reconciliado),
                Err(ErroPagamento::Provedor(erro)) => Ok(Desfecho::Falhou(erro.to_string())),
                Err(erro) => Err(erro),
            },
            Err(erro) => Ok(Desfecho::Falhou(erro.to_string())),
        }
    }

    fn recuperar_pendencias(&mut self, relatorio: &mut Vec<Recuperacao>) -> Result<(), ErroPagamento> {
        let pendencias = match self.provedor.pendencias() {
            Ok(pendencias) => pendencias,
            Err(erro) => {
                relatorio.push(Recuperacao {
                    id: None,
                    antes: None,
                    depois: None,
                    desfecho: Desfecho::Falhou(erro.to_string()),
                });
                return Ok(());
            }
        };
        for referencia in pendencias {
            let estado = referencia.id.as_ref().and_then(|id| self.registros.get(id)).map(|r| r.estado);
            // Payments still open here were already reported above and stay as they are.
            if estado.is_some_and(Estado::pendente) {
                continue;
            }
            let (resultado, desfecho_ok) = if estado == Some(Estado::Confirmado) {
                (self.provedor.confirmar(&referencia), Desfecho::PendenciaConfirmada)
            } else {
                (self.provedor.desfazer(&referencia), Desfecho::PendenciaDesfeita)
            };
            relatorio.push(Recuperacao {
                id: referencia.id.clone(),
                antes: estado,
                depois: estado,
                desfecho: match resultado {
                    Ok(()) => desfecho_ok,
                    Err(erro) => Desfecho::Falhou(erro.to_string()),
                },
            });
        }
        Ok(())
    }

    fn guardar_rastro(&mut self, id: &IdPagamento) -> Result<(), ErroPagamento> {
        let Some(valor) = self.provedor.rastro(id) else { return Ok(()) };
        let igual = self.registros.get(id).and_then(|r| r.rastro.as_deref()) == Some(valor.as_str());
        if !igual {
            self.lancar(id, Conteudo::Rastro { valor })?;
        }
        Ok(())
    }

    /// Walks the state along the legal path to what the provider says.
    fn reconciliar(&mut self, id: &IdPagamento, situacao: &SituacaoProvedor) -> Result<(), ErroPagamento> {
        let estado = self.estado(id)?;
        match situacao {
            SituacaoProvedor::Desconhecida
            | SituacaoProvedor::Indeterminada
            | SituacaoProvedor::EmAndamento => {}
            SituacaoProvedor::Aprovada(aprovacao) => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Aprovar, Some(aprovacao.clone()), None)?;
                }
            }
            SituacaoProvedor::Negada { codigo, motivo } => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Negar, None, Some(format!("{codigo}: {motivo}")))?;
                }
            }
            SituacaoProvedor::Cancelada => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Cancelar, None, None)?;
                }
            }
            SituacaoProvedor::Confirmada(aprovacao) => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Aprovar, Some(aprovacao.clone()), None)?;
                }
                if self.estado(id)? == Estado::Aprovado {
                    self.transicao(id, Evento::Confirmar, None, None)?;
                }
            }
            SituacaoProvedor::Desfeita => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Aprovar, None, None)?;
                }
                if self.estado(id)? == Estado::Aprovado {
                    self.transicao(id, Evento::Desfazer, None, None)?;
                }
            }
            SituacaoProvedor::Estornada => {
                if estado == Estado::Aguardando {
                    self.transicao(id, Evento::Aprovar, None, None)?;
                }
                if matches!(self.estado(id)?, Estado::Aprovado | Estado::Confirmado) {
                    self.transicao(id, Evento::Estornar, None, None)?;
                }
            }
        }
        Ok(())
    }

    fn absorver(
        &mut self,
        id: &IdPagamento,
        resultado: Result<Passo, ErroProvedor>,
    ) -> Result<Passo, ErroPagamento> {
        match resultado {
            Ok(Passo::Aprovado(aprovacao)) => {
                self.transicao(id, Evento::Aprovar, Some(aprovacao.clone()), None)?;
                Ok(Passo::Aprovado(aprovacao))
            }
            Ok(Passo::Negado { codigo, motivo }) => {
                self.transicao(id, Evento::Negar, None, Some(format!("{codigo}: {motivo}")))?;
                Ok(Passo::Negado { codigo, motivo })
            }
            Ok(Passo::Cancelado { motivo }) => {
                self.transicao(id, Evento::Cancelar, None, Some(motivo.clone()))?;
                Ok(Passo::Cancelado { motivo })
            }
            Ok(passo) => Ok(passo),
            Err(ErroProvedor::Recusado { codigo, motivo }) => {
                self.transicao(id, Evento::Negar, None, Some(format!("{codigo}: {motivo}")))?;
                Ok(Passo::Negado { codigo, motivo })
            }
            Err(erro) => Err(erro.into()),
        }
    }

    fn estado(&self, id: &IdPagamento) -> Result<Estado, ErroPagamento> {
        self.registros.get(id).map(|r| r.estado).ok_or_else(|| ErroPagamento::NaoEncontrado(id.clone()))
    }

    fn aprovada(&self, id: &IdPagamento) -> Result<ReferenciaTransacao, ErroPagamento> {
        let estado = self.estado(id)?;
        if estado != Estado::Aprovado {
            return Err(ErroDominio::TransicaoInvalida { estado, evento: Evento::Confirmar }.into());
        }
        self.referencia(id)
    }

    fn referencia(&self, id: &IdPagamento) -> Result<ReferenciaTransacao, ErroPagamento> {
        let registro = self.registros.get(id).ok_or_else(|| ErroPagamento::NaoEncontrado(id.clone()))?;
        let mut referencia = registro
            .aprovacao
            .as_ref()
            .map_or_else(ReferenciaTransacao::default, |aprovacao| aprovacao.referencia.clone());
        referencia.id = Some(id.clone());
        if referencia.valor_centavos == 0 {
            referencia.valor_centavos = registro.pedido.valor_centavos;
        }
        Ok(referencia)
    }

    fn transicao(
        &mut self,
        id: &IdPagamento,
        evento: Evento,
        aprovacao: Option<Box<Aprovacao>>,
        motivo: Option<String>,
    ) -> Result<Estado, ErroPagamento> {
        let novo = transitar(self.estado(id)?, evento)?;
        self.lancar(id, Conteudo::Transicao { evento, aprovacao, motivo })?;
        Ok(novo)
    }

    fn lancar(&mut self, id: &IdPagamento, conteudo: Conteudo) -> Result<(), ErroPagamento> {
        let lancamento = Lancamento { seq: self.proximo_seq, id: id.clone(), conteudo };
        self.diario.gravar(&lancamento)?;
        self.proximo_seq += 1;
        aplicar(&mut self.registros, &lancamento)?;
        Ok(())
    }
}
