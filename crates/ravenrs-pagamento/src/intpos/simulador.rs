//! A standard manager for tests: a program of files, in one call at a time, with no thread.

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use super::{ArquivoIntPos, Caminhos};
use crate::relogio::{Relogio, RelogioManual};

/// What the simulated manager does with the next `CRT` or `CNC`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RespostaGerenciador {
    /// Approves with the data of a fictional network.
    Aprovar,
    /// Approves with no `013-000`.
    AprovarSemAutorizacao,
    /// Refuses with this status and message.
    Negar(String, String),
    /// Writes the status file and then never answers.
    Calar,
}

#[derive(Debug, Default)]
struct Interno {
    respostas: VecDeque<RespostaGerenciador>,
    pedidos: Vec<ArquivoIntPos>,
    pendentes: Vec<String>,
    confirmados: Vec<String>,
    desfeitos: Vec<String>,
    cancelados: Vec<String>,
    desligado: bool,
    segura_resposta: Option<(ArquivoIntPos, u32)>,
    sequencia: u64,
}

/// The manager on the other side of the files. Its clones are the same manager.
///
/// Each [`passo`](Self::passo) does what a real one would do by then, so a test decides the order
/// of everything. There is no card here, and no value.
#[derive(Clone, Debug)]
pub struct GerenciadorSimulado {
    caminhos: Caminhos,
    interno: Arc<Mutex<Interno>>,
}

impl GerenciadorSimulado {
    /// A manager over these files.
    #[must_use]
    pub fn new(caminhos: Caminhos) -> Self {
        Self { caminhos, interno: Arc::default() }
    }

    fn com<T>(&self, f: impl FnOnce(&mut Interno) -> T) -> T {
        f(&mut self.interno.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Queues the outcome of the next `CRT` or `CNC`. With nothing queued, it approves.
    pub fn programar(&self, resposta: RespostaGerenciador) {
        self.com(|i| i.respostas.push_back(resposta));
    }

    /// Stops reading the folder, as a manager that was closed.
    pub fn desligar(&self, desligado: bool) {
        self.com(|i| i.desligado = desligado);
    }

    /// Delays the response by this many steps after the status file.
    pub fn atrasar_resposta(&self) {
        self.com(|i| i.segura_resposta = Some((ArquivoIntPos::default(), 2)));
    }

    /// All requests it read, in order.
    #[must_use]
    pub fn pedidos(&self) -> Vec<ArquivoIntPos> {
        self.com(|i| i.pedidos.clone())
    }

    /// The commands it read, in order.
    #[must_use]
    pub fn comandos(&self) -> Vec<String> {
        self.com(|i| i.pedidos.iter().filter_map(|p| p.comando().map(str::to_owned)).collect())
    }

    /// The control codes approved and waiting for the third leg.
    #[must_use]
    pub fn pendentes(&self) -> Vec<String> {
        self.com(|i| i.pendentes.clone())
    }

    /// The control codes that got `CNF`.
    #[must_use]
    pub fn confirmados(&self) -> Vec<String> {
        self.com(|i| i.confirmados.clone())
    }

    /// The control codes that got `NCN`.
    #[must_use]
    pub fn desfeitos(&self) -> Vec<String> {
        self.com(|i| i.desfeitos.clone())
    }

    /// The NSUs that got `CNC`.
    #[must_use]
    pub fn cancelados(&self) -> Vec<String> {
        self.com(|i| i.cancelados.clone())
    }

    /// Reads the folder once and does what it finds.
    pub fn passo(&self) -> io::Result<()> {
        if self.com(|i| i.desligado) {
            return Ok(());
        }
        self.soltar_resposta_segura()?;
        let bytes = match fs::read(&self.caminhos.req) {
            Ok(bytes) => bytes,
            Err(erro) if erro.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(erro) => return Err(erro),
        };
        let pedido = ArquivoIntPos::ler(&bytes);
        fs::remove_file(&self.caminhos.req)?;
        let comando = pedido.comando().unwrap_or_default().to_owned();
        let numero = pedido.identificacao().unwrap_or_default().to_owned();
        self.com(|i| i.pedidos.push(pedido.clone()));

        let mut sts = ArquivoIntPos::default();
        sts.definir(0, 0, comando.as_str());
        sts.definir(1, 0, numero.as_str());
        sts.definir(999, 999, "0");
        fs::write(&self.caminhos.sts, sts.codificar())?;

        match comando.as_str() {
            "CRT" | "CNC" => self.responder(&pedido, &comando, &numero),
            "CNF" => {
                let controle = pedido.valor(27).unwrap_or_default().to_owned();
                self.com(|i| {
                    i.pendentes.retain(|p| p != &controle);
                    i.confirmados.push(controle);
                });
                Ok(())
            }
            "NCN" => {
                let controle = pedido.valor(27).unwrap_or_default().to_owned();
                self.com(|i| {
                    i.pendentes.retain(|p| p != &controle);
                    i.desfeitos.push(controle);
                });
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn soltar_resposta_segura(&self) -> io::Result<()> {
        let pronta = self.com(|i| match i.segura_resposta.as_mut() {
            Some((arquivo, passos)) if arquivo.completo() => {
                if *passos == 0 {
                    i.segura_resposta.take().map(|(arquivo, _)| arquivo)
                } else {
                    *passos -= 1;
                    None
                }
            }
            _ => None,
        });
        if let Some(arquivo) = pronta {
            fs::write(&self.caminhos.resp, arquivo.codificar())?;
        }
        Ok(())
    }

    fn responder(&self, pedido: &ArquivoIntPos, comando: &str, numero: &str) -> io::Result<()> {
        let programada = self.com(|i| i.respostas.pop_front()).unwrap_or(RespostaGerenciador::Aprovar);
        if programada == RespostaGerenciador::Calar {
            return Ok(());
        }
        let sequencia = self.com(|i| {
            i.sequencia += 1;
            i.sequencia
        });
        let mut resposta = ArquivoIntPos::default();
        resposta.definir(0, 0, comando);
        resposta.definir(1, 0, numero);
        resposta.definir(3, 0, pedido.valor(3).unwrap_or("0"));
        resposta.definir(4, 0, "0");
        resposta.definir(999, 999, "0");
        if let RespostaGerenciador::Negar(status, mensagem) = &programada {
            resposta.definir(9, 0, status.as_str());
            resposta.definir(30, 0, mensagem.as_str());
        } else {
            let controle = format!("CTRL{sequencia:04}");
            let nsu = format!("{:012}", 700_000 + sequencia);
            resposta.definir(9, 0, "0");
            resposta.definir(10, 0, "STONE");
            resposta.definir(12, 0, nsu.as_str());
            if programada != RespostaGerenciador::AprovarSemAutorizacao {
                resposta.definir(13, 0, format!("A{sequencia:05}"));
            }
            resposta.definir(22, 0, "05112026");
            resposta.definir(23, 0, "143000");
            resposta.definir(27, 0, controle.as_str());
            resposta.definir(30, 0, "APROVADA (SIMULADOR)");
            resposta.definir(748, 0, "MASTERCARD");
            resposta.definir(731, 0, "2");
            resposta.definir(712, 0, "2");
            resposta.definir(713, 1, "VIA DO CLIENTE (SIMULADOR)");
            resposta.definir(713, 2, "SEM VALOR");
            resposta.definir(714, 0, "1");
            resposta.definir(715, 1, "VIA DA LOJA (SIMULADOR)");
            if comando == "CRT" {
                self.com(|i| i.pendentes.push(controle));
            } else {
                let cancelado = pedido.valor(12).unwrap_or_default().to_owned();
                self.com(|i| {
                    i.cancelados.push(cancelado);
                    i.pendentes.push(controle);
                });
            }
        }
        let segurando = self.com(|i| {
            if let Some((arquivo, _)) = i.segura_resposta.as_mut() {
                *arquivo = resposta.clone();
                true
            } else {
                false
            }
        });
        if segurando {
            return Ok(());
        }
        fs::write(&self.caminhos.resp, resposta.codificar())
    }
}

/// A clock whose waits run the manager, so that blocking parts work in a single thread.
#[derive(Clone, Debug)]
pub struct RelogioComGerenciador {
    /// The time.
    pub relogio: RelogioManual,
    /// Who runs during a wait.
    pub gerenciador: GerenciadorSimulado,
}

impl Relogio for RelogioComGerenciador {
    fn agora_ms(&self) -> u64 {
        self.relogio.agora_ms()
    }

    fn esperar_ms(&mut self, ms: u64) {
        self.relogio.avancar(ms);
        // A failure here shows up to the caller as the manager not answering.
        let _ = self.gerenciador.passo();
    }
}
