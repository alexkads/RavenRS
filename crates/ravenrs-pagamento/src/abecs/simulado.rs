//! A pinpad in memory, with what a checkout uses of a real one.
//!
//! It knows no card commands: `GPN`, `GCX` and the like get the same answer as an unknown command.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use super::comandos::{
    PP_DSPTXTSZ, PP_MFSUP, PP_MNNAME, PP_MODEL, PP_SERNUM, PP_SPECVER, PP_VALUE, StatusAbecs,
};
use super::driver::PortaPinpad;
use super::quadro::{self, ACK, Decodificador, EOT, EventoQuadro, NAK, crc16};
use crate::relogio::{Relogio, RelogioManual};

#[derive(Debug, Default)]
struct Carga {
    nome: String,
    tamanho: u32,
    crc: u16,
    dados: Vec<u8>,
}

#[derive(Debug, Default)]
struct Estado {
    decodificador: Decodificador,
    saida: Vec<u8>,
    aberto: bool,
    bloqueado: Option<[u8; 3]>,
    ultima_resposta: Vec<u8>,
    tela: String,
    midias: BTreeMap<String, Vec<u8>>,
    midia_exibida: Option<String>,
    carga: Option<Carga>,
    capturas: VecDeque<Result<String, StatusAbecs>>,
    menus: VecDeque<Result<u8, StatusAbecs>>,
    teclas: VecDeque<StatusAbecs>,
    comandos: Vec<String>,
    naks_restantes: u8,
    respostas_ruins: u8,
    mudo: bool,
}

/// A pinpad driven from a test. Its clones are the same pinpad.
#[derive(Clone, Debug, Default)]
pub struct PinpadSimulado(Arc<Mutex<Estado>>);

impl PinpadSimulado {
    /// A pinpad that was just plugged in.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn com<R>(&self, f: impl FnOnce(&mut Estado) -> R) -> R {
        f(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Queues what the customer will type for `GCD`. With nothing queued, `GCD` blocks.
    pub fn digitar(&self, valor: impl Into<String>) {
        let valor = valor.into();
        self.com(|e| e.capturas.push_back(Ok(valor)));
    }

    /// Queues an outcome with no value for `GCD`, such as [`StatusAbecs::CANCEL`].
    pub fn capturar_com_status(&self, status: StatusAbecs) {
        self.com(|e| e.capturas.push_back(Err(status)));
    }

    /// Queues the option the customer will pick in `MNU`, from 1. With nothing queued, `MNU` blocks.
    pub fn escolher(&self, opcao: u8) {
        self.com(|e| e.menus.push_back(Ok(opcao)));
    }

    /// Queues the key for `GKY`, as a status. With nothing queued, `GKY` blocks.
    pub fn pressionar(&self, tecla: StatusAbecs) {
        self.com(|e| e.teclas.push_back(tecla));
    }

    /// Refuses this many of the next commands as if they arrived broken.
    pub fn recusar_proximos(&self, vezes: u8) {
        self.com(|e| e.naks_restantes = vezes);
    }

    /// Spoils the CRC of this many of the next responses.
    pub fn estragar_respostas(&self, vezes: u8) {
        self.com(|e| e.respostas_ruins = vezes);
    }

    /// Stops answering anything.
    pub fn emudecer(&self, mudo: bool) {
        self.com(|e| e.mudo = mudo);
    }

    /// What its display shows.
    #[must_use]
    pub fn tela(&self) -> String {
        self.com(|e| e.tela.clone())
    }

    /// The media file on display.
    #[must_use]
    pub fn midia_exibida(&self) -> Option<String> {
        self.com(|e| e.midia_exibida.clone())
    }

    /// The bytes of a loaded media file.
    #[must_use]
    pub fn midia(&self, nome: &str) -> Option<Vec<u8>> {
        self.com(|e| e.midias.get(&nome.to_ascii_uppercase()).cloned())
    }

    /// The codes of the commands it took, in order.
    #[must_use]
    pub fn comandos(&self) -> Vec<String> {
        self.com(|e| e.comandos.clone())
    }

    /// Takes in bytes from the point of sale.
    pub fn receber(&self, bytes: &[u8]) {
        self.com(|e| {
            if e.mudo {
                return;
            }
            for byte in bytes {
                if let Some(evento) = e.decodificador.alimentar(*byte) {
                    e.tratar(evento);
                }
            }
        });
    }

    /// Takes the bytes it wants to send.
    #[must_use]
    pub fn retirar_saida(&self) -> Vec<u8> {
        self.com(|e| std::mem::take(&mut e.saida))
    }
}

fn tlv(campos: &[(u16, &[u8])]) -> Vec<u8> {
    let mut bloco = Vec::new();
    for (id, valor) in campos {
        bloco.extend_from_slice(&id.to_be_bytes());
        bloco.extend_from_slice(&u16::try_from(valor.len()).unwrap_or(0).to_be_bytes());
        bloco.extend_from_slice(valor);
    }
    bloco
}

/// Reads the parameters of an "Abecs Command" that follow the code.
fn parametros(corpo: &[u8]) -> Option<Vec<(u16, Vec<u8>)>> {
    let mut lista = Vec::new();
    let mut resto = corpo;
    while !resto.is_empty() {
        let tamanho: usize = std::str::from_utf8(resto.get(..3)?).ok()?.parse().ok()?;
        let mut bloco = resto.get(3..3 + tamanho)?;
        resto = &resto[3 + tamanho..];
        while !bloco.is_empty() {
            let id = u16::from_be_bytes([*bloco.first()?, *bloco.get(1)?]);
            let n = usize::from(u16::from_be_bytes([*bloco.get(2)?, *bloco.get(3)?]));
            lista.push((id, bloco.get(4..4 + n)?.to_vec()));
            bloco = &bloco[4 + n..];
        }
    }
    Some(lista)
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

impl Estado {
    fn tratar(&mut self, evento: EventoQuadro) {
        match evento {
            EventoQuadro::Can => {
                self.bloqueado = None;
                self.saida.push(EOT);
            }
            EventoQuadro::PacoteInvalido => self.saida.push(NAK),
            EventoQuadro::Pacote(dados) => {
                if self.naks_restantes > 0 {
                    self.naks_restantes -= 1;
                    self.saida.push(NAK);
                    return;
                }
                self.saida.push(ACK);
                self.executar(&dados);
            }
            EventoQuadro::Nak => {
                let dados = self.ultima_resposta.clone();
                self.transmitir(&dados);
            }
            EventoQuadro::Ack | EventoQuadro::Eot => {}
        }
    }

    fn responder(&mut self, id: &[u8], status: StatusAbecs, bloco: Option<&[u8]>) {
        let mut dados = id.to_vec();
        dados.extend_from_slice(format!("{:03}", status.0).as_bytes());
        if let Some(bloco) = bloco {
            dados.extend_from_slice(format!("{:03}", bloco.len()).as_bytes());
            dados.extend_from_slice(bloco);
        }
        self.ultima_resposta.clone_from(&dados);
        self.transmitir(&dados);
    }

    fn transmitir(&mut self, dados: &[u8]) {
        let Ok(mut pacote) = quadro::codificar_pacote(dados) else { return };
        if self.respostas_ruins > 0 {
            self.respostas_ruins -= 1;
            if let Some(ultimo) = pacote.last_mut() {
                *ultimo ^= 0xFF;
            }
        }
        self.saida.extend_from_slice(&pacote);
    }

    fn executar(&mut self, dados: &[u8]) {
        let Some(id) = dados.get(..3) else {
            self.responder(b"ERR", StatusAbecs::INVCALL, None);
            return;
        };
        let codigo = latin1(id);
        self.comandos.push(codigo.clone());
        let corpo = &dados[3..];
        if id != b"OPN" && !self.aberto {
            self.responder(id, StatusAbecs::INVCALL, None);
            return;
        }
        match id {
            b"OPN" => {
                self.aberto = true;
                self.responder(id, StatusAbecs::OK, None);
            }
            b"CLO" => {
                self.aberto = false;
                self.tela = latin1(corpo.get(3..).unwrap_or_default());
                self.responder(id, StatusAbecs::OK, None);
            }
            b"DSP" => {
                self.tela = latin1(corpo.get(3..).unwrap_or_default());
                self.midia_exibida = None;
                self.responder(id, StatusAbecs::OK, None);
            }
            b"DEX" => {
                self.tela = latin1(corpo.get(6..).unwrap_or_default());
                self.midia_exibida = None;
                self.responder(id, StatusAbecs::OK, None);
            }
            b"GIN" => {
                let bloco = format!(
                    "{:<20}{:<19}C{:<20}2.20{:<16}{:<20}",
                    "SIMULADOR", "PINPAD;1MB", "SEM VALOR", "000.01 260101", "SIM0001"
                );
                self.responder(id, StatusAbecs::OK, Some(bloco.as_bytes()));
            }
            b"GIX" => {
                let bloco = tlv(&[
                    (PP_SERNUM, b"SIM0001"),
                    (PP_MODEL, b"PINPAD;1MB"),
                    (PP_MNNAME, b"SIMULADOR"),
                    (PP_SPECVER, b"2.20"),
                    (PP_DSPTXTSZ, b"0216"),
                    (PP_MFSUP, b"111"),
                ]);
                self.responder(id, StatusAbecs::OK, Some(&bloco));
            }
            b"GCD" => match self.capturas.pop_front() {
                Some(Ok(valor)) => {
                    let bloco = tlv(&[(PP_VALUE, valor.as_bytes())]);
                    self.responder(id, StatusAbecs::OK, Some(&bloco));
                }
                Some(Err(status)) => self.responder(id, status, None),
                None => self.bloqueado = Some(*b"GCD"),
            },
            b"MNU" => match self.menus.pop_front() {
                Some(Ok(opcao)) => {
                    let texto = format!("{opcao:02}");
                    let bloco = tlv(&[(PP_VALUE, texto.as_bytes())]);
                    self.responder(id, StatusAbecs::OK, Some(&bloco));
                }
                Some(Err(status)) => self.responder(id, status, None),
                None => self.bloqueado = Some(*b"MNU"),
            },
            b"GKY" => match self.teclas.pop_front() {
                Some(status) => self.responder(id, status, None),
                None => self.bloqueado = Some(*b"GKY"),
            },
            b"MLI" => self.mli(id, corpo),
            b"MLR" => self.mlr(id, corpo),
            b"MLE" => self.mle(id),
            b"DSI" => self.dsi(id, corpo),
            _ => self.responder(b"ERR", StatusAbecs::INVCALL, None),
        }
    }

    fn mli(&mut self, id: &[u8], corpo: &[u8]) {
        let lista = parametros(corpo).unwrap_or_default();
        let nome = lista.iter().find(|(p, _)| *p == 0x001E).map(|(_, v)| latin1(v).to_ascii_uppercase());
        let info = lista.iter().find(|(p, _)| *p == 0x001F).map(|(_, v)| v.clone());
        match (nome, info) {
            (Some(nome), Some(info)) if info.len() == 10 => {
                self.carga = Some(Carga {
                    nome,
                    tamanho: u32::from_be_bytes([info[0], info[1], info[2], info[3]]),
                    crc: u16::from_be_bytes([info[4], info[5]]),
                    dados: Vec::new(),
                });
                self.responder(id, StatusAbecs::OK, None);
            }
            _ => self.responder(id, StatusAbecs::MANDAT, None),
        }
    }

    fn mlr(&mut self, id: &[u8], corpo: &[u8]) {
        let Some(carga) = self.carga.as_mut() else {
            self.responder(id, StatusAbecs::INVCALL, None);
            return;
        };
        for (parametro, valor) in parametros(corpo).unwrap_or_default() {
            if parametro == 0x000F {
                carga.dados.extend_from_slice(&valor);
            }
        }
        self.responder(id, StatusAbecs::OK, None);
    }

    fn mle(&mut self, id: &[u8]) {
        let Some(carga) = self.carga.take() else {
            self.responder(id, StatusAbecs::INVCALL, None);
            return;
        };
        let certo = u32::try_from(carga.dados.len()) == Ok(carga.tamanho) && crc16(&carga.dados) == carga.crc;
        if certo {
            self.midias.insert(carga.nome, carga.dados);
            self.responder(id, StatusAbecs::OK, None);
        } else {
            self.responder(id, StatusAbecs::MFERR, None);
        }
    }

    fn dsi(&mut self, id: &[u8], corpo: &[u8]) {
        let nome = parametros(corpo)
            .unwrap_or_default()
            .into_iter()
            .find(|(p, _)| *p == 0x001E)
            .map(|(_, v)| latin1(&v).to_ascii_uppercase());
        match nome {
            Some(nome) if self.midias.contains_key(&nome) => {
                self.midia_exibida = Some(nome);
                self.responder(id, StatusAbecs::OK, None);
            }
            _ => self.responder(id, StatusAbecs::MFNFOUND, None),
        }
    }
}

/// A port plugged into a [`PinpadSimulado`], whose reads move a manual clock.
#[derive(Debug)]
pub struct PortaSimulada {
    pinpad: PinpadSimulado,
    relogio: RelogioManual,
    passo_ms: u64,
}

impl PortaSimulada {
    /// Plugs in. Each empty read moves the clock by 100 ms.
    #[must_use]
    pub fn new(pinpad: PinpadSimulado, relogio: RelogioManual) -> Self {
        Self { pinpad, relogio, passo_ms: 100 }
    }
}

impl PortaPinpad for PortaSimulada {
    fn escrever(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.pinpad.receber(bytes);
        Ok(())
    }

    fn ler(&mut self, destino: &mut [u8]) -> io::Result<usize> {
        let saida = self.pinpad.retirar_saida();
        if saida.is_empty() {
            self.relogio.avancar(self.passo_ms);
            return Ok(0);
        }
        // The buffer of the driver takes it all at once; a larger answer would not be split here.
        let n = saida.len().min(destino.len());
        destino[..n].copy_from_slice(&saida[..n]);
        if n < saida.len() {
            self.pinpad.com(|e| {
                let mut resto = saida[n..].to_vec();
                resto.append(&mut e.saida);
                e.saida = resto;
            });
        }
        Ok(n)
    }

    fn agora_ms(&self) -> u64 {
        self.relogio.agora_ms()
    }
}
