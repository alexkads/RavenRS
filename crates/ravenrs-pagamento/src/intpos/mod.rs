//! PayGo Windows and the standard manager, through files. Only files: no `unsafe`, no library.
//!
//! The steps are always these: write `req/intpos.tmp`, rename it to `req/intpos.001`, wait for
//! `resp/intpos.sts`, which says the manager took it, then wait for `resp/intpos.001`. The manager
//! talks to the operator and the pinpad on its own window, which is why no question reaches ours.
//!
//! **Never run against a real manager, and those are Windows programs.** Only
//! [`GerenciadorSimulado`] was run. Field numbers come from the public PayGo pages and from reading
//! the behavior of an LGPL implementation; see `docs/PAGAMENTOS.md`.

mod formato;
mod simulador;

pub use formato::ArquivoIntPos;
pub use simulador::{GerenciadorSimulado, RelogioComGerenciador, RespostaGerenciador};

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dominio::{
    Aprovacao, Capacidades, EntradaOperador, IdPagamento, Passo, PedidoPagamento, ProvedorId,
    ReferenciaTransacao, TipoPagamento, Vias,
};
use crate::fiscal::{AlertaFiscal, DadosFiscaisPagamento};
use crate::provedor::{Cancelamento, ErroProvedor, ProvedorPagamento, SituacaoProvedor};
use crate::relogio::Relogio;

/// How long the manager has to write the status file, as others use it.
pub const TEMPO_STS_MS: u64 = 7_000;
/// How long to wait for a response before reporting the time out. The manager may still answer.
pub const TEMPO_RESPOSTA_MS: u64 = 300_000;
const INTERVALO_MS: u32 = 250;
/// PayGo: the interface version these fields follow.
const VERSAO_INTERFACE_PAYGO: &str = "225";
/// PayGo field 706: fixed value 4, plus 8 (separate copies) and 128 (long NSU).
const CAPACIDADES_PAYGO: &str = "140";

/// The files of the exchange. The last one is ours, and keeps an approval until its third leg.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caminhos {
    /// `req/intpos.tmp`.
    pub req_tmp: PathBuf,
    /// `req/intpos.001`.
    pub req: PathBuf,
    /// `resp/intpos.001`.
    pub resp: PathBuf,
    /// `resp/intpos.sts`.
    pub sts: PathBuf,
    /// Our note of the approval in the air, kept out of the manager's folders.
    pub pendente: PathBuf,
}

impl Caminhos {
    /// The exchange under this folder, with `req` and `resp` inside it.
    #[must_use]
    pub fn em(pasta: impl AsRef<Path>) -> Self {
        let pasta = pasta.as_ref();
        Self {
            req_tmp: pasta.join("req").join("intpos.tmp"),
            req: pasta.join("req").join("intpos.001"),
            resp: pasta.join("resp").join("intpos.001"),
            sts: pasta.join("resp").join("intpos.sts"),
            pendente: pasta.join("ravenrs-pendente.json"),
        }
    }
}

/// What differs between managers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Perfil {
    /// The plain standard manager, as in `C:\TEF_DIAL`.
    GerenciadorPadrao,
    /// PayGo Windows, as in `C:\PAYGO`, with the identification fields it asks for.
    PayGo {
        /// 716-000: who developed the automation.
        empresa: String,
        /// 735-000: the name of the application.
        aplicacao: String,
        /// 736-000: its version.
        versao: String,
        /// 738-000: the code PayGo gives when certification starts.
        registro_certificacao: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Fase {
    Status { limite: u64 },
    Resposta { limite: u64 },
}

#[derive(Clone, Debug)]
struct EmCurso {
    id: IdPagamento,
    identificacao: u64,
    tipo: TipoPagamento,
    fase: Fase,
}

/// What survives a restart: the payment and the answer of the manager.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct NotaPendente {
    id: Option<IdPagamento>,
    pix: bool,
    resposta: String,
}

/// A provider that speaks through files. The clock's wait runs in third legs and refunds.
#[derive(Debug)]
pub struct ProvedorArquivos<R> {
    caminhos: Caminhos,
    perfil: Perfil,
    relogio: R,
    em_curso: Option<EmCurso>,
    tempo_sts_ms: u64,
    tempo_resposta_ms: u64,
}

fn io_erro(erro: &io::Error) -> ErroProvedor {
    ErroProvedor::Indisponivel(format!("arquivo: {erro}"))
}

/// A number for field 001-000 that is the same for the same payment and step, also after a restart.
fn identificacao(id: &str, passo: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.bytes().chain(std::iter::once(b':')).chain(passo.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Ten digits, with no zero, which an empty field would look like.
    hash % 9_999_999_999 + 1
}

fn gravar_sincronizado(caminho: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut arquivo = fs::File::create(caminho)?;
    arquivo.write_all(bytes)?;
    arquivo.sync_all()
}

fn apagar_se_existe(caminho: &Path) -> io::Result<()> {
    match fs::remove_file(caminho) {
        Err(erro) if erro.kind() != io::ErrorKind::NotFound => Err(erro),
        _ => Ok(()),
    }
}

fn ler_se_existe(caminho: &Path) -> io::Result<Option<ArquivoIntPos>> {
    match fs::read(caminho) {
        Ok(bytes) => Ok(Some(ArquivoIntPos::ler(&bytes))),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(erro) => Err(erro),
    }
}

impl<R: Relogio> ProvedorArquivos<R> {
    /// A provider over these files. The folders are created if missing.
    pub fn new(caminhos: Caminhos, perfil: Perfil, relogio: R) -> Result<Self, ErroProvedor> {
        for caminho in [&caminhos.req, &caminhos.resp, &caminhos.pendente] {
            if let Some(pasta) = caminho.parent() {
                fs::create_dir_all(pasta).map_err(|e| io_erro(&e))?;
            }
        }
        Ok(Self {
            caminhos,
            perfil,
            relogio,
            em_curso: None,
            tempo_sts_ms: TEMPO_STS_MS,
            tempo_resposta_ms: TEMPO_RESPOSTA_MS,
        })
    }

    /// Other time limits, in milliseconds.
    #[must_use]
    pub fn com_tempos(mut self, sts_ms: u64, resposta_ms: u64) -> Self {
        self.tempo_sts_ms = sts_ms;
        self.tempo_resposta_ms = resposta_ms;
        self
    }

    /// `ATV`: whether the manager is alive. It waits at most the status limit.
    pub fn ativo(&mut self) -> Result<bool, ErroProvedor> {
        let numero = identificacao("atv", &self.relogio.agora_ms().to_string());
        let pedido = self.pedido("ATV", numero);
        match self.trocar_so_status(&pedido, numero) {
            Ok(()) => Ok(true),
            Err(ErroProvedor::Tempo) => Ok(false),
            Err(erro) => Err(erro),
        }
    }

    fn pedido(&self, comando: &str, numero: u64) -> ArquivoIntPos {
        let mut arquivo = ArquivoIntPos::novo(comando, numero);
        if let Perfil::PayGo { empresa, aplicacao, versao, registro_certificacao } = &self.perfil {
            arquivo.definir(733, 0, VERSAO_INTERFACE_PAYGO);
            arquivo.definir(738, 0, registro_certificacao.as_str());
            if !matches!(comando, "ATV" | "CNF" | "NCN") {
                arquivo.definir(706, 0, CAPACIDADES_PAYGO);
                arquivo.definir(716, 0, empresa.as_str());
                arquivo.definir(735, 0, aplicacao.as_str());
                arquivo.definir(736, 0, versao.as_str());
            }
        }
        arquivo
    }

    /// Clears what the last exchange left, writes the temporary file and renames it. The rename is
    /// what the manager sees, so it never reads half a file.
    fn publicar(&self, pedido: &ArquivoIntPos) -> Result<(), ErroProvedor> {
        if self.caminhos.req.exists() {
            return Err(ErroProvedor::Indisponivel("o gerenciador não retirou a requisição anterior".into()));
        }
        apagar_se_existe(&self.caminhos.sts).map_err(|e| io_erro(&e))?;
        apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
        gravar_sincronizado(&self.caminhos.req_tmp, &pedido.codificar()).map_err(|e| io_erro(&e))?;
        fs::rename(&self.caminhos.req_tmp, &self.caminhos.req).map_err(|e| io_erro(&e))
    }

    /// True if the status file for this number arrived; it is removed then.
    fn status_chegou(&self, numero: u64) -> Result<bool, ErroProvedor> {
        match ler_se_existe(&self.caminhos.sts).map_err(|e| io_erro(&e))? {
            Some(sts) if sts.identificacao() == Some(numero.to_string().as_str()) => {
                apagar_se_existe(&self.caminhos.sts).map_err(|e| io_erro(&e))?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn resposta_chegou(&self, numero: u64) -> Result<Option<ArquivoIntPos>, ErroProvedor> {
        match ler_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))? {
            Some(resposta)
                if resposta.completo() && resposta.identificacao() == Some(numero.to_string().as_str()) =>
            {
                Ok(Some(resposta))
            }
            _ => Ok(None),
        }
    }

    /// For commands with no response file: waits for the status, blocking up to the limit.
    fn trocar_so_status(&mut self, pedido: &ArquivoIntPos, numero: u64) -> Result<(), ErroProvedor> {
        self.publicar(pedido)?;
        let limite = self.relogio.agora_ms() + self.tempo_sts_ms;
        loop {
            if self.status_chegou(numero)? {
                return Ok(());
            }
            if self.relogio.agora_ms() >= limite {
                // Taking it back means the manager never saw it; it can simply be sent again.
                let _ = apagar_se_existe(&self.caminhos.req);
                return Err(ErroProvedor::Tempo);
            }
            self.relogio.esperar_ms(u64::from(INTERVALO_MS));
        }
    }

    /// Keeps the answer in our note before anything else, so that a crash here loses nothing.
    fn anotar(
        &self,
        id: Option<&IdPagamento>,
        pix: bool,
        resposta: &ArquivoIntPos,
    ) -> Result<(), ErroProvedor> {
        let nota = NotaPendente {
            id: id.cloned(),
            pix,
            resposta: resposta.codificar().iter().map(|b| char::from(*b)).collect(),
        };
        let bytes = serde_json::to_vec(&nota).map_err(|e| ErroProvedor::Protocolo(e.to_string()))?;
        let temporario = self.caminhos.pendente.with_extension("tmp");
        gravar_sincronizado(&temporario, &bytes).map_err(|e| io_erro(&e))?;
        fs::rename(&temporario, &self.caminhos.pendente).map_err(|e| io_erro(&e))
    }

    fn nota(&self) -> Result<Option<NotaPendente>, ErroProvedor> {
        match fs::read(&self.caminhos.pendente) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| ErroProvedor::Protocolo("nota de pendência ilegível".into())),
            Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(erro) => Err(io_erro(&erro)),
        }
    }

    fn resposta_da_nota(nota: &NotaPendente) -> ArquivoIntPos {
        let bytes: Vec<u8> =
            nota.resposta.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect();
        ArquivoIntPos::ler(&bytes)
    }

    fn terceira_perna(
        &mut self,
        comando: &str,
        referencia: &ReferenciaTransacao,
    ) -> Result<(), ErroProvedor> {
        let chave = referencia.controle.as_deref().or(referencia.nsu.as_deref()).unwrap_or("sem-controle");
        let numero = identificacao(chave, comando);
        let mut pedido = self.pedido(comando, numero);
        if let Some(rede) = &referencia.rede {
            pedido.definir(10, 0, rede.as_str());
        }
        if let Some(nsu) = &referencia.nsu {
            pedido.definir(12, 0, nsu.as_str());
        }
        if let Some(controle) = &referencia.controle {
            pedido.definir(27, 0, controle.as_str());
        }
        self.trocar_so_status(&pedido, numero)?;
        // The note only goes away when it is about this same transaction.
        let e_esta = self
            .nota()?
            .is_some_and(|nota| Self::resposta_da_nota(&nota).valor(27) == referencia.controle.as_deref());
        if e_esta {
            apagar_se_existe(&self.caminhos.pendente).map_err(|e| io_erro(&e))?;
        }
        Ok(())
    }
}

/// True when field 009-000 says it worked.
fn aprovada(resposta: &ArquivoIntPos) -> bool {
    resposta.valor(9) == Some("0")
}

fn negado(resposta: &ArquivoIntPos) -> Passo {
    Passo::Negado {
        codigo: resposta.valor(9).unwrap_or("?").to_owned(),
        motivo: resposta.valor(30).unwrap_or("Transação não aprovada").to_owned(),
    }
}

fn referencia(id: Option<&IdPagamento>, resposta: &ArquivoIntPos) -> ReferenciaTransacao {
    let mut extras = BTreeMap::new();
    for (nome, campo) in [("data", 22), ("hora", 23)] {
        if let Some(valor) = resposta.valor(campo) {
            extras.insert(nome.to_owned(), valor.to_owned());
        }
    }
    ReferenciaTransacao {
        id: id.cloned(),
        rede: resposta.valor(10).map(str::to_owned),
        nsu: resposta.valor(12).map(str::to_owned),
        controle: resposta.valor(27).map(str::to_owned),
        valor_centavos: resposta.valor(3).and_then(|valor| valor.parse().ok()).unwrap_or(0),
        extras,
    }
}

fn aprovacao(id: Option<&IdPagamento>, pix: bool, resposta: &ArquivoIntPos) -> Aprovacao {
    let mut fiscais = DadosFiscaisPagamento::integrado();
    // PayGo: 013-000 does not apply to Pix, and for Pix 012-000 returns the end-to-end id.
    let carteira = resposta.valor(749) == Some("8");
    if pix || carteira {
        fiscais.com_c_aut(resposta.valor(12), "a resposta do Pix veio sem o campo 012-000");
        if fiscais.c_aut.is_some() {
            fiscais.alertas.push(AlertaFiscal::CAutPixPeloEndToEnd);
        }
    } else {
        fiscais.com_c_aut(resposta.valor(13), "a resposta veio sem o campo 013-000");
        fiscais.com_bandeira(resposta.valor(748).or_else(|| resposta.valor(40)));
    }
    fiscais.com_cnpj_da_rede(resposta.valor(10));
    let tipo_efetivo = match resposta.valor(731) {
        _ if pix || carteira => Some(TipoPagamento::Pix),
        Some("1") => Some(TipoPagamento::Credito {
            parcelas: resposta.valor(18).and_then(|valor| valor.parse().ok()).unwrap_or(1),
        }),
        Some("2") => Some(TipoPagamento::Debito),
        Some("3") => Some(TipoPagamento::Voucher),
        _ => None,
    };
    Aprovacao {
        referencia: referencia(id, resposta),
        dados_fiscais: fiscais,
        vias: Vias {
            cliente: resposta.linhas(712, 713),
            estabelecimento: resposta.linhas(714, 715),
            unica: resposta.linhas(28, 29),
            impressas_no_terminal: false,
        },
        mensagem_operador: resposta.valor(30).map(str::to_owned),
        tipo_efetivo,
        avisos: Vec::new(),
    }
}

impl<R: Relogio> ProvedorPagamento for ProvedorArquivos<R> {
    fn id(&self) -> ProvedorId {
        match self.perfil {
            Perfil::GerenciadorPadrao => ProvedorId::GerenciadorPadrao,
            Perfil::PayGo { .. } => ProvedorId::PayGoArquivos,
        }
    }

    fn capacidades(&self) -> Capacidades {
        Capacidades::ARQUIVOS
    }

    /// Refuses to start while an approval is in the air: a new request clears the manager's files.
    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        if self.nota()?.is_some() {
            return Err(ErroProvedor::Recusado {
                codigo: "PENDENCIA".into(),
                motivo: "há uma transação sem confirmação; recupere antes".into(),
            });
        }
        let numero = identificacao(pedido.id.as_str(), "CRT");
        let mut arquivo = self.pedido("CRT", numero);
        if let Some(documento) = &pedido.documento {
            arquivo.definir(2, 0, documento.as_str());
        }
        arquivo.definir(3, 0, pedido.valor_centavos.to_string());
        arquivo.definir(4, 0, "0");
        match pedido.tipo {
            TipoPagamento::Escolher => {}
            TipoPagamento::Credito { .. } => arquivo.definir(731, 0, "1"),
            TipoPagamento::Debito => arquivo.definir(731, 0, "2"),
            TipoPagamento::Voucher => arquivo.definir(731, 0, "3"),
            TipoPagamento::Pix => arquivo.definir(749, 0, "8"),
        }
        self.publicar(&arquivo)?;
        self.em_curso = Some(EmCurso {
            id: pedido.id.clone(),
            identificacao: numero,
            tipo: pedido.tipo,
            fase: Fase::Status { limite: self.relogio.agora_ms() + self.tempo_sts_ms },
        });
        Ok(Passo::Aguardar { mensagem: Some("Aguardando o gerenciador".into()), tentar_em_ms: INTERVALO_MS })
    }

    fn continuar(&mut self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        let Some(curso) = self.em_curso.clone().filter(|curso| &curso.id == id) else {
            return Err(ErroProvedor::Protocolo(format!("{id} não está em curso neste adaptador")));
        };
        let agora = self.relogio.agora_ms();
        match curso.fase {
            Fase::Status { limite } => {
                if self.status_chegou(curso.identificacao)? {
                    let limite = agora + self.tempo_resposta_ms;
                    if let Some(atual) = self.em_curso.as_mut() {
                        atual.fase = Fase::Resposta { limite };
                    }
                } else if agora >= limite {
                    self.em_curso = None;
                    if self.caminhos.req.exists() {
                        // Taken back before the manager saw it: nothing happened, for certain.
                        apagar_se_existe(&self.caminhos.req).map_err(|e| io_erro(&e))?;
                        return Err(ErroProvedor::Recusado {
                            codigo: "GP-INATIVO".into(),
                            motivo: "o gerenciador não respondeu".into(),
                        });
                    }
                    return Err(ErroProvedor::Tempo);
                }
                Ok(Passo::Aguardar {
                    mensagem: Some("Aguardando o gerenciador".into()),
                    tentar_em_ms: INTERVALO_MS,
                })
            }
            Fase::Resposta { limite } => {
                let Some(resposta) = self.resposta_chegou(curso.identificacao)? else {
                    if agora >= limite {
                        self.em_curso = None;
                        return Err(ErroProvedor::Tempo);
                    }
                    let mensagem = if entrada == EntradaOperador::Cancelar {
                        "Cancele na janela do gerenciador"
                    } else {
                        "Conclua na janela do gerenciador"
                    };
                    return Ok(Passo::Aguardar {
                        mensagem: Some(mensagem.into()),
                        tentar_em_ms: INTERVALO_MS,
                    });
                };
                self.em_curso = None;
                let pix = curso.tipo == TipoPagamento::Pix;
                if aprovada(&resposta) {
                    self.anotar(Some(id), pix, &resposta)?;
                    apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
                    Ok(Passo::Aprovado(Box::new(aprovacao(Some(id), pix, &resposta))))
                } else {
                    apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
                    Ok(negado(&resposta))
                }
            }
        }
    }

    fn confirmar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        self.terceira_perna("CNF", referencia)
    }

    fn desfazer(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        self.terceira_perna("NCN", referencia)
    }

    /// The manager has the screen, so the operator cancels there.
    fn cancelar(&mut self, _: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        Err(ErroProvedor::SemSuporte("o cancelamento é na janela do gerenciador".into()))
    }

    /// `CNC`, then its own `CNF`. It blocks until the manager answers, up to the limit, since the
    /// operator works on the manager's window in between.
    fn estornar(&mut self, referencia: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        let nsu =
            referencia.nsu.as_deref().ok_or_else(|| ErroProvedor::Protocolo("referência sem NSU".into()))?;
        let numero = identificacao(nsu, "CNC");
        let mut pedido = self.pedido("CNC", numero);
        pedido.definir(3, 0, referencia.valor_centavos.to_string());
        pedido.definir(4, 0, "0");
        pedido.definir(12, 0, nsu);
        if let Some(rede) = &referencia.rede {
            pedido.definir(10, 0, rede.as_str());
        }
        for (nome, campo) in [("data", 22), ("hora", 23)] {
            if let Some(valor) = referencia.extras.get(nome) {
                pedido.definir(campo, 0, valor.as_str());
            }
        }
        self.publicar(&pedido)?;
        let limite = self.relogio.agora_ms() + self.tempo_resposta_ms;
        let resposta = loop {
            if let Some(resposta) = self.resposta_chegou(numero)? {
                break resposta;
            }
            if self.relogio.agora_ms() >= limite {
                return Err(ErroProvedor::Tempo);
            }
            self.relogio.esperar_ms(u64::from(INTERVALO_MS));
        };
        let _ = self.status_chegou(numero);
        apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
        if !aprovada(&resposta) {
            return Err(ErroProvedor::Recusado {
                codigo: resposta.valor(9).unwrap_or("?").to_owned(),
                motivo: resposta.valor(30).unwrap_or("Cancelamento negado").to_owned(),
            });
        }
        let do_estorno = self::referencia(referencia.id.as_ref(), &resposta);
        self.terceira_perna("CNF", &do_estorno)
    }

    fn consultar(&mut self, id: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        if self.em_curso.as_ref().is_some_and(|curso| &curso.id == id) {
            return Ok(SituacaoProvedor::EmAndamento);
        }
        if let Some(nota) = self.nota()?.filter(|nota| nota.id.as_ref() == Some(id)) {
            let resposta = Self::resposta_da_nota(&nota);
            return Ok(SituacaoProvedor::Aprovada(Box::new(aprovacao(Some(id), nota.pix, &resposta))));
        }
        let numero = identificacao(id.as_str(), "CRT");
        if let Some(resposta) = self.resposta_chegou(numero)? {
            if aprovada(&resposta) {
                self.anotar(Some(id), false, &resposta)?;
                apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
                return Ok(SituacaoProvedor::Aprovada(Box::new(aprovacao(Some(id), false, &resposta))));
            }
            apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
            return Ok(SituacaoProvedor::Negada {
                codigo: resposta.valor(9).unwrap_or("?").to_owned(),
                motivo: resposta.valor(30).unwrap_or("Transação não aprovada").to_owned(),
            });
        }
        if let Some(pedido) = ler_se_existe(&self.caminhos.req).map_err(|e| io_erro(&e))?
            && pedido.identificacao() == Some(numero.to_string().as_str())
        {
            // The manager never took it. Taking it back settles the matter.
            apagar_se_existe(&self.caminhos.req).map_err(|e| io_erro(&e))?;
            return Ok(SituacaoProvedor::Desconhecida);
        }
        // It may be with the manager, on its window, and no file tells that.
        Ok(SituacaoProvedor::Indeterminada)
    }

    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        if let Some(nota) = self.nota()? {
            let resposta = Self::resposta_da_nota(&nota);
            return Ok(vec![referencia(nota.id.as_ref(), &resposta)]);
        }
        // An approved response nobody took, from an earlier session.
        if let Some(resposta) = ler_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?
            && resposta.completo()
            && aprovada(&resposta)
            && resposta.comando() == Some("CRT")
        {
            self.anotar(None, false, &resposta)?;
            apagar_se_existe(&self.caminhos.resp).map_err(|e| io_erro(&e))?;
            return Ok(vec![referencia(None, &resposta)]);
        }
        Ok(Vec::new())
    }
}
