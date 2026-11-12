//! The commands as calls, over any port. It blocks, so run it apart from the screen.

use std::fmt;
use std::io;

use super::comandos::{
    Comando, ErroComando, InformacaoGeral, MensagemGcd, PP_DSPTXTSZ, PP_MFSUP, PP_MNNAME, PP_MODEL,
    PP_SERNUM, PP_SPECVER, PP_VALUE, RespostaAbecs, StatusAbecs, TAMANHO_MAXIMO_MLR, TipoMidia,
    comando_inicio_midia,
};
use super::sessao::{ErroSessao, EventoSessao, FalhaEnlace, Sessao};

/// A serial link to a pinpad.
pub trait PortaPinpad {
    /// Writes all these bytes.
    fn escrever(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Reads what there is, waiting at most a short while. Zero means nothing arrived.
    fn ler(&mut self, destino: &mut [u8]) -> io::Result<usize>;

    /// Milliseconds from any fixed moment.
    fn agora_ms(&self) -> u64;
}

/// Why a call to the pinpad failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum ErroPinpad {
    /// The port failed.
    Porta(io::Error),
    /// The link failed.
    Enlace(FalhaEnlace),
    /// The command could not be built, or the response could not be read.
    Comando(ErroComando),
    /// The session refused it.
    Sessao(ErroSessao),
    /// The pinpad answered with an error.
    Status(StatusAbecs),
    /// The pinpad answered for another command.
    RespostaTrocada,
    /// A response is missing a field the specification calls mandatory.
    CampoAusente(u16),
}

impl fmt::Display for ErroPinpad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Porta(erro) => write!(f, "falha na porta: {erro}"),
            Self::Enlace(falha) => falha.fmt(f),
            Self::Comando(erro) => erro.fmt(f),
            Self::Sessao(erro) => erro.fmt(f),
            Self::Status(status) => write!(f, "o pinpad respondeu o status {:03}", status.0),
            Self::RespostaTrocada => f.write_str("o pinpad respondeu a outro comando"),
            Self::CampoAusente(id) => write!(f, "falta o campo {id:04X}h"),
        }
    }
}

impl std::error::Error for ErroPinpad {}

impl From<ErroComando> for ErroPinpad {
    fn from(erro: ErroComando) -> Self {
        Self::Comando(erro)
    }
}

/// What `GIX` says about the pinpad, in the fields a checkout uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InformacaoEstendida {
    /// Its serial number.
    pub serie: Option<String>,
    /// Its model.
    pub modelo: Option<String>,
    /// Its maker.
    pub fabricante: Option<String>,
    /// The version of the specification it follows.
    pub especificacao: Option<String>,
    /// Rows and columns of text, as `RRCC`.
    pub tela_texto: Option<String>,
    /// The media formats, as flags for PNG, JPG and GIF.
    pub midias: Option<String>,
}

impl InformacaoEstendida {
    /// True if it takes PNG, which is what a generated QR code uses.
    #[must_use]
    pub fn aceita_png(&self) -> bool {
        self.midias.as_deref().is_some_and(|m| m.starts_with('1'))
    }
}

/// A pinpad on a port. Only what needs no acquirer key is here.
#[derive(Debug)]
pub struct Pinpad<P> {
    porta: P,
    sessao: Sessao,
    notificacoes: Vec<String>,
}

impl<P: PortaPinpad> Pinpad<P> {
    /// Takes a port. Nothing is sent until [`abrir`](Self::abrir).
    #[must_use]
    pub fn new(porta: P) -> Self {
        Self { porta, sessao: Sessao::new(), notificacoes: Vec::new() }
    }

    /// Clears anything blocked with `CAN`, as the specification asks, and sends the classic `OPN`.
    ///
    /// The specification calls the classic `OPN` obsolete and prefers the secure one, which needs
    /// RSA and no command here depends on. A pinpad that refuses the classic format fails here.
    pub fn abrir(&mut self) -> Result<(), ErroPinpad> {
        self.interromper()?;
        self.executar_ok(&Comando::Abrir)
    }

    /// Aborts whatever is blocked in the pinpad.
    pub fn interromper(&mut self) -> Result<(), ErroPinpad> {
        let agora = self.porta.agora_ms();
        self.sessao.cancelar(agora);
        loop {
            self.bombear()?;
            match self.sessao.proximo_evento() {
                Some(EventoSessao::Cancelado) => return Ok(()),
                Some(EventoSessao::Falhou(falha)) => return Err(ErroPinpad::Enlace(falha)),
                _ => {}
            }
        }
    }

    /// `GIN` with index zero.
    pub fn informacao_geral(&mut self) -> Result<InformacaoGeral, ErroPinpad> {
        let resposta = self.executar(&Comando::InformacaoGeral { rede: 0 })?;
        Ok(InformacaoGeral::ler(&resposta)?)
    }

    /// `GIX`, asking only for what [`InformacaoEstendida`] holds.
    pub fn informacao_estendida(&mut self) -> Result<InformacaoEstendida, ErroPinpad> {
        let ids = vec![PP_SERNUM, PP_MODEL, PP_MNNAME, PP_SPECVER, PP_DSPTXTSZ, PP_MFSUP];
        let resposta = self.executar(&Comando::InformacaoEstendida { ids })?;
        Ok(InformacaoEstendida {
            serie: resposta.texto(PP_SERNUM)?,
            modelo: resposta.texto(PP_MODEL)?,
            fabricante: resposta.texto(PP_MNNAME)?,
            especificacao: resposta.texto(PP_SPECVER)?,
            tela_texto: resposta.texto(PP_DSPTXTSZ)?,
            midias: resposta.texto(PP_MFSUP)?,
        })
    }

    /// `DSP`.
    pub fn exibir(&mut self, linha1: &str, linha2: &str) -> Result<(), ErroPinpad> {
        self.executar_ok(&Comando::Exibir { linha1: linha1.into(), linha2: linha2.into() })
    }

    /// `DEX`.
    pub fn exibir_livre(&mut self, mensagem: &str) -> Result<(), ErroPinpad> {
        self.executar_ok(&Comando::ExibirLivre { mensagem: mensagem.into() })
    }

    /// `GKY`. It blocks until a key, and any key is an answer rather than an error.
    pub fn ler_tecla(&mut self) -> Result<StatusAbecs, ErroPinpad> {
        Ok(self.executar(&Comando::LerTecla)?.status)
    }

    /// `GCD`: the customer types a value on the keyboard, in clear. It blocks.
    ///
    /// The value is returned and nothing here stores it. The customer pressing cancel is the error
    /// [`ErroPinpad::Status`] with [`StatusAbecs::CANCEL`].
    pub fn capturar(
        &mut self,
        mensagem: MensagemGcd,
        minimo: Option<u8>,
        maximo: Option<u8>,
        tempo_s: Option<u8>,
    ) -> Result<String, ErroPinpad> {
        let comando = Comando::Capturar { mensagem, minimo, maximo, tempo_s, alfanumerico: false };
        let resposta = self.executar_com_sucesso(&comando)?;
        resposta.texto(PP_VALUE)?.ok_or(ErroPinpad::CampoAusente(PP_VALUE))
    }

    /// `MNU`: returns the chosen option, from zero. It blocks.
    pub fn menu(
        &mut self,
        titulo: Option<&str>,
        opcoes: &[&str],
        tempo_s: Option<u8>,
    ) -> Result<usize, ErroPinpad> {
        let comando = Comando::Menu {
            titulo: titulo.map(str::to_owned),
            opcoes: opcoes.iter().map(|opcao| (*opcao).to_owned()).collect(),
            tempo_s,
        };
        let resposta = self.executar_com_sucesso(&comando)?;
        let texto = resposta.texto(PP_VALUE)?.ok_or(ErroPinpad::CampoAusente(PP_VALUE))?;
        texto
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .ok_or(ErroPinpad::Comando(ErroComando::RespostaInvalida("índice do menu")))
    }

    /// `MLI`, `MLR` and `MLE`: loads an image, replacing any with the same name.
    pub fn carregar_midia(&mut self, nome: &str, tipo: TipoMidia, arquivo: &[u8]) -> Result<(), ErroPinpad> {
        self.executar_ok(&comando_inicio_midia(nome, tipo, arquivo)?)?;
        for parte in arquivo.chunks(TAMANHO_MAXIMO_MLR) {
            self.executar_ok(&Comando::CarregarMidiaDados { dados: parte.to_vec() })?;
        }
        self.executar_ok(&Comando::CarregarMidiaFim)
    }

    /// `DSI`.
    pub fn exibir_midia(&mut self, nome: &str) -> Result<(), ErroPinpad> {
        self.executar_ok(&Comando::ExibirMidia { nome: nome.into() })
    }

    /// Shows a QR code: loads the PNG under this name and displays it.
    ///
    /// Generating the image and the Pix payload is not done here. This does not make a charge.
    pub fn exibir_qr(&mut self, nome: &str, png: &[u8]) -> Result<(), ErroPinpad> {
        self.carregar_midia(nome, TipoMidia::Png, png)?;
        self.exibir_midia(nome)
    }

    /// `CLO`: leaves these two rows on the display.
    pub fn fechar(&mut self, linha1: &str, linha2: &str) -> Result<(), ErroPinpad> {
        self.executar_ok(&Comando::Fechar { linha1: linha1.into(), linha2: linha2.into() })
    }

    /// Takes the messages the pinpad sent during blocking commands.
    pub fn retirar_notificacoes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notificacoes)
    }

    /// Gives the port back.
    pub fn liberar(self) -> P {
        self.porta
    }

    fn executar_ok(&mut self, comando: &Comando) -> Result<(), ErroPinpad> {
        self.executar_com_sucesso(comando).map(|_| ())
    }

    fn executar_com_sucesso(&mut self, comando: &Comando) -> Result<RespostaAbecs, ErroPinpad> {
        let resposta = self.executar(comando)?;
        if resposta.status.ok() { Ok(resposta) } else { Err(ErroPinpad::Status(resposta.status)) }
    }

    fn executar(&mut self, comando: &Comando) -> Result<RespostaAbecs, ErroPinpad> {
        let dados = comando.codificar()?;
        let agora = self.porta.agora_ms();
        self.sessao.enviar(&dados, comando.bloqueante(), agora).map_err(ErroPinpad::Sessao)?;
        loop {
            self.bombear()?;
            match self.sessao.proximo_evento() {
                Some(EventoSessao::Resposta(resposta)) => {
                    let eco = resposta.id[..] == dados[..3] || &resposta.id == b"ERR";
                    if !eco {
                        return Err(ErroPinpad::RespostaTrocada);
                    }
                    if &resposta.id == b"ERR" {
                        return Err(ErroPinpad::Status(resposta.status));
                    }
                    return Ok(resposta);
                }
                Some(EventoSessao::Notificacao(texto)) => self.notificacoes.push(texto),
                Some(EventoSessao::Falhou(falha)) => return Err(ErroPinpad::Enlace(falha)),
                Some(EventoSessao::Cancelado) | None => {}
            }
        }
    }

    fn bombear(&mut self) -> Result<(), ErroPinpad> {
        let saida = self.sessao.retirar_saida();
        if !saida.is_empty() {
            self.porta.escrever(&saida).map_err(ErroPinpad::Porta)?;
        }
        let mut buffer = [0u8; 512];
        let lidos = self.porta.ler(&mut buffer).map_err(ErroPinpad::Porta)?;
        let agora = self.porta.agora_ms();
        self.sessao.receber(&buffer[..lidos], agora);
        self.sessao.tick(agora);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::simulado::{PinpadSimulado, PortaSimulada};
    use super::*;
    use crate::relogio::RelogioManual;

    fn aberto() -> (Pinpad<PortaSimulada>, PinpadSimulado) {
        let simulado = PinpadSimulado::new();
        let mut pinpad = Pinpad::new(PortaSimulada::new(simulado.clone(), RelogioManual::new()));
        pinpad.abrir().expect("abre");
        (pinpad, simulado)
    }

    #[test]
    fn abre_com_can_e_exibe() {
        let (mut pinpad, simulado) = aberto();
        pinpad.exibir("TOTAL", "R$ 19,90").expect("dsp");
        assert_eq!(simulado.tela(), format!("{:<16}{:<16}", "TOTAL", "R$ 19,90"));
        pinpad.exibir_livre("PASSE\nO CARTAO").expect("dex");
        assert_eq!(simulado.tela(), "PASSE\rO CARTAO");
        pinpad.fechar("OBRIGADO", "").expect("clo");
        assert_eq!(simulado.comandos(), ["OPN", "DSP", "DEX", "CLO"]);
    }

    #[test]
    fn sem_opn_o_pinpad_recusa() {
        let simulado = PinpadSimulado::new();
        let mut pinpad = Pinpad::new(PortaSimulada::new(simulado, RelogioManual::new()));
        assert!(matches!(pinpad.exibir("A", "B"), Err(ErroPinpad::Status(StatusAbecs::INVCALL))));
    }

    #[test]
    fn informacoes() {
        let (mut pinpad, _) = aberto();
        let geral = pinpad.informacao_geral().expect("gin");
        assert_eq!(geral.especificacao, "2.20");
        let estendida = pinpad.informacao_estendida().expect("gix");
        assert_eq!(estendida.serie.as_deref(), Some("SIM0001"));
        assert!(estendida.aceita_png());
    }

    #[test]
    fn cpf_digitado_volta_e_cancelamento_vira_status() {
        let (mut pinpad, simulado) = aberto();
        simulado.digitar("11122233344");
        assert_eq!(
            pinpad.capturar(MensagemGcd::Cpf, Some(11), Some(11), Some(60)).expect("gcd"),
            "11122233344"
        );
        simulado.capturar_com_status(StatusAbecs::CANCEL);
        assert!(matches!(
            pinpad.capturar(MensagemGcd::Cpf, None, None, None),
            Err(ErroPinpad::Status(StatusAbecs::CANCEL))
        ));
    }

    #[test]
    fn indice_de_cartao_nem_chega_ao_pinpad() {
        let (mut pinpad, simulado) = aberto();
        assert!(matches!(
            pinpad.capturar(MensagemGcd::Outra(0x0032), None, None, None),
            Err(ErroPinpad::Comando(ErroComando::MensagemProibida(0x0032)))
        ));
        assert_eq!(simulado.comandos(), ["OPN"]);
    }

    #[test]
    fn menu_e_tecla() {
        let (mut pinpad, simulado) = aberto();
        simulado.escolher(2);
        assert_eq!(pinpad.menu(Some("NOTA?"), &["1.Sim", "2.Nao"], Some(30)).expect("mnu"), 1);
        simulado.pressionar(StatusAbecs::CANCEL);
        assert_eq!(pinpad.ler_tecla().expect("gky"), StatusAbecs::CANCEL);
    }

    #[test]
    fn qr_grande_passa_por_varios_mlr() {
        let (mut pinpad, simulado) = aberto();
        let png: Vec<u8> = (0..5000u32).map(|n| (n % 251) as u8).collect();
        pinpad.exibir_qr("qr01", &png).expect("qr");
        assert_eq!(simulado.midia("QR01"), Some(png));
        assert_eq!(simulado.midia_exibida().as_deref(), Some("QR01"));
        assert_eq!(simulado.comandos(), ["OPN", "MLI", "MLR", "MLR", "MLR", "MLE", "DSI"]);
    }

    #[test]
    fn midia_inexistente_e_status_proprio() {
        let (mut pinpad, _) = aberto();
        assert!(matches!(pinpad.exibir_midia("NADA"), Err(ErroPinpad::Status(StatusAbecs::MFNFOUND))));
    }

    #[test]
    fn recuperacao_de_enlace_com_nak_e_crc_ruim() {
        let (mut pinpad, simulado) = aberto();
        simulado.recusar_proximos(2);
        pinpad.exibir("A", "B").expect("passa no terceiro envio");
        simulado.estragar_respostas(2);
        pinpad.exibir("C", "D").expect("o pinpad reenvia");
        simulado.recusar_proximos(3);
        assert!(matches!(pinpad.exibir("E", "F"), Err(ErroPinpad::Enlace(FalhaEnlace::NakExcedido))));
    }

    #[test]
    fn pinpad_mudo_vira_falha_e_nao_trava() {
        let (mut pinpad, simulado) = aberto();
        simulado.emudecer(true);
        assert!(matches!(pinpad.exibir("A", "B"), Err(ErroPinpad::Enlace(FalhaEnlace::SemAck))));
    }

    #[test]
    fn comando_de_senha_ou_cartao_nao_existe_no_simulador() {
        let simulado = PinpadSimulado::new();
        simulado.receber(&super::super::quadro::codificar_pacote(b"OPN").expect("opn"));
        let _ = simulado.retirar_saida();
        simulado.receber(&super::super::quadro::codificar_pacote(b"GPN000").expect("gpn"));
        let saida = simulado.retirar_saida();
        let mut sessao = Sessao::new();
        sessao.enviar(b"X", false, 0).expect("envia");
        sessao.receber(&saida, 0);
        let Some(EventoSessao::Resposta(resposta)) = sessao.proximo_evento() else { panic!("sem resposta") };
        assert_eq!(&resposta.id, b"ERR");
        assert_eq!(resposta.status, StatusAbecs::INVCALL);
    }
}
