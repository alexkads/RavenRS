//! What only a server can check: what it already holds. The rest comes from `analise`.

use std::collections::{HashMap, HashSet};

use super::analise::{self, Analise, Diagnostico, Opcoes};
use super::catalogo::regra;
use super::compras;
use super::resposta::{self, DadosEvento, DadosProtocolo};
use super::tempo::Instante;
use super::{Resposta, SandboxRequest, SefazSandbox};
use crate::chave::{ChaveAcesso, cnpj_valido, forma_valida};
use crate::documento::{TP_EVENTO_CANCELAMENTO, TipoManifestacao};
use crate::retorno::descricao_c_stat;
use crate::soap::{NS_NFE, NS_SOAP12, Servico};
use crate::xml::{self, Element};

/// The prazo of a cancellation, by rule 2P12-14: 24 hours.
const PRAZO_CANCELAMENTO: i64 = 24 * 3600;
/// How long to wait after nothing was found, by NT 2014.002, item 3.11.4.
const ESPERA_DISTRIBUICAO: i64 = 3600;
/// The tolerance of rule 2P13-14, in seconds.
const TOLERANCIA_DATA: i64 = 300;
/// The largest batch of the distribution, by NT 2014.002.
const MAXIMO_LOTE: usize = 50;

/// What the sandbox knows about a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EstadoChave {
    /// The sandbox has never seen it.
    Desconhecida,
    /// It was refused and therefore does not exist for the SEFAZ.
    Rejeitada {
        /// The last refusal.
        c_stat: u32,
    },
    /// Authorized.
    Autorizada {
        /// The protocol.
        n_prot: String,
    },
    /// Authorized, but the sender never got the answer: the dangerous case.
    AutorizadaSemRetorno {
        /// The protocol that exists and the sender does not know.
        n_prot: String,
    },
    /// Cancelled.
    Cancelada {
        /// The protocol of the authorization.
        n_prot: String,
    },
}

/// One document the sandbox already authorized.
#[derive(Clone, Debug)]
struct Nota {
    n_prot: String,
    dig_val: String,
    dh: String,
    emitida_em: Option<Instante>,
    autorizada_em: Option<Instante>,
    cancelada: bool,
}

/// One document in the distribution.
#[derive(Clone, Debug)]
struct Entrada {
    nsu: u64,
    destinatario: String,
    schema: String,
    xml: String,
}

/// A document waiting for the recipient.
#[derive(Clone, Debug)]
struct Recebida {
    destinatario: String,
    proc_nfe: String,
    resumo: String,
    manifestacoes: HashSet<String>,
    liberada: bool,
}

/// What the sandbox remembers.
#[derive(Debug, Default)]
pub(crate) struct Notas {
    por_chave: HashMap<String, Nota>,
    /// Issuer, model, series and number to the key that took them.
    por_numero: HashMap<(String, String, String, String), String>,
    rejeitadas: HashMap<String, u32>,
    sem_retorno: HashSet<String>,
    recebidas: HashMap<String, Recebida>,
    entradas: Vec<Entrada>,
    /// For each recipient: the NSU handed over last, and when nothing new came.
    consultas: HashMap<String, Consulta>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Consulta {
    ultimo_entregue: u64,
    sem_novidade_em: Option<Option<Instante>>,
}

impl Notas {
    pub(super) fn estado(&self, chave: &str) -> EstadoChave {
        if let Some(nota) = self.por_chave.get(chave) {
            let n_prot = nota.n_prot.clone();
            if nota.cancelada {
                return EstadoChave::Cancelada { n_prot };
            }
            if self.sem_retorno.contains(chave) {
                return EstadoChave::AutorizadaSemRetorno { n_prot };
            }
            return EstadoChave::Autorizada { n_prot };
        }
        match self.rejeitadas.get(chave) {
            Some(c_stat) => EstadoChave::Rejeitada { c_stat: *c_stat },
            None => EstadoChave::Desconhecida,
        }
    }

    /// Records that the sender never got the answer about this key.
    pub(super) fn marcar_sem_retorno(&mut self, chave: &str) {
        self.sem_retorno.insert(chave.to_string());
    }

    /// Records that the sender saw an answer about this key.
    pub(super) fn marcar_com_retorno(&mut self, chave: &str) {
        self.sem_retorno.remove(chave);
    }

    pub(super) fn registrar_rejeicao(&mut self, chave: &str, c_stat: u32) {
        if !self.por_chave.contains_key(chave) {
            self.rejeitadas.insert(chave.to_string(), c_stat);
        }
    }

    fn proximo_nsu(&self) -> u64 {
        self.entradas.iter().map(|e| e.nsu).max().unwrap_or(0) + 1
    }

    /// Puts a document in the distribution of its recipient, as a summary.
    pub(super) fn colocar(&mut self, chave: String, destinatario: String, proc_nfe: String, resumo: String) {
        let nsu = self.proximo_nsu();
        self.entradas.push(Entrada {
            nsu,
            destinatario: destinatario.clone(),
            schema: "resNFe_v1.01.xsd".to_string(),
            xml: resumo.clone(),
        });
        self.recebidas.insert(
            chave,
            Recebida { destinatario, proc_nfe, resumo, manifestacoes: HashSet::new(), liberada: false },
        );
    }
}

fn namespace_proprio(element: &Element) -> Option<&str> {
    match element.name.split_once(':') {
        Some((prefixo, _)) => element.attr(&format!("xmlns:{prefixo}")),
        None => element.attr("xmlns"),
    }
}

/// Checks the envelope and gives back the message inside it.
fn abrir_envelope(pedido: &SandboxRequest, servico: Servico) -> Result<Element, Resposta> {
    let tipo = pedido.http.header("Content-Type").unwrap_or("");
    if !tipo.to_ascii_lowercase().starts_with("application/soap+xml") {
        return Err(Resposta::Http(resposta::fault(415, "Content-Type não é SOAP 1.2")));
    }
    let envelope = xml::parse(pedido.body()).map_err(|_| pedido.responder(243))?;
    if envelope.local_name() != "Envelope" || namespace_proprio(&envelope) != Some(NS_SOAP12) {
        return Err(Resposta::Http(resposta::fault(400, "Envelope não é SOAP 1.2")));
    }
    let corpo =
        envelope.child("Body").ok_or_else(|| Resposta::Http(resposta::fault(400, "Envelope sem Body")))?;
    let mensagem = corpo
        .find("nfeDadosMsg")
        .ok_or_else(|| Resposta::Http(resposta::fault(400, "Body sem nfeDadosMsg")))?;
    let namespace = if servico == Servico::Distribuicao {
        corpo.child(servico.operacao()).and_then(|grupo| grupo.attr("xmlns"))
    } else {
        mensagem.attr("xmlns")
    };
    if namespace != Some(servico.namespace().as_str()) {
        return Err(Resposta::Http(resposta::fault(400, "nfeDadosMsg com namespace diferente da ação")));
    }
    let mut filhos = mensagem.elements();
    let dados = filhos.next().ok_or_else(|| pedido.responder(225))?;
    if filhos.next().is_some() {
        return Err(pedido.responder(225));
    }
    Ok(dados.clone())
}

/// Answers the first refusal of an analysis, if there is one, as a server would.
fn responder_rejeicao(pedido: &SandboxRequest, analise: &Analise) -> Option<(Resposta, u32)> {
    let achado = analise.primeira_rejeicao()?;
    Some((responder_achado(pedido, achado), achado.c_stat))
}

fn responder_achado(pedido: &SandboxRequest, achado: &Diagnostico) -> Resposta {
    let mut texto = achado.titulo.clone();
    if achado.regra == "B24-10" || achado.regra == "J01" {
        texto.push_str(". O sandbox só aceita tpAmb=2");
    }
    if matches!(
        achado.regra,
        "E01"
            | "E02"
            | "E03"
            | "E04"
            | "E06"
            | "A01"
            | "F03"
            | "SBX-FORA-DO-LABORATORIO"
            | "CERT-CHAVE-FRACA"
            | "CERT-SHA1"
    ) {
        texto.push_str(": ");
        texto.push_str(achado.explicacao.split(". ").next().unwrap_or(""));
    }
    if achado.c_stat == 0 {
        return Resposta::Http(resposta::fault(400, &achado.explicacao));
    }
    pedido.responder_com(achado.c_stat, &texto)
}

fn por_regra(pedido: &SandboxRequest, id: &str, detalhe: Option<&str>) -> Resposta {
    let Some(regra) = regra(id) else {
        return pedido.responder(999);
    };
    match detalhe {
        Some(detalhe) => pedido.responder_com(regra.c_stat, &format!("{} {detalhe}", regra.titulo())),
        None => pedido.responder_com(regra.c_stat, regra.titulo()),
    }
}

/// Runs the checks of the service of this request.
pub(super) fn conferir(pedido: &SandboxRequest, sandbox: &SefazSandbox) -> Resposta {
    let Some(servico) = pedido.servico else {
        return Resposta::Proximo;
    };
    let dados = match abrir_envelope(pedido, servico) {
        Ok(dados) => dados,
        Err(resposta) => return resposta,
    };
    if dados.attr("xmlns") != Some(NS_NFE) {
        return por_regra(pedido, "D01-LOTE", None);
    }
    let opcoes = sandbox.opcoes();
    match servico {
        Servico::Status => match responder_rejeicao(pedido, &analise::analisar_status(&dados, &opcoes)) {
            Some((resposta, _)) => resposta,
            None => pedido.responder(107),
        },
        Servico::Autorizacao => conferir_autorizacao(pedido, sandbox, &dados, &opcoes),
        Servico::Consulta => conferir_consulta(pedido, sandbox, &dados, &opcoes),
        Servico::Evento => conferir_lote_evento(pedido, sandbox, &dados, &opcoes),
        Servico::Distribuicao => conferir_distribuicao(pedido, sandbox, &dados, &opcoes),
    }
}

fn conferir_autorizacao(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    dados: &Element,
    opcoes: &Opcoes,
) -> Resposta {
    let analise = analise::analisar_lote(dados, Some(pedido.body()), opcoes);
    if let Some((resposta, c_stat)) = responder_rejeicao(pedido, &analise) {
        if let Some(chave) = &analise.chave {
            sandbox.lock().notas.registrar_rejeicao(chave, c_stat);
        }
        return resposta;
    }
    let (Some(chave), Some(nfe)) = (analise.chave.as_deref(), dados.child("NFe")) else {
        return por_regra(pedido, "D01-LOTE", None);
    };
    let Some(inf) = nfe.child("infNFe") else {
        return por_regra(pedido, "D01-LOTE", None);
    };
    registrar_nota(pedido, sandbox, nfe, inf, chave, opcoes)
}

fn registrar_nota(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    nfe: &Element,
    inf: &Element,
    chave: &str,
    opcoes: &Opcoes,
) -> Resposta {
    let ide = inf.child("ide");
    let campo = |tag: &str| ide.and_then(|i| i.child_text(tag)).unwrap_or_default();
    let cnpj = inf.child("emit").and_then(|e| e.child_text("CNPJ")).unwrap_or_default();
    let numero_chave = (cnpj, campo("mod"), campo("serie"), campo("nNF"));
    let dig_val = nfe.find_text("DigestValue").unwrap_or_default();
    let dh = pedido.dh_recbto.as_str();

    let mut estado = sandbox.lock();
    if let Some(nota) = estado.notas.por_chave.get(chave) {
        let motivo = format!("{} [nProt:{}]", texto_de(204), nota.n_prot);
        drop(estado);
        return pedido.responder_com(204, &motivo);
    }
    if let Some(existente) = estado.notas.por_numero.get(&numero_chave) {
        let motivo = format!("{} [chNFe:{existente}]", texto_de(539));
        drop(estado);
        return pedido.responder_com(539, &motivo);
    }
    let n_prot = pedido.proximo_protocolo();
    estado.notas.por_numero.insert(numero_chave, chave.to_string());
    estado.notas.rejeitadas.remove(chave);
    estado.notas.por_chave.insert(
        chave.to_string(),
        Nota {
            n_prot: n_prot.clone(),
            dig_val: dig_val.clone(),
            dh: dh.to_string(),
            emitida_em: Instante::de_xml(&campo("dhEmi")),
            autorizada_em: opcoes.agora,
            cancelada: false,
        },
    );
    drop(estado);

    let prot = resposta::prot_nfe(&DadosProtocolo {
        chave,
        dh,
        n_prot: Some(&n_prot),
        dig_val: Some(&dig_val),
        c_stat: 100,
        motivo: texto_de(100),
    });
    let retorno = resposta::ret_lote(104, texto_de(104), &campo("cUF"), dh, Some(&prot));
    Resposta::Http(resposta::soap(Servico::Autorizacao, &retorno))
}

fn texto_de(c_stat: u32) -> &'static str {
    descricao_c_stat(c_stat).unwrap_or("")
}

fn conferir_consulta(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    dados: &Element,
    opcoes: &Opcoes,
) -> Resposta {
    let analise = analise::analisar_consulta(dados, opcoes);
    if let Some((resposta, _)) = responder_rejeicao(pedido, &analise) {
        return resposta;
    }
    let Some(chave) = analise.chave else {
        return por_regra(pedido, "D01", None);
    };
    let nota = sandbox.lock().notas.por_chave.get(&chave).cloned();
    let Some(nota) = nota else {
        return por_regra(pedido, "J03", None);
    };
    let c_uf = chave.get(..2).unwrap_or("");
    let (c_stat, texto) = if nota.cancelada { (101, texto_de(101)) } else { (100, texto_de(100)) };
    let prot = resposta::prot_nfe(&DadosProtocolo {
        chave: &chave,
        dh: &nota.dh,
        n_prot: Some(&nota.n_prot),
        dig_val: Some(&nota.dig_val),
        c_stat: 100,
        motivo: texto_de(100),
    });
    let retorno = resposta::ret_consulta(c_stat, texto, c_uf, &pedido.dh_recbto, &chave, Some(&prot));
    Resposta::Http(resposta::soap(Servico::Consulta, &retorno))
}

fn conferir_lote_evento(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    dados: &Element,
    opcoes: &Opcoes,
) -> Resposta {
    if dados.local_name() != "envEvento" {
        return por_regra(pedido, "D01a", None);
    }
    if dados.attr("versao") != Some("1.00") {
        return por_regra(pedido, "B06", None);
    }
    let eventos: Vec<&Element> = dados.elements().filter(|e| e.local_name() == "evento").collect();
    let [evento] = eventos.as_slice() else {
        return pedido.responder_com(215, "O sandbox só aceita lote com um evento");
    };
    let analise = analise::analisar_evento(evento, Some(pedido.body()), opcoes);
    if let Some((resposta, _)) = responder_rejeicao(pedido, &analise) {
        return resposta;
    }
    let Some(inf) = evento.child("infEvento") else {
        return por_regra(pedido, "D01", None);
    };
    let campo = |tag: &str| inf.child_text(tag).unwrap_or_default();
    let chave = campo("chNFe");
    let tp_evento = campo("tpEvento");
    let negada = if tp_evento == TP_EVENTO_CANCELAMENTO {
        cancelar(sandbox, inf, &chave, opcoes)
    } else {
        manifestar(sandbox, &chave, &campo("CNPJ"), &tp_evento)
    };
    if let Some(id) = negada {
        return por_regra(pedido, id, None);
    }
    let id_lote = dados.child_text("idLote").unwrap_or_default();
    let c_orgao = campo("cOrgao");
    let dh = pedido.dh_recbto.as_str();
    let n_prot = pedido.proximo_protocolo();
    let retorno_evento = resposta::ret_evento(&DadosEvento {
        c_orgao: &c_orgao,
        c_stat: 135,
        motivo: texto_de(135),
        chave: &chave,
        tp_evento: &tp_evento,
        sequencia: &campo("nSeqEvento"),
        dh,
        n_prot: Some(&n_prot),
    });
    let retorno = resposta::ret_lote_evento(&id_lote, 128, texto_de(128), &c_orgao, Some(&retorno_evento));
    Resposta::Http(resposta::soap(Servico::Evento, &retorno))
}

/// The rules of a cancellation that depend on the stored document, in the order of Table 5-38.
fn cancelar(sandbox: &SefazSandbox, inf: &Element, chave: &str, opcoes: &Opcoes) -> Option<&'static str> {
    let protocolo = inf.child("detEvento").and_then(|d| d.child_text("nProt")).unwrap_or_default();
    let momento = inf.child_text("dhEvento").and_then(|d| Instante::de_xml(&d));
    let mut estado = sandbox.lock();
    let Some(nota) = estado.notas.por_chave.get_mut(chave) else {
        return Some("2P12-10");
    };
    if let (Some(agora), Some(autorizada)) = (opcoes.agora, nota.autorizada_em)
        && agora.0 - autorizada.0 > PRAZO_CANCELAMENTO
        && !nota.cancelada
    {
        return Some("2P12-14");
    }
    if nota.cancelada {
        return Some("2P12-22");
    }
    if let (Some(momento), Some(emitida)) = (momento, nota.emitida_em)
        && momento < emitida
    {
        return Some("2P13-10");
    }
    if let (Some(momento), Some(autorizada)) = (momento, nota.autorizada_em)
        && momento.0 + TOLERANCIA_DATA < autorizada.0
    {
        return Some("2P13-14");
    }
    if nota.n_prot != protocolo {
        return Some("2P23-10");
    }
    nota.cancelada = true;
    None
}

/// The recipient's manifestation, which releases the full document.
fn manifestar(sandbox: &SefazSandbox, chave: &str, autor: &str, tp_evento: &str) -> Option<&'static str> {
    let mut estado = sandbox.lock();
    let notas = &mut estado.notas;
    let nsu = notas.proximo_nsu();
    let Some(recebida) = notas.recebidas.get_mut(chave) else {
        return Some("MANIF-494");
    };
    if recebida.destinatario.get(..8) != autor.get(..8) {
        return Some("MANIF-575");
    }
    if !recebida.manifestacoes.insert(tp_evento.to_string()) {
        return Some("3P15-10");
    }
    // By NT 2014.002, item 7: every manifestation but "desconhecimento" releases the download.
    if tp_evento != TipoManifestacao::Desconhecimento.codigo() && !recebida.liberada {
        recebida.liberada = true;
        let entrada = Entrada {
            nsu,
            destinatario: recebida.destinatario.clone(),
            schema: "procNFe_v4.00.xsd".to_string(),
            xml: recebida.proc_nfe.clone(),
        };
        notas.entradas.push(entrada);
    }
    None
}

fn conferir_distribuicao(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    dados: &Element,
    opcoes: &Opcoes,
) -> Resposta {
    if dados.local_name() != "distDFeInt" {
        return por_regra(pedido, "D01a", None);
    }
    if dados.child_text("tpAmb").as_deref() != opcoes.tp_amb_esperado.as_deref() {
        return por_regra(pedido, "DIST-252", None);
    }
    let cnpj = dados.child_text("CNPJ").unwrap_or_default();
    if !cnpj_valido(&cnpj) {
        return por_regra(pedido, "DIST-489", None);
    }
    let dh = pedido.dh_recbto.as_str();
    if let Some(grupo) = dados.child("consChNFe") {
        return distribuir_por_chave(pedido, sandbox, grupo, &cnpj);
    }
    let Some(grupo) = dados.child("distNSU") else {
        return por_regra(pedido, "D01", None);
    };
    let ultimo: u64 = grupo.child_text("ultNSU").and_then(|t| t.parse().ok()).unwrap_or(0);

    let mut estado = sandbox.lock();
    let notas = &mut estado.notas;
    let chave_consulta = cnpj.get(..8).unwrap_or("").to_string();
    let maior = notas
        .entradas
        .iter()
        .filter(|e| e.destinatario.get(..8) == Some(chave_consulta.as_str()))
        .map(|e| e.nsu)
        .max()
        .unwrap_or(0);
    let consulta = notas.consultas.entry(chave_consulta.clone()).or_default();
    if ultimo > maior && maior > 0 {
        drop(estado);
        return por_regra(pedido, "DIST-589", None);
    }
    if ultimo < consulta.ultimo_entregue {
        let entregue = consulta.ultimo_entregue;
        drop(estado);
        let retorno = resposta::ret_distribuicao(
            656,
            &format!("{}. Deve ser utilizado o ultNSU nas solicitacoes subsequentes", texto_de(656)),
            dh,
            entregue,
            maior,
            &[],
        );
        return Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno));
    }
    let documentos: Vec<(u64, String, String)> = notas
        .entradas
        .iter()
        .filter(|e| e.nsu > ultimo && e.destinatario.get(..8) == Some(chave_consulta.as_str()))
        .take(MAXIMO_LOTE)
        .map(|e| (e.nsu, e.schema.clone(), e.xml.clone()))
        .collect();
    if documentos.is_empty() {
        let bloqueado = match (consulta.sem_novidade_em, opcoes.agora) {
            (Some(Some(antes)), Some(agora)) => agora.0 - antes.0 < ESPERA_DISTRIBUICAO,
            (Some(_), _) => true,
            (None, _) => false,
        };
        if bloqueado {
            drop(estado);
            let retorno = resposta::ret_distribuicao(
                656,
                &format!("{}. Aguarde 1 hora quando nao houver mais documentos", texto_de(656)),
                dh,
                ultimo,
                maior,
                &[],
            );
            return Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno));
        }
        consulta.sem_novidade_em = Some(opcoes.agora);
        drop(estado);
        let retorno = resposta::ret_distribuicao(137, texto_de(137), dh, ultimo.max(maior), maior, &[]);
        return Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno));
    }
    let ult = documentos.last().map_or(ultimo, |d| d.0);
    consulta.ultimo_entregue = ult;
    consulta.sem_novidade_em = None;
    drop(estado);
    let retorno = resposta::ret_distribuicao(138, texto_de(138), dh, ult, maior, &documentos);
    Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno))
}

fn distribuir_por_chave(
    pedido: &SandboxRequest,
    sandbox: &SefazSandbox,
    grupo: &Element,
    cnpj: &str,
) -> Resposta {
    let chave = grupo.child_text("chNFe").unwrap_or_default();
    if !forma_valida(&chave) || ChaveAcesso::parse(&chave).is_err() {
        return por_regra(pedido, "DIST-236", None);
    }
    let dh = pedido.dh_recbto.as_str();
    let estado = sandbox.lock();
    let Some(recebida) = estado.notas.recebidas.get(&chave) else {
        drop(estado);
        let retorno = resposta::ret_distribuicao(137, texto_de(137), dh, 0, 0, &[]);
        return Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno));
    };
    if recebida.destinatario.get(..8) != cnpj.get(..8) {
        drop(estado);
        return por_regra(pedido, "DIST-640", None);
    }
    let documento = if recebida.liberada {
        (0, "procNFe_v4.00.xsd".to_string(), recebida.proc_nfe.clone())
    } else {
        (0, "resNFe_v1.01.xsd".to_string(), recebida.resumo.clone())
    };
    drop(estado);
    let retorno = resposta::ret_distribuicao(138, texto_de(138), dh, 0, 0, &[documento]);
    Resposta::Http(resposta::soap(Servico::Distribuicao, &retorno))
}

/// Puts a purchase in the distribution of its recipient. Fails if the XML has no recipient CNPJ.
pub(super) fn colocar_nota(sandbox: &SefazSandbox, xml_texto: &str) -> Result<(), crate::NfeError> {
    let (chave, destinatario, proc_nfe, resumo) = compras::separar(xml_texto)?;
    sandbox.lock().notas.colocar(chave, destinatario, proc_nfe, resumo);
    Ok(())
}
