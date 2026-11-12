//! The contract of the helper process: how a library with `unsafe` stays out of this workspace.
//!
//! The workspace forbids `unsafe`, and CliSiTef and Stone TEF only exist as native libraries. So a
//! separate binary, outside this workspace, loads the library. It speaks one JSON per line: it reads
//! a [`PedidoAuxiliar`] from stdin and writes one [`RespostaAuxiliar`] to stdout, with logs on
//! stderr. A crash there does not take the point of sale down, and it may be 32-bit if the library is.
//!
//! [`ProvedorAuxiliar`] is the side of the workspace. [`atender`] is the side of the helper, for any
//! [`ProvedorPagamento`]: the real helper only implements that trait over the library, using
//! [`crate::sitef::CicloSiTef`]. **The helper with FFI does not exist yet**; `docs/PAGAMENTOS.md`
//! describes it. Only a simulated helper was run.
//!
//! The answer of the operator crosses the pipe, and neither side may log it.

use std::collections::VecDeque;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::dominio::{
    Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, ProvedorId, ReferenciaTransacao,
};
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};

/// The version of this contract. A helper with another one is refused.
pub const VERSAO_CONTRATO: u32 = 1;

/// A line from the workspace to the helper.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoAuxiliar {
    /// Goes back in the response, to catch a helper that lost its place.
    pub seq: u64,
    /// What to do.
    pub operacao: OperacaoAuxiliar,
}

impl fmt::Debug for PedidoAuxiliar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PedidoAuxiliar").field("seq", &self.seq).finish_non_exhaustive()
    }
}

/// The operations of the contract, one for each method of the trait, and the opening.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum OperacaoAuxiliar {
    /// The first line: says the version and asks who it is.
    Ola {
        /// [`VERSAO_CONTRATO`].
        versao: u32,
    },
    /// [`ProvedorPagamento::iniciar`].
    Iniciar {
        /// The request.
        pedido: PedidoPagamento,
    },
    /// [`ProvedorPagamento::continuar`].
    Continuar {
        /// The payment.
        id: IdPagamento,
        /// What the operator did.
        entrada: EntradaOperador,
    },
    /// [`ProvedorPagamento::confirmar`].
    Confirmar {
        /// The transaction.
        referencia: ReferenciaTransacao,
    },
    /// [`ProvedorPagamento::desfazer`].
    Desfazer {
        /// The transaction.
        referencia: ReferenciaTransacao,
    },
    /// [`ProvedorPagamento::cancelar`].
    Cancelar {
        /// The payment.
        id: IdPagamento,
    },
    /// [`ProvedorPagamento::estornar`].
    Estornar {
        /// The transaction.
        referencia: ReferenciaTransacao,
    },
    /// [`ProvedorPagamento::consultar`].
    Consultar {
        /// The payment.
        id: IdPagamento,
    },
    /// [`ProvedorPagamento::pendencias`].
    Pendencias,
}

/// A line from the helper to the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RespostaAuxiliar {
    /// The `seq` of the request.
    pub seq: u64,
    /// The answer.
    pub resultado: ResultadoAuxiliar,
}

/// The answers of the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "tipo", content = "dados")]
pub enum ResultadoAuxiliar {
    /// The answer to [`OperacaoAuxiliar::Ola`].
    Ola {
        /// The version of the contract it speaks.
        versao: u32,
        /// Who it is.
        provedor: ProvedorId,
        /// What it can do.
        capacidades: Capacidades,
    },
    /// A step.
    Passo(Passo),
    /// Done.
    Feito,
    /// The outcome of a cancellation.
    Cancelamento(Cancelamento),
    /// What it knows about a payment.
    Situacao(SituacaoProvedor),
    /// What it holds open.
    Pendencias(Vec<ReferenciaTransacao>),
    /// A failure.
    Erro(ErroProvedor),
}

/// The helper's side: answers one line with one line, over any provider.
///
/// A line that cannot be read gets `seq` zero and a protocol error, and the workspace treats that
/// as an unknown outcome.
pub fn atender<P: ProvedorPagamento>(provedor: &mut P, linha: &str) -> String {
    let Ok(pedido) = serde_json::from_str::<PedidoAuxiliar>(linha) else {
        return serializar(&RespostaAuxiliar {
            seq: 0,
            resultado: ResultadoAuxiliar::Erro(ErroProvedor::Protocolo("linha ilegível".into())),
        });
    };
    let resultado = match pedido.operacao {
        OperacaoAuxiliar::Ola { versao } if versao != VERSAO_CONTRATO => {
            ResultadoAuxiliar::Erro(ErroProvedor::Protocolo(format!("contrato {versao} não atendido")))
        }
        OperacaoAuxiliar::Ola { .. } => ResultadoAuxiliar::Ola {
            versao: VERSAO_CONTRATO,
            provedor: provedor.id(),
            capacidades: provedor.capacidades(),
        },
        OperacaoAuxiliar::Iniciar { pedido } => passo(provedor.iniciar(&pedido)),
        OperacaoAuxiliar::Continuar { id, entrada } => passo(provedor.continuar(&id, entrada)),
        OperacaoAuxiliar::Confirmar { referencia } => feito(provedor.confirmar(&referencia)),
        OperacaoAuxiliar::Desfazer { referencia } => feito(provedor.desfazer(&referencia)),
        OperacaoAuxiliar::Estornar { referencia } => feito(provedor.estornar(&referencia)),
        OperacaoAuxiliar::Cancelar { id } => match provedor.cancelar(&id) {
            Ok(cancelamento) => ResultadoAuxiliar::Cancelamento(cancelamento),
            Err(erro) => ResultadoAuxiliar::Erro(erro),
        },
        OperacaoAuxiliar::Consultar { id } => match provedor.consultar(&id) {
            Ok(situacao) => ResultadoAuxiliar::Situacao(situacao),
            Err(erro) => ResultadoAuxiliar::Erro(erro),
        },
        OperacaoAuxiliar::Pendencias => match provedor.pendencias() {
            Ok(lista) => ResultadoAuxiliar::Pendencias(lista),
            Err(erro) => ResultadoAuxiliar::Erro(erro),
        },
    };
    serializar(&RespostaAuxiliar { seq: pedido.seq, resultado })
}

fn passo(resultado: Result<Passo, ErroProvedor>) -> ResultadoAuxiliar {
    resultado.map_or_else(ResultadoAuxiliar::Erro, ResultadoAuxiliar::Passo)
}

fn feito(resultado: Result<(), ErroProvedor>) -> ResultadoAuxiliar {
    resultado.map_or_else(ResultadoAuxiliar::Erro, |()| ResultadoAuxiliar::Feito)
}

fn serializar<V: Serialize>(valor: &V) -> String {
    serde_json::to_string(valor).unwrap_or_else(|_| "{}".to_owned())
}

/// The pipe to the helper.
pub trait CanalLinhas {
    /// Sends one line, without the line break.
    fn enviar(&mut self, linha: &str) -> Result<(), ErroProvedor>;

    /// Receives the next line. The other side being gone is [`ErroProvedor::Indisponivel`].
    fn receber(&mut self) -> Result<String, ErroProvedor>;
}

/// A channel that runs the helper's side in the same process, for tests.
#[derive(Debug)]
pub struct CanalLocal<P> {
    /// The provider behind the helper's side.
    pub provedor: P,
    respostas: VecDeque<String>,
}

impl<P: ProvedorPagamento> CanalLocal<P> {
    /// A channel to this provider.
    #[must_use]
    pub fn new(provedor: P) -> Self {
        Self { provedor, respostas: VecDeque::new() }
    }
}

impl<P: ProvedorPagamento> CanalLinhas for CanalLocal<P> {
    fn enviar(&mut self, linha: &str) -> Result<(), ErroProvedor> {
        let resposta = atender(&mut self.provedor, linha);
        self.respostas.push_back(resposta);
        Ok(())
    }

    fn receber(&mut self) -> Result<String, ErroProvedor> {
        self.respostas.pop_front().ok_or_else(|| ErroProvedor::Indisponivel("sem resposta".into()))
    }
}

/// A [`ProvedorPagamento`] that lives in a helper process.
///
/// A provider's answers are not kept here between calls, so a helper that dies leaves the journal as
/// the only memory, which is how it must be.
#[derive(Debug)]
pub struct ProvedorAuxiliar<C> {
    canal: C,
    provedor: ProvedorId,
    capacidades: Capacidades,
    seq: u64,
}

impl<C: CanalLinhas> ProvedorAuxiliar<C> {
    /// Greets the helper and learns who it is. A wrong version is refused here.
    pub fn conectar(mut canal: C) -> Result<Self, ErroProvedor> {
        let linha = serializar(&PedidoAuxiliar {
            seq: 1,
            operacao: OperacaoAuxiliar::Ola { versao: VERSAO_CONTRATO },
        });
        canal.enviar(&linha)?;
        let resposta = ler(&canal.receber()?, 1)?;
        match resposta {
            ResultadoAuxiliar::Ola { versao, provedor, capacidades } if versao == VERSAO_CONTRATO => {
                Ok(Self { canal, provedor, capacidades, seq: 1 })
            }
            ResultadoAuxiliar::Erro(erro) => Err(erro),
            outro => Err(ErroProvedor::Protocolo(format!("saudação inesperada: {outro:?}"))),
        }
    }

    fn pedir(&mut self, operacao: OperacaoAuxiliar) -> Result<ResultadoAuxiliar, ErroProvedor> {
        self.seq += 1;
        let linha = serializar(&PedidoAuxiliar { seq: self.seq, operacao });
        self.canal.enviar(&linha)?;
        let resultado = ler(&self.canal.receber()?, self.seq)?;
        if let ResultadoAuxiliar::Erro(erro) = resultado {
            return Err(erro);
        }
        Ok(resultado)
    }

    fn so_passo(&mut self, operacao: OperacaoAuxiliar) -> Result<Passo, ErroProvedor> {
        match self.pedir(operacao)? {
            ResultadoAuxiliar::Passo(passo) => Ok(passo),
            outro => Err(inesperada(&outro)),
        }
    }

    fn so_feito(&mut self, operacao: OperacaoAuxiliar) -> Result<(), ErroProvedor> {
        match self.pedir(operacao)? {
            ResultadoAuxiliar::Feito => Ok(()),
            outro => Err(inesperada(&outro)),
        }
    }
}

fn inesperada(resultado: &ResultadoAuxiliar) -> ErroProvedor {
    ErroProvedor::Protocolo(format!("resposta inesperada do auxiliar: {resultado:?}"))
}

fn ler(linha: &str, seq: u64) -> Result<ResultadoAuxiliar, ErroProvedor> {
    let resposta: RespostaAuxiliar = serde_json::from_str(linha)
        .map_err(|_| ErroProvedor::Protocolo("linha ilegível vinda do auxiliar".into()))?;
    if resposta.seq != seq {
        return Err(ErroProvedor::Protocolo(format!("esperava seq {seq}, veio {}", resposta.seq)));
    }
    Ok(resposta.resultado)
}

impl<C: CanalLinhas> ProvedorPagamento for ProvedorAuxiliar<C> {
    fn id(&self) -> ProvedorId {
        self.provedor
    }

    fn capacidades(&self) -> Capacidades {
        self.capacidades
    }

    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        self.so_passo(OperacaoAuxiliar::Iniciar { pedido: pedido.clone() })
    }

    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        self.so_passo(OperacaoAuxiliar::Continuar { id: id.clone(), entrada })
    }

    fn confirmar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        self.so_feito(OperacaoAuxiliar::Confirmar { referencia: referencia.clone() })
    }

    fn desfazer(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        self.so_feito(OperacaoAuxiliar::Desfazer { referencia: referencia.clone() })
    }

    fn cancelar(&mut self, id: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        match self.pedir(OperacaoAuxiliar::Cancelar { id: id.clone() })? {
            ResultadoAuxiliar::Cancelamento(cancelamento) => Ok(cancelamento),
            outro => Err(inesperada(&outro)),
        }
    }

    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        self.so_feito(OperacaoAuxiliar::Estornar { referencia: referencia.clone() })
    }

    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        match self.pedir(OperacaoAuxiliar::Consultar { id: id.clone() })? {
            ResultadoAuxiliar::Situacao(situacao) => Ok(situacao),
            outro => Err(inesperada(&outro)),
        }
    }

    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        match self.pedir(OperacaoAuxiliar::Pendencias)? {
            ResultadoAuxiliar::Pendencias(lista) => Ok(lista),
            outro => Err(inesperada(&outro)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use processo::CanalProcesso;

#[cfg(not(target_arch = "wasm32"))]
mod processo {
    use std::io::{BufRead, BufReader, Write};
    use std::path::Path;
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

    use super::CanalLinhas;
    use crate::provedor::ErroProvedor;

    /// A real helper process, spoken to over its stdin and stdout.
    ///
    /// Reads block until the helper answers. If it dies, the reading fails and the outcome is
    /// unknown, so the journal and `recuperar` take it from there.
    #[derive(Debug)]
    pub struct CanalProcesso {
        filho: Child,
        entrada: ChildStdin,
        saida: BufReader<ChildStdout>,
    }

    fn indisponivel(erro: &std::io::Error) -> ErroProvedor {
        ErroProvedor::Indisponivel(format!("auxiliar: {erro}"))
    }

    impl CanalProcesso {
        /// Starts the helper at this path.
        pub fn iniciar(executavel: impl AsRef<Path>, argumentos: &[&str]) -> Result<Self, ErroProvedor> {
            let mut filho = Command::new(executavel.as_ref())
                .args(argumentos)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|erro| indisponivel(&erro))?;
            let entrada =
                filho.stdin.take().ok_or_else(|| ErroProvedor::Indisponivel("auxiliar sem stdin".into()))?;
            let saida = filho
                .stdout
                .take()
                .ok_or_else(|| ErroProvedor::Indisponivel("auxiliar sem stdout".into()))?;
            Ok(Self { filho, entrada, saida: BufReader::new(saida) })
        }

        /// Kills the helper, as a test of what its crash does.
        pub fn matar(&mut self) {
            let _ = self.filho.kill();
            let _ = self.filho.wait();
        }
    }

    impl Drop for CanalProcesso {
        fn drop(&mut self) {
            self.matar();
        }
    }

    impl CanalLinhas for CanalProcesso {
        fn enviar(&mut self, linha: &str) -> Result<(), ErroProvedor> {
            self.entrada
                .write_all(linha.as_bytes())
                .and_then(|()| self.entrada.write_all(b"\n"))
                .and_then(|()| self.entrada.flush())
                .map_err(|erro| indisponivel(&erro))
        }

        fn receber(&mut self) -> Result<String, ErroProvedor> {
            let mut linha = String::new();
            let lidos = self.saida.read_line(&mut linha).map_err(|erro| indisponivel(&erro))?;
            if lidos == 0 {
                return Err(ErroProvedor::Indisponivel("o auxiliar fechou a saída".into()));
            }
            Ok(linha.trim_end().to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::TipoPagamento;
    use crate::simulador::{SimuladorProvedor, cenarios, iniciar};

    fn pedido() -> PedidoPagamento {
        PedidoPagamento::new(IdPagamento::new("x1").expect("id"), 500, TipoPagamento::Debito).expect("pedido")
    }

    #[test]
    fn saudacao_traz_quem_e_e_o_que_faz() {
        let provedor =
            ProvedorAuxiliar::conectar(CanalLocal::new(SimuladorProvedor::new())).expect("conecta");
        assert_eq!(provedor.id(), ProvedorId::Simulador);
        assert!(provedor.capacidades().terceira_perna);
    }

    #[test]
    fn versao_errada_e_recusada_pelo_auxiliar() {
        let mut simulador = SimuladorProvedor::new();
        let linha = serializar(&PedidoAuxiliar { seq: 7, operacao: OperacaoAuxiliar::Ola { versao: 99 } });
        let resposta: RespostaAuxiliar =
            serde_json::from_str(&atender(&mut simulador, &linha)).expect("json");
        assert_eq!(resposta.seq, 7);
        assert!(matches!(resposta.resultado, ResultadoAuxiliar::Erro(ErroProvedor::Protocolo(_))));
    }

    #[test]
    fn linha_ruim_nao_derruba_o_auxiliar() {
        let mut simulador = SimuladorProvedor::new();
        let resposta: RespostaAuxiliar =
            serde_json::from_str(&atender(&mut simulador, "{lixo")).expect("json");
        assert_eq!(resposta.seq, 0);
    }

    #[test]
    fn seq_trocado_e_incerto() {
        let linha = serializar(&RespostaAuxiliar { seq: 9, resultado: ResultadoAuxiliar::Feito });
        assert!(ler(&linha, 1).is_err_and(|erro| erro.incerto()));
    }

    #[test]
    fn todas_as_operacoes_atravessam_e_o_erro_tambem() {
        let simulador = SimuladorProvedor::new();
        let mut provedor = ProvedorAuxiliar::conectar(CanalLocal::new(simulador.clone())).expect("conecta");
        let pedido = pedido();
        let Passo::Aprovado(aprovacao) = provedor.iniciar(&pedido).expect("inicia") else { panic!("aprova") };
        assert!(matches!(provedor.consultar(&pedido.id), Ok(SituacaoProvedor::Aprovada(_))));
        assert_eq!(provedor.pendencias().expect("lista").len(), 1);
        provedor.confirmar(&aprovacao.referencia).expect("confirma");
        provedor.estornar(&aprovacao.referencia).expect("estorna");
        assert!(matches!(provedor.consultar(&pedido.id), Ok(SituacaoProvedor::Estornada)));
        simulador.use_once(iniciar(cenarios::timeout));
        assert_eq!(provedor.iniciar(&pedido), Err(ErroProvedor::Tempo));
    }
}
