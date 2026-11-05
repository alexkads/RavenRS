//! The flow of the link with time as state: it holds no clock, no thread and no port.
//!
//! Give it the bytes that arrived and the current time; take the bytes to send and the events.

use std::collections::VecDeque;
use std::fmt;

use super::comandos::{ErroComando, RespostaAbecs, de_latin1};
use super::quadro::{self, CAN, Decodificador, EventoQuadro, NAK, PacoteGrande};

/// How long to wait for `ACK` or `NAK` after a command, and for `EOT` after `CAN`.
pub const TEMPO_ACK_MS: u64 = 2_000;
/// How long to wait for the response of a command that does not block.
pub const TEMPO_RESPOSTA_MS: u64 = 10_000;
/// How many times a command is sent, a response is refused or `CAN` is tried.
pub const TENTATIVAS: u8 = 3;

/// What the link reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventoSessao {
    /// The response to the command.
    Resposta(RespostaAbecs),
    /// A message for the screen, in the middle of a blocking command.
    Notificacao(String),
    /// The pinpad confirmed the cancellation with `EOT`.
    Cancelado,
    /// The exchange failed.
    Falhou(FalhaEnlace),
}

/// Why the exchange failed. The session goes back to idle in all of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FalhaEnlace {
    /// No `ACK` nor `NAK` in time.
    SemAck,
    /// The third `NAK` came.
    NakExcedido,
    /// No response in time.
    SemResposta,
    /// The response stayed invalid after refusing it the most times allowed.
    RespostaInvalida,
    /// No `EOT` after trying `CAN` the most times allowed.
    SemEot,
}

impl fmt::Display for FalhaEnlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SemAck => "o pinpad não respondeu ACK nem NAK",
            Self::NakExcedido => "o pinpad recusou o comando três vezes",
            Self::SemResposta => "o pinpad não respondeu a tempo",
            Self::RespostaInvalida => "a resposta chegou inválida três vezes",
            Self::SemEot => "o pinpad não confirmou o cancelamento",
        })
    }
}

/// Why a command could not be started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroSessao {
    /// The last exchange has not finished.
    Ocupada,
    /// The packet would be too long.
    Pacote(PacoteGrande),
}

impl fmt::Display for ErroSessao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ocupada => f.write_str("a sessão ainda espera a troca anterior"),
            Self::Pacote(erro) => erro.fmt(f),
        }
    }
}

impl std::error::Error for ErroSessao {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Fase {
    Ociosa,
    EsperandoAck { pacote: Vec<u8>, envios: u8, limite: u64, bloqueante: bool },
    EsperandoResposta { recusas: u8, limite: Option<u64> },
    Cancelando { envios: u8, limite: u64 },
}

/// The link with a pinpad, from the side of the point of sale.
#[derive(Clone, Debug)]
pub struct Sessao {
    fase: Fase,
    decodificador: Decodificador,
    saida: Vec<u8>,
    eventos: VecDeque<EventoSessao>,
}

impl Default for Sessao {
    fn default() -> Self {
        Self::new()
    }
}

impl Sessao {
    /// An idle session.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fase: Fase::Ociosa,
            decodificador: Decodificador::default(),
            saida: Vec::new(),
            eventos: VecDeque::new(),
        }
    }

    /// True when nothing is in progress.
    #[must_use]
    pub fn ociosa(&self) -> bool {
        self.fase == Fase::Ociosa
    }

    /// Sends a command's bytes. A blocking one has no time limit for its response.
    pub fn enviar(&mut self, dados: &[u8], bloqueante: bool, agora_ms: u64) -> Result<(), ErroSessao> {
        if !self.ociosa() {
            return Err(ErroSessao::Ocupada);
        }
        let pacote = quadro::codificar_pacote(dados).map_err(ErroSessao::Pacote)?;
        self.saida.extend_from_slice(&pacote);
        self.decodificador.reiniciar();
        self.fase = Fase::EsperandoAck { pacote, envios: 1, limite: agora_ms + TEMPO_ACK_MS, bloqueante };
        Ok(())
    }

    /// Sends `CAN`. The specification asks to start every conversation with this.
    pub fn cancelar(&mut self, agora_ms: u64) {
        self.saida.push(CAN);
        self.decodificador.reiniciar();
        self.fase = Fase::Cancelando { envios: 1, limite: agora_ms + TEMPO_ACK_MS };
    }

    /// Takes in the bytes that arrived.
    pub fn receber(&mut self, bytes: &[u8], agora_ms: u64) {
        for byte in bytes {
            if let Some(evento) = self.decodificador.alimentar(*byte) {
                self.tratar(evento, agora_ms);
            }
        }
    }

    /// Lets time pass; call it when [`prazo_ms`](Self::prazo_ms) arrives.
    pub fn tick(&mut self, agora_ms: u64) {
        match &mut self.fase {
            Fase::EsperandoAck { limite, .. } if agora_ms >= *limite => {
                self.falhar(FalhaEnlace::SemAck);
            }
            Fase::EsperandoResposta { limite: Some(limite), .. } if agora_ms >= *limite => {
                self.falhar(FalhaEnlace::SemResposta);
            }
            Fase::Cancelando { envios, limite } if agora_ms >= *limite => {
                if *envios >= TENTATIVAS {
                    self.falhar(FalhaEnlace::SemEot);
                } else {
                    *envios += 1;
                    *limite = agora_ms + TEMPO_ACK_MS;
                    self.saida.push(CAN);
                }
            }
            _ => {}
        }
    }

    /// When [`tick`](Self::tick) has something to do, if ever.
    #[must_use]
    pub fn prazo_ms(&self) -> Option<u64> {
        match &self.fase {
            Fase::Ociosa => None,
            Fase::EsperandoAck { limite, .. } | Fase::Cancelando { limite, .. } => Some(*limite),
            Fase::EsperandoResposta { limite, .. } => *limite,
        }
    }

    /// Takes the bytes waiting to go out.
    pub fn retirar_saida(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.saida)
    }

    /// Takes the next event.
    pub fn proximo_evento(&mut self) -> Option<EventoSessao> {
        self.eventos.pop_front()
    }

    fn falhar(&mut self, falha: FalhaEnlace) {
        self.fase = Fase::Ociosa;
        self.eventos.push_back(EventoSessao::Falhou(falha));
    }

    fn tratar(&mut self, evento: EventoQuadro, agora_ms: u64) {
        match (&mut self.fase, evento) {
            (Fase::EsperandoAck { bloqueante, .. }, EventoQuadro::Ack) => {
                let resposta = (!*bloqueante).then_some(agora_ms + TEMPO_RESPOSTA_MS);
                self.fase = Fase::EsperandoResposta { recusas: 0, limite: resposta };
            }
            (Fase::EsperandoAck { pacote, envios, limite, .. }, EventoQuadro::Nak) => {
                if *envios >= TENTATIVAS {
                    self.falhar(FalhaEnlace::NakExcedido);
                } else {
                    *envios += 1;
                    *limite = agora_ms + TEMPO_ACK_MS;
                    self.saida.extend_from_slice(pacote);
                }
            }
            (Fase::EsperandoResposta { .. }, EventoQuadro::Pacote(dados)) => {
                self.tratar_pacote(&dados, agora_ms)
            }
            (Fase::EsperandoResposta { recusas, .. }, EventoQuadro::PacoteInvalido) => {
                if *recusas >= TENTATIVAS {
                    self.falhar(FalhaEnlace::RespostaInvalida);
                } else {
                    *recusas += 1;
                    self.saida.push(NAK);
                }
            }
            (Fase::Cancelando { .. }, EventoQuadro::Eot) => {
                self.fase = Fase::Ociosa;
                self.eventos.push_back(EventoSessao::Cancelado);
            }
            // The specification asks to ignore whatever else comes while cancelling.
            _ => {}
        }
    }

    fn tratar_pacote(&mut self, dados: &[u8], agora_ms: u64) {
        let lida: Result<RespostaAbecs, ErroComando> = RespostaAbecs::ler(dados);
        match lida {
            Ok(resposta) if resposta.notificacao() => {
                let texto = resposta.blocos.first().map_or_else(String::new, |bloco| de_latin1(bloco));
                self.eventos.push_back(EventoSessao::Notificacao(texto.trim().to_owned()));
            }
            Ok(resposta) => {
                self.fase = Fase::Ociosa;
                self.eventos.push_back(EventoSessao::Resposta(resposta));
            }
            Err(_) => self.tratar(EventoQuadro::PacoteInvalido, agora_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::quadro::{ACK, EOT, codificar_pacote};
    use super::*;

    fn resposta(dados: &[u8]) -> Vec<u8> {
        codificar_pacote(dados).expect("cabe")
    }

    #[test]
    fn caminho_feliz() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 0).expect("envia");
        assert_eq!(sessao.retirar_saida(), [0x16, 0x4F, 0x50, 0x4E, 0x17, 0xA8, 0xA9]);
        assert_eq!(sessao.prazo_ms(), Some(TEMPO_ACK_MS));
        sessao.receber(&[ACK], 100);
        assert_eq!(sessao.prazo_ms(), Some(100 + TEMPO_RESPOSTA_MS));
        sessao.receber(&resposta(b"OPN000"), 200);
        assert!(sessao.ociosa());
        assert!(matches!(sessao.proximo_evento(), Some(EventoSessao::Resposta(r)) if r.status.ok()));
    }

    #[test]
    fn comando_durante_outro_e_recusado() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 0).expect("envia");
        assert_eq!(sessao.enviar(b"OPN", false, 0), Err(ErroSessao::Ocupada));
    }

    #[test]
    fn nak_reenvia_e_o_terceiro_aborta() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 0).expect("envia");
        let primeiro = sessao.retirar_saida();
        sessao.receber(&[NAK], 10);
        assert_eq!(sessao.retirar_saida(), primeiro);
        sessao.receber(&[NAK], 20);
        assert_eq!(sessao.retirar_saida(), primeiro);
        sessao.receber(&[NAK], 30);
        assert!(sessao.retirar_saida().is_empty());
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Falhou(FalhaEnlace::NakExcedido)));
        assert!(sessao.ociosa());
    }

    #[test]
    fn sem_ack_em_dois_segundos_aborta() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 1000).expect("envia");
        sessao.tick(2999);
        assert_eq!(sessao.proximo_evento(), None);
        sessao.tick(3000);
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Falhou(FalhaEnlace::SemAck)));
    }

    #[test]
    fn sem_resposta_em_dez_segundos_aborta_quando_nao_bloqueia() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"GIX", false, 0).expect("envia");
        sessao.receber(&[ACK], 0);
        sessao.tick(TEMPO_RESPOSTA_MS);
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Falhou(FalhaEnlace::SemResposta)));
    }

    #[test]
    fn bloqueante_espera_para_sempre_e_recebe_notificacao() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"GKY", true, 0).expect("envia");
        sessao.receber(&[ACK], 0);
        assert_eq!(sessao.prazo_ms(), None);
        sessao.tick(u64::MAX);
        assert_eq!(sessao.proximo_evento(), None);
        sessao.receber(&resposta(b"NTM000016AGUARDE         "), 5);
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Notificacao("AGUARDE".into())));
        assert!(!sessao.ociosa());
        sessao.receber(&resposta(b"GKY013"), 6);
        assert!(matches!(sessao.proximo_evento(), Some(EventoSessao::Resposta(_))));
    }

    #[test]
    fn resposta_invalida_pede_de_novo_ate_o_limite() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 0).expect("envia");
        sessao.receber(&[ACK], 0);
        sessao.retirar_saida();
        let mut ruim = resposta(b"OPN000");
        let ultimo = ruim.len() - 1;
        ruim[ultimo] ^= 0xFF;
        for _ in 0..TENTATIVAS {
            sessao.receber(&ruim, 1);
            assert_eq!(sessao.retirar_saida(), [NAK]);
        }
        sessao.receber(&ruim, 1);
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Falhou(FalhaEnlace::RespostaInvalida)));
    }

    #[test]
    fn nao_manda_ack_para_resposta_boa() {
        let mut sessao = Sessao::new();
        sessao.enviar(b"OPN", false, 0).expect("envia");
        sessao.retirar_saida();
        sessao.receber(&[ACK], 0);
        sessao.receber(&resposta(b"OPN000"), 0);
        assert!(sessao.retirar_saida().is_empty());
    }

    #[test]
    fn cancelamento_tenta_tres_vezes_e_ignora_o_que_vier() {
        let mut sessao = Sessao::new();
        sessao.cancelar(0);
        assert_eq!(sessao.retirar_saida(), [CAN]);
        sessao.receber(&resposta(b"GKY013"), 10);
        assert_eq!(sessao.proximo_evento(), None);
        sessao.tick(TEMPO_ACK_MS);
        assert_eq!(sessao.retirar_saida(), [CAN]);
        sessao.tick(2 * TEMPO_ACK_MS);
        assert_eq!(sessao.retirar_saida(), [CAN]);
        sessao.tick(3 * TEMPO_ACK_MS);
        assert!(sessao.retirar_saida().is_empty());
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Falhou(FalhaEnlace::SemEot)));
    }

    #[test]
    fn eot_confirma_o_cancelamento() {
        let mut sessao = Sessao::new();
        sessao.cancelar(0);
        sessao.receber(&[EOT], 10);
        assert_eq!(sessao.proximo_evento(), Some(EventoSessao::Cancelado));
        assert!(sessao.ociosa());
    }
}
