//! The journal: what was about to happen is written before it happens.
//!
//! It keeps ids, states, references, fiscal data and receipts. It never keeps what the operator
//! typed, since that may be a tax id, and no provider adapter puts a card number in a reference.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::dominio::{Aprovacao, Estado, Evento, IdPagamento, PedidoPagamento, ProvedorId, transitar};

/// An action on the provider that is about to be attempted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Acao {
    /// Start the payment.
    Iniciar,
    /// Confirm it.
    Confirmar,
    /// Undo it.
    Desfazer,
    /// Cancel it while in progress.
    Cancelar,
    /// Refund it.
    Estornar,
}

/// What each line of the journal says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conteudo {
    /// A new payment.
    Aberto {
        /// The request.
        pedido: PedidoPagamento,
        /// Who will process it.
        provedor: ProvedorId,
    },
    /// The provider is about to be called for this.
    Intencao {
        /// The action.
        acao: Acao,
    },
    /// What the adapter needs to find this payment after a restart.
    Rastro {
        /// Opaque to all but the adapter.
        valor: String,
    },
    /// The state changes now.
    Transicao {
        /// What moved it.
        evento: Evento,
        /// The approval, when this is one.
        aprovacao: Option<Box<Aprovacao>>,
        /// The reason, when there is one for the operator.
        motivo: Option<String>,
    },
}

/// A line of the journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lancamento {
    /// The position in the journal, from 1.
    pub seq: u64,
    /// The payment.
    pub id: IdPagamento,
    /// What it says.
    pub conteudo: Conteudo,
}

/// Why the journal failed. Without a journal no provider is called.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroDiario {
    /// It could not be written or read.
    Io(String),
    /// A line in the middle does not parse: this is damage, not a power loss.
    Corrompido {
        /// The line number, from 1.
        linha: usize,
    },
    /// The lines do not form a legal history.
    Incoerente(String),
}

impl fmt::Display for ErroDiario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(motivo) => write!(f, "falha no diário: {motivo}"),
            Self::Corrompido { linha } => write!(f, "diário corrompido na linha {linha}"),
            Self::Incoerente(motivo) => write!(f, "diário incoerente: {motivo}"),
        }
    }
}

impl std::error::Error for ErroDiario {}

/// Where the journal lives.
pub trait Diario {
    /// Writes one line durably, and returns only after that.
    fn gravar(&mut self, lancamento: &Lancamento) -> Result<(), ErroDiario>;

    /// Reads all lines, in order.
    fn ler(&self) -> Result<Vec<Lancamento>, ErroDiario>;
}

/// What the journal knows about one payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registro {
    /// The request.
    pub pedido: PedidoPagamento,
    /// Who processes it.
    pub provedor: ProvedorId,
    /// The current state.
    pub estado: Estado,
    /// The approval, once there is one.
    pub aprovacao: Option<Box<Aprovacao>>,
    /// The reason of a refusal or a cancellation.
    pub motivo: Option<String>,
    /// The action last attempted that has no outcome yet.
    pub intencao: Option<Acao>,
    /// What the adapter needs to find this payment after a restart.
    pub rastro: Option<String>,
}

/// Applies one line to the map of payments.
pub(crate) fn aplicar(
    registros: &mut BTreeMap<IdPagamento, Registro>,
    lancamento: &Lancamento,
) -> Result<(), ErroDiario> {
    let id = &lancamento.id;
    match &lancamento.conteudo {
        Conteudo::Aberto { pedido, provedor } => {
            if registros.contains_key(id) {
                return Err(ErroDiario::Incoerente(format!("{id} aberto duas vezes")));
            }
            registros.insert(
                id.clone(),
                Registro {
                    pedido: pedido.clone(),
                    provedor: *provedor,
                    estado: Estado::Criado,
                    aprovacao: None,
                    motivo: None,
                    intencao: None,
                    rastro: None,
                },
            );
        }
        Conteudo::Intencao { acao } => {
            let registro =
                registros.get_mut(id).ok_or_else(|| ErroDiario::Incoerente(format!("{id} sem abertura")))?;
            registro.intencao = Some(*acao);
        }
        Conteudo::Rastro { valor } => {
            let registro =
                registros.get_mut(id).ok_or_else(|| ErroDiario::Incoerente(format!("{id} sem abertura")))?;
            registro.rastro = Some(valor.clone());
        }
        Conteudo::Transicao { evento, aprovacao, motivo } => {
            let registro =
                registros.get_mut(id).ok_or_else(|| ErroDiario::Incoerente(format!("{id} sem abertura")))?;
            registro.estado = transitar(registro.estado, *evento)
                .map_err(|erro| ErroDiario::Incoerente(format!("{id}: {erro}")))?;
            if aprovacao.is_some() {
                registro.aprovacao.clone_from(aprovacao);
            }
            if motivo.is_some() {
                registro.motivo.clone_from(motivo);
            }
            // Sending is what the intention of starting was about, and it stays open until the
            // provider answers. Every other move settles the intention.
            if *evento != Evento::Enviar {
                registro.intencao = None;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct DiarioMemoriaInterno {
    linhas: Vec<Lancamento>,
    resta: Option<usize>,
}

/// A journal in memory, shared by its clones, for tests and for targets with no files.
#[derive(Clone, Debug, Default)]
pub struct DiarioMemoria(Arc<Mutex<DiarioMemoriaInterno>>);

impl DiarioMemoria {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Simulates the power going off: after this many more lines, every write fails.
    pub fn cair_depois_de(&self, linhas: usize) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).resta = Some(linhas);
    }

    /// The power came back: writes work again.
    pub fn religar(&self) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).resta = None;
    }

    /// How many lines it holds.
    #[must_use]
    pub fn tamanho(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).linhas.len()
    }
}

impl Diario for DiarioMemoria {
    fn gravar(&mut self, lancamento: &Lancamento) -> Result<(), ErroDiario> {
        let mut interno = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(resta) = interno.resta.as_mut() {
            if *resta == 0 {
                return Err(ErroDiario::Io("energia caiu (simulada)".into()));
            }
            *resta -= 1;
        }
        interno.linhas.push(lancamento.clone());
        Ok(())
    }

    fn ler(&self) -> Result<Vec<Lancamento>, ErroDiario> {
        Ok(self.0.lock().unwrap_or_else(PoisonError::into_inner).linhas.clone())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use arquivo::DiarioArquivo;

#[cfg(not(target_arch = "wasm32"))]
mod arquivo {
    use std::fs::{self, File, OpenOptions};
    use std::io::{Seek, SeekFrom, Write};
    use std::path::{Path, PathBuf};

    use super::{Diario, ErroDiario, Lancamento};

    /// The native default: one JSON per line, flushed to the disk before each return.
    ///
    /// A partial last line is what a power loss leaves, so opening cuts it off. Damage anywhere
    /// else is refused with [`ErroDiario::Corrompido`]. The file holds receipts and fiscal data:
    /// keep it where only the point of sale reads.
    #[derive(Debug)]
    pub struct DiarioArquivo {
        caminho: PathBuf,
        arquivo: File,
    }

    fn io(erro: &std::io::Error) -> ErroDiario {
        ErroDiario::Io(erro.to_string())
    }

    impl DiarioArquivo {
        /// Opens or creates the journal, creating its folder if needed.
        pub fn abrir(caminho: impl AsRef<Path>) -> Result<Self, ErroDiario> {
            let caminho = caminho.as_ref().to_path_buf();
            if let Some(pasta) = caminho.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(pasta).map_err(|e| io(&e))?;
            }
            let mut arquivo =
                OpenOptions::new().create(true).read(true).append(true).open(&caminho).map_err(|e| io(&e))?;
            let bytes = fs::read(&caminho).map_err(|e| io(&e))?;
            let bom = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |fim| fim + 1);
            if bom != bytes.len() {
                arquivo.set_len(bom as u64).map_err(|e| io(&e))?;
                arquivo.seek(SeekFrom::End(0)).map_err(|e| io(&e))?;
                arquivo.sync_all().map_err(|e| io(&e))?;
            }
            Ok(Self { caminho, arquivo })
        }
    }

    impl Diario for DiarioArquivo {
        fn gravar(&mut self, lancamento: &Lancamento) -> Result<(), ErroDiario> {
            let mut linha = serde_json::to_vec(lancamento).map_err(|e| ErroDiario::Io(e.to_string()))?;
            linha.push(b'\n');
            self.arquivo.write_all(&linha).map_err(|e| io(&e))?;
            self.arquivo.sync_data().map_err(|e| io(&e))
        }

        fn ler(&self) -> Result<Vec<Lancamento>, ErroDiario> {
            let texto = fs::read(&self.caminho).map_err(|e| io(&e))?;
            let mut linhas = Vec::new();
            for (indice, linha) in texto.split(|b| *b == b'\n').enumerate() {
                if linha.is_empty() {
                    continue;
                }
                let lancamento = serde_json::from_slice(linha)
                    .map_err(|_| ErroDiario::Corrompido { linha: indice + 1 })?;
                linhas.push(lancamento);
            }
            Ok(linhas)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::TipoPagamento;

    fn abertura(seq: u64) -> Lancamento {
        let id = IdPagamento::new(format!("p{seq}")).expect("id");
        let pedido = PedidoPagamento::new(id.clone(), 100, TipoPagamento::Debito).expect("pedido");
        Lancamento { seq, id, conteudo: Conteudo::Aberto { pedido, provedor: ProvedorId::Simulador } }
    }

    #[test]
    fn memoria_para_de_gravar_quando_a_energia_cai() {
        let mut diario = DiarioMemoria::new();
        diario.cair_depois_de(1);
        assert!(diario.gravar(&abertura(1)).is_ok());
        assert!(diario.gravar(&abertura(2)).is_err());
        assert_eq!(diario.tamanho(), 1);
        diario.religar();
        assert!(diario.gravar(&abertura(2)).is_ok());
    }

    #[test]
    fn historia_ilegal_e_recusada() {
        let id = IdPagamento::new("x").expect("id");
        let mut registros = BTreeMap::new();
        let sem_abertura =
            Lancamento { seq: 1, id: id.clone(), conteudo: Conteudo::Intencao { acao: Acao::Iniciar } };
        assert!(aplicar(&mut registros, &sem_abertura).is_err());
        let aberto = abertura(1);
        aplicar(&mut registros, &aberto).expect("abre");
        assert!(aplicar(&mut registros, &aberto).is_err());
        let salto = Lancamento {
            seq: 2,
            id: aberto.id,
            conteudo: Conteudo::Transicao { evento: Evento::Confirmar, aprovacao: None, motivo: None },
        };
        assert!(aplicar(&mut registros, &salto).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod arquivo {
        use std::fs;
        use std::path::PathBuf;

        use super::*;

        fn caminho(nome: &str) -> PathBuf {
            let pasta =
                std::env::temp_dir().join(format!("ravenrs-pagamento-diario-{}-{nome}", std::process::id()));
            let _ = fs::remove_dir_all(&pasta);
            pasta.join("sub").join("diario.jsonl")
        }

        #[test]
        fn grava_e_le_de_volta() {
            let arquivo = caminho("volta");
            let mut diario = DiarioArquivo::abrir(&arquivo).expect("abre");
            diario.gravar(&abertura(1)).expect("grava");
            diario.gravar(&abertura(2)).expect("grava");
            drop(diario);
            let diario = DiarioArquivo::abrir(&arquivo).expect("reabre");
            assert_eq!(diario.ler().expect("lê"), vec![abertura(1), abertura(2)]);
            let _ = fs::remove_dir_all(arquivo.parent().and_then(|p| p.parent()).expect("pasta"));
        }

        #[test]
        fn linha_cortada_no_fim_e_o_que_a_queda_deixa() {
            let arquivo = caminho("cortada");
            let mut diario = DiarioArquivo::abrir(&arquivo).expect("abre");
            diario.gravar(&abertura(1)).expect("grava");
            drop(diario);
            let mut bytes = fs::read(&arquivo).expect("lê");
            bytes.extend_from_slice(b"{\"seq\":2,\"id\":\"p2\",\"conte");
            fs::write(&arquivo, &bytes).expect("escreve");

            let mut diario = DiarioArquivo::abrir(&arquivo).expect("reabre");
            assert_eq!(diario.ler().expect("lê"), vec![abertura(1)]);
            diario.gravar(&abertura(2)).expect("grava depois do corte");
            assert_eq!(diario.ler().expect("lê"), vec![abertura(1), abertura(2)]);
            let _ = fs::remove_dir_all(arquivo.parent().and_then(|p| p.parent()).expect("pasta"));
        }

        #[test]
        fn dano_no_meio_nao_passa_em_silencio() {
            let arquivo = caminho("dano");
            let mut diario = DiarioArquivo::abrir(&arquivo).expect("abre");
            diario.gravar(&abertura(1)).expect("grava");
            drop(diario);
            let mut bytes = b"lixo\n".to_vec();
            bytes.extend(fs::read(&arquivo).expect("lê"));
            fs::write(&arquivo, &bytes).expect("escreve");
            let diario = DiarioArquivo::abrir(&arquivo).expect("reabre");
            assert_eq!(diario.ler(), Err(ErroDiario::Corrompido { linha: 1 }));
            let _ = fs::remove_dir_all(arquivo.parent().and_then(|p| p.parent()).expect("pasta"));
        }
    }
}
