//! A sandbox of the Sefin Nacional, in the style of MSW: it implements [`Transport`], so the
//! emitter builds the same bytes and calls the same addresses, and nothing leaves the computer.
//!
//! | MSW | Here |
//! |---|---|
//! | `http.post('/nfse', resolver)` | [`post`], [`get`], [`head`] with a [`Rota`] |
//! | `server.use(handler)` | [`SefinSandbox::use_handler`] |
//! | `{ once: true }` | [`SefinSandbox::use_once`] |
//! | `server.resetHandlers()` | [`SefinSandbox::reset_handlers`] |
//! | `onUnhandledRequest` | [`SefinSandbox::on_unhandled`] |
//!
//! With no handler, it answers as the Sefin would, after checking for real: GZip and Base64, the
//! namespace and version, the Id against the fields it is made of, `tpAmb=2`, the signature with
//! the hash it was made with, a DPS already used, and the agreement of the city. Each refusal comes
//! with its code, where the code came from and how sure that is ([`regra`]).
//!
//! The NFS-e it gives back is a reduced one: `infNFSe` with the Id, the number, the date of
//! processing and the DPS received. It does not follow the whole `TCInfNFSe`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use ravenrs_dfe::assinatura::{FalhaAssinatura, verificar_envelopado};
use ravenrs_dfe::compactacao::{de_gzip_base64, gzip_base64};
use ravenrs_dfe::interceptacao::Rota;
use ravenrs_dfe::xml::{self, Element, escape_text};
use ravenrs_dfe::{
    Cadeia, HashAssinatura, HttpRequest, HttpResponse, Metodo, OnUnhandled, Transport, TransportError,
};
use serde_json::{Value, json};

use crate::ambiente::{NS_NFSE, VERSAO};
use crate::chave::dv_modulo_11;
use crate::evento::CANCELAMENTO;

/// `versaoAplicativo` of every answer of the sandbox.
pub const VER_APLIC: &str = "RAVENRS-SANDBOX";
/// The start of every description the sandbox writes.
pub const PREFIXO: &str = "[SANDBOX] ";

/// How sure a rule of the sandbox is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Confianca {
    /// Read in an official document or schema.
    Oficial,
    /// From the code or the catalog of third parties; the official table was not read.
    Terceiros,
    /// A rule of the sandbox itself, with no code of the Sefin behind it.
    DoSandbox,
}

/// A rule the sandbox applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regra {
    /// The code answered.
    pub codigo: &'static str,
    /// What it means.
    pub descricao: &'static str,
    /// How to fix it, for whoever is in front of the screen.
    pub como_corrigir: &'static str,
    /// Where it came from.
    pub fonte: &'static str,
    /// How sure that is.
    pub confianca: Confianca,
}

/// What the sandbox knows how to refuse.
pub const REGRAS: &[Regra] = &[
    Regra {
        codigo: "E0004",
        descricao: "O Id da DPS não confere com os campos de que é feito.",
        como_corrigir: "Monte o Id com município, tipo de inscrição, inscrição, série e número da própria DPS.",
        fonte: "Catálogo de erros da Berga; composição do Id no XSD v1.01",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0006",
        descricao: "O ambiente da DPS não é o do serviço que a recebeu.",
        como_corrigir: "Na produção restrita e no sandbox, tpAmb é sempre 2.",
        fonte: "Catálogo de erros da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0008",
        descricao: "A data de emissão é posterior ao processamento.",
        como_corrigir: "Confira o relógio do computador e o fuso de dhEmi.",
        fonte: "Catálogo de erros da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0014",
        descricao: "Já existe uma DPS com este Id.",
        como_corrigir: "Não troque o número: consulte /dps/{id}, que devolve a chave da NFS-e já gerada.",
        fonte: "Catálogo de erros da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0037",
        descricao: "O município de emissão não aderiu ao padrão nacional.",
        como_corrigir: "Consulte o convênio do município antes; sem convênio, a emissão é no sistema da prefeitura.",
        fonte: "Catálogo de erros da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0714",
        descricao: "Assinatura inválida.",
        como_corrigir: "Assine infDPS com Signature dentro de DPS, sem prefixo; não altere o XML depois; tire CR de xDescServ.",
        fonte: "Fórum NFS-e Brasil e catálogo da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "E0840",
        descricao: "A NFS-e já está cancelada.",
        como_corrigir: "Nada a fazer: consulte a NFS-e para ver o evento que já existe.",
        fonte: "Catálogo de erros da Berga",
        confianca: Confianca::Terceiros,
    },
    Regra {
        codigo: "SANDBOX-CONTEUDO",
        descricao: "O conteúdo não é JSON com o documento em GZip + Base64.",
        como_corrigir: "Envie {\"dpsXmlGZipB64\": ...} com o XML em UTF-8 compactado.",
        fonte: "Manual Integrado V1.00.02 (formato); nomes dos campos do ACBr e do nfse-php",
        confianca: Confianca::DoSandbox,
    },
    Regra {
        codigo: "SANDBOX-LEIAUTE",
        descricao: "O XML não é uma DPS da versão 1.01 no namespace do padrão nacional.",
        como_corrigir: "Raiz DPS, xmlns http://www.sped.fazenda.gov.br/nfse, versao 1.00 ou 1.01.",
        fonte: "XSD v1.01",
        confianca: Confianca::Oficial,
    },
    Regra {
        codigo: "SANDBOX-NAO-ENCONTRADA",
        descricao: "Não há documento com este identificador.",
        como_corrigir: "Confira o Id ou a chave.",
        fonte: "Comportamento do sandbox",
        confianca: Confianca::DoSandbox,
    },
];

/// The rule of a code.
#[must_use]
pub fn regra(codigo: &str) -> Option<&'static Regra> {
    REGRAS.iter().find(|regra| regra.codigo == codigo)
}

/// What a handler decides.
#[derive(Clone, Debug)]
pub enum Resposta {
    /// Answers this.
    Http(HttpResponse),
    /// Behaves as if the time limit ran out, with nothing processed.
    Timeout,
    /// Processes the request as usual and then loses the answer: the NFS-e exists, the sender
    /// does not know. This is what recovery by the Id is for.
    ProcessarEPerder,
    /// Fails with this error.
    Erro(TransportError),
    /// Leaves it to the next handler, and in the end to the default behavior.
    Proximo,
}

/// What a handler receives.
#[derive(Clone, Debug)]
pub struct PedidoSefin {
    /// The request as sent.
    pub http: HttpRequest,
    /// The parts of the route that vary, by name.
    pub parametros: Vec<(String, String)>,
}

impl PedidoSefin {
    /// A part of the route, by its name.
    #[must_use]
    pub fn parametro(&self, nome: &str) -> Option<&str> {
        self.parametros.iter().find(|(chave, _)| chave == nome).map(|(_, valor)| valor.as_str())
    }
}

/// A rule that answers a kind of request.
#[derive(Clone)]
pub struct Handler {
    rota: Rota,
    responder: Arc<dyn Fn(&PedidoSefin) -> Resposta + Send + Sync>,
}

impl std::fmt::Debug for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handler").field("rota", &self.rota.to_string()).finish_non_exhaustive()
    }
}

fn handler(
    metodo: Metodo,
    caminho: &str,
    f: impl Fn(&PedidoSefin) -> Resposta + Send + Sync + 'static,
) -> Handler {
    Handler { rota: Rota::new(metodo, caminho), responder: Arc::new(f) }
}

/// Handles a POST on this path, as `http.post` in MSW.
pub fn post(caminho: &str, f: impl Fn(&PedidoSefin) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Metodo::Post, caminho, f)
}

/// Handles a GET on this path.
pub fn get(caminho: &str, f: impl Fn(&PedidoSefin) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Metodo::Get, caminho, f)
}

/// Handles a HEAD on this path.
pub fn head(caminho: &str, f: impl Fn(&PedidoSefin) -> Resposta + Send + Sync + 'static) -> Handler {
    handler(Metodo::Head, caminho, f)
}

/// Ready-made situations of production.
pub mod cenarios {
    use super::*;

    /// The DPS becomes an NFS-e, and the answer is lost on the way.
    #[must_use]
    pub fn resposta_perdida() -> Handler {
        post("/nfse", |_| Resposta::ProcessarEPerder)
    }

    /// The time runs out before anything is processed.
    #[must_use]
    pub fn timeout() -> Handler {
        post("/nfse", |_| Resposta::Timeout)
    }

    /// The Sefin is down: 503 on every send.
    #[must_use]
    pub fn indisponivel() -> Handler {
        post("/nfse", |_| {
            Resposta::Http(json_resposta(
                503,
                &json!({ "erro": { "codigo": "503", "descricao": format!("{PREFIXO}Serviço indisponível") } }),
            ))
        })
    }

    /// Refuses the send with this code; the text comes from [`regra`] when there is one.
    #[must_use]
    pub fn recusar(codigo: &'static str) -> Handler {
        post("/nfse", move |_| Resposta::Http(recusa(400, codigo, None)))
    }

    /// The mutual TLS refused the certificate: HTTP 403, as the Sefin answers it.
    #[must_use]
    pub fn certificado_recusado() -> Handler {
        post("/nfse", |_| Resposta::Http(HttpResponse { status: 403, headers: Vec::new(), body: Vec::new() }))
    }
}

/// A request the sandbox received, and what it answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registro {
    /// The request as sent.
    pub http: HttpRequest,
    /// The status answered, if an answer went back.
    pub status: Option<u16>,
    /// The codes answered.
    pub codigos: Vec<String>,
}

#[derive(Clone, Debug)]
struct Nota {
    chave: String,
    xml: String,
    cancelada: bool,
}

struct Estado {
    handlers: Cadeia<Handler>,
    on_unhandled: OnUnhandled,
    registros: Vec<Registro>,
    avisos: Vec<String>,
    por_dps: HashMap<String, Nota>,
    dps_da_chave: HashMap<String, String>,
    proximo_numero: u64,
    sem_convenio: Vec<String>,
    aceitos: Vec<HashAssinatura>,
    agora: String,
}

/// The sandbox of the Sefin Nacional. A clone is the same sandbox.
#[derive(Clone)]
pub struct SefinSandbox {
    estado: Arc<Mutex<Estado>>,
}

impl std::fmt::Debug for SefinSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SefinSandbox").field("pedidos", &self.lock().registros.len()).finish_non_exhaustive()
    }
}

impl Default for SefinSandbox {
    fn default() -> Self {
        Self::new()
    }
}

fn json_resposta(status: u16, corpo: &Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
        body: corpo.to_string().into_bytes(),
    }
}

fn recusa(status: u16, codigo: &str, complemento: Option<String>) -> HttpResponse {
    let descricao = regra(codigo).map_or("Recusada", |regra| regra.descricao);
    let mut erro = json!({ "Codigo": codigo, "Descricao": format!("{PREFIXO}{descricao}") });
    if let Some(complemento) = complemento {
        erro["Complemento"] = Value::String(complemento);
    }
    json_resposta(status, &json!({ "tipoAmbiente": 2, "versaoAplicativo": VER_APLIC, "erros": [erro] }))
}

fn codigos(resposta: &HttpResponse) -> Vec<String> {
    let Ok(corpo) = serde_json::from_slice::<Value>(&resposta.body) else { return Vec::new() };
    ["erros", "erro"]
        .iter()
        .filter_map(|nome| corpo.get(*nome))
        .flat_map(|item| match item {
            Value::Array(lista) => lista.clone(),
            outro => vec![outro.clone()],
        })
        .filter_map(|erro| {
            erro.get("Codigo").or_else(|| erro.get("codigo")).and_then(Value::as_str).map(str::to_owned)
        })
        .collect()
}

impl SefinSandbox {
    /// A sandbox that checks and answers as the Sefin, with SHA-1 and SHA-256 accepted.
    #[must_use]
    pub fn new() -> Self {
        Self::com_handlers(Vec::new())
    }

    /// A sandbox with handlers that [`Self::reset_handlers`] brings back.
    #[must_use]
    pub fn com_handlers(handlers: Vec<Handler>) -> Self {
        Self {
            estado: Arc::new(Mutex::new(Estado {
                handlers: Cadeia::com_iniciais(handlers),
                on_unhandled: OnUnhandled::Error,
                registros: Vec::new(),
                avisos: Vec::new(),
                por_dps: HashMap::new(),
                dps_da_chave: HashMap::new(),
                proximo_numero: 1,
                sem_convenio: Vec::new(),
                aceitos: vec![HashAssinatura::Sha1, HashAssinatura::Sha256],
                agora: "2026-11-06T10:00:00-03:00".to_owned(),
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds a handler that stays until [`Self::reset_handlers`].
    pub fn use_handler(&self, handler: Handler) {
        self.lock().handlers.adicionar(handler, false);
    }

    /// Adds a handler that answers once and goes away.
    pub fn use_once(&self, handler: Handler) {
        self.lock().handlers.adicionar(handler, true);
    }

    /// Goes back to the initial handlers. The NFS-e already issued stay.
    pub fn reset_handlers(&self) {
        self.lock().handlers.restaurar();
    }

    /// What to do with a request no route takes.
    pub fn on_unhandled(&self, regra: OnUnhandled) {
        self.lock().on_unhandled = regra;
    }

    /// The warnings of `OnUnhandled::Warn`.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.lock().avisos.clone()
    }

    /// Everything received, in order, with what was answered.
    #[must_use]
    pub fn requests(&self) -> Vec<Registro> {
        self.lock().registros.clone()
    }

    /// Which hashes the signature may use. Both by default, while the vigent one is not confirmed.
    pub fn aceitar_hashes(&self, hashes: &[HashAssinatura]) {
        self.lock().aceitos = hashes.to_vec();
    }

    /// The moment of processing, in `AAAA-MM-DDThh:mm:ss-03:00`.
    pub fn agora(&self, quando: &str) {
        quando.clone_into(&mut self.lock().agora);
    }

    /// A city that has not joined the national standard: its DPS gets E0037.
    pub fn sem_convenio(&self, municipio: &str) {
        self.lock().sem_convenio.push(municipio.to_owned());
    }

    /// How many NFS-e it issued.
    #[must_use]
    pub fn emitidas(&self) -> usize {
        self.lock().por_dps.len()
    }

    fn rodar_handlers(&self, http: &HttpRequest) -> Option<Resposta> {
        let candidatos = self.lock().handlers.candidatos(|handler| handler.rota.casar(http).is_some());
        for (id, once, handler) in candidatos {
            if once && !self.lock().handlers.consumir(id) {
                continue;
            }
            let pedido =
                PedidoSefin { http: http.clone(), parametros: handler.rota.casar(http).unwrap_or_default() };
            let resposta = (handler.responder)(&pedido);
            if !matches!(resposta, Resposta::Proximo) {
                return Some(resposta);
            }
        }
        None
    }

    fn padrao(&self, http: &HttpRequest) -> Option<HttpResponse> {
        let rotas: [(Metodo, &str); 6] = [
            (Metodo::Post, "/nfse"),
            (Metodo::Head, "/dps/{id}"),
            (Metodo::Get, "/dps/{id}"),
            (Metodo::Get, "/nfse/{chave}"),
            (Metodo::Post, "/nfse/{chave}/eventos"),
            (Metodo::Get, "/parametros_municipais/{municipio}/convenio"),
        ];
        let (indice, parametros) = rotas.iter().enumerate().find_map(|(indice, (metodo, caminho))| {
            Rota::new(*metodo, caminho).casar(http).map(|parametros| (indice, parametros))
        })?;
        let parametro = |nome: &str| {
            parametros
                .iter()
                .find(|(chave, _)| chave == nome)
                .map(|(_, valor)| valor.clone())
                .unwrap_or_default()
        };
        Some(match indice {
            0 => self.receber_dps(http),
            1 => {
                let existe = self.lock().por_dps.contains_key(&parametro("id"));
                HttpResponse { status: if existe { 200 } else { 404 }, headers: Vec::new(), body: Vec::new() }
            }
            2 => match self.lock().por_dps.get(&parametro("id")) {
                Some(nota) => {
                    json_resposta(200, &json!({ "idDps": parametro("id"), "chaveAcesso": nota.chave }))
                }
                None => recusa(404, "SANDBOX-NAO-ENCONTRADA", None),
            },
            3 => self.devolver_nfse(&parametro("chave")),
            4 => self.receber_evento(http, &parametro("chave")),
            _ => {
                let municipio = parametro("municipio");
                let aderente = !self.lock().sem_convenio.contains(&municipio);
                json_resposta(
                    200,
                    &json!({ "codigoMunicipio": municipio, "aderente": aderente, "origem": VER_APLIC }),
                )
            }
        })
    }

    fn devolver_nfse(&self, chave: &str) -> HttpResponse {
        let estado = self.lock();
        let nota = estado.dps_da_chave.get(chave).and_then(|id| estado.por_dps.get(id));
        match nota.and_then(|nota| gzip_base64(&nota.xml).ok().map(|zip| (nota, zip))) {
            Some((nota, zip)) => {
                json_resposta(200, &json!({ "chaveAcesso": nota.chave, "nfseXmlGZipB64": zip }))
            }
            None => recusa(404, "SANDBOX-NAO-ENCONTRADA", None),
        }
    }

    fn documento(http: &HttpRequest, campo: &str) -> Result<(Element, String), HttpResponse> {
        let corpo: Value = serde_json::from_slice(&http.body)
            .map_err(|_| recusa(400, "SANDBOX-CONTEUDO", Some("o corpo não é JSON".to_owned())))?;
        let zip = corpo
            .get(campo)
            .and_then(Value::as_str)
            .ok_or_else(|| recusa(400, "SANDBOX-CONTEUDO", Some(format!("{campo} ausente"))))?;
        let texto =
            de_gzip_base64(zip).map_err(|erro| recusa(400, "SANDBOX-CONTEUDO", Some(erro.to_string())))?;
        let raiz =
            xml::parse(&texto).map_err(|erro| recusa(400, "SANDBOX-LEIAUTE", Some(erro.to_string())))?;
        Ok((raiz, texto))
    }

    fn conferir_assinatura(&self, raiz: &Element, id: &str) -> Result<(), HttpResponse> {
        let aceitos = self.lock().aceitos.clone();
        verificar_envelopado(raiz, id, &aceitos).map(|_| ()).map_err(|falha| {
            let motivo = match falha {
                FalhaAssinatura::ForaDoPadrao(motivo) => format!("fora do padrão: {motivo}"),
                FalhaAssinatura::Difere(motivo) => format!("não confere: {motivo}"),
            };
            recusa(400, "E0714", Some(motivo))
        })
    }

    fn receber_dps(&self, http: &HttpRequest) -> HttpResponse {
        match self.conferir_dps(http) {
            Ok(resposta) | Err(resposta) => resposta,
        }
    }

    fn conferir_dps(&self, http: &HttpRequest) -> Result<HttpResponse, HttpResponse> {
        let (raiz, dps_recebida) = Self::documento(http, "dpsXmlGZipB64")?;
        let leiaute = |motivo: &str| recusa(400, "SANDBOX-LEIAUTE", Some(motivo.to_owned()));
        if raiz.local_name() != "DPS" || raiz.attr("xmlns") != Some(NS_NFSE) {
            return Err(leiaute("raiz DPS no namespace do padrão nacional"));
        }
        if !matches!(raiz.attr("versao"), Some("1.00" | "1.01")) {
            return Err(leiaute("versao 1.00 ou 1.01"));
        }
        let info = raiz.child("infDPS").ok_or_else(|| leiaute("infDPS ausente"))?;
        let id = info.attr("Id").unwrap_or_default().to_owned();
        let campo = |nome: &str| info.child_text(nome).unwrap_or_default();
        let prest = info.child("prest").ok_or_else(|| leiaute("prest ausente"))?;
        let (tipo, inscricao) = match (prest.child_text("CNPJ"), prest.child_text("CPF")) {
            (Some(cnpj), _) => (2, cnpj),
            (None, Some(cpf)) => (1, cpf),
            _ => return Err(leiaute("prest sem CNPJ nem CPF")),
        };
        let serie: u32 = campo("serie").parse().unwrap_or(0);
        let numero: u64 = campo("nDPS").parse().unwrap_or(0);
        let municipio = campo("cLocEmi");
        let esperado = format!("DPS{municipio}{tipo}{inscricao:0>14}{serie:05}{numero:015}");
        if id.len() != 45 || id != esperado {
            return Err(recusa(400, "E0004", Some(format!("esperado {esperado}"))));
        }
        if campo("tpAmb") != "2" {
            return Err(recusa(400, "E0006", Some(format!("tpAmb={}", campo("tpAmb")))));
        }
        let agora = self.lock().agora.clone();
        let (emissao, processamento) =
            (ravenrs_dfe::Instante::de_xml(&campo("dhEmi")), ravenrs_dfe::Instante::de_xml(&agora));
        match (emissao, processamento) {
            (Some(emissao), Some(processamento)) if emissao > processamento => {
                return Err(recusa(
                    400,
                    "E0008",
                    Some(format!("dhEmi {} depois de {agora}", campo("dhEmi"))),
                ));
            }
            (None, _) => return Err(leiaute("dhEmi fora do formato")),
            _ => {}
        }
        self.conferir_assinatura(&raiz, &id)?;
        if self.lock().sem_convenio.contains(&municipio) {
            return Err(recusa(400, "E0037", Some(municipio)));
        }
        if self.lock().por_dps.contains_key(&id) {
            return Err(recusa(400, "E0014", Some(id)));
        }
        Ok(self.gerar_nfse(&id, &municipio, tipo, &inscricao, &dps_recebida, &agora))
    }

    fn gerar_nfse(
        &self,
        id_dps: &str,
        municipio: &str,
        tipo: u8,
        inscricao: &str,
        dps: &str,
        agora: &str,
    ) -> HttpResponse {
        let mut estado = self.lock();
        let numero = estado.proximo_numero;
        estado.proximo_numero += 1;
        let ano_mes = format!("{}{}", agora.get(2..4).unwrap_or("26"), agora.get(5..7).unwrap_or("01"));
        // Generating environment 2: the Sefin Nacional. Numeric code from the number, for repeatable tests.
        let sem_dv = format!(
            "{municipio}2{tipo}{inscricao:0>14}{numero:013}{ano_mes}{:09}",
            numero * 7919 % 1_000_000_000
        );
        let chave = format!("{sem_dv}{}", dv_modulo_11(&sem_dv));
        // The DPS goes inside as received, without its XML declaration if it had one.
        let dps = dps.trim_start_matches(|c| c != '<').to_owned();
        let xml = format!(
            "<NFSe xmlns=\"{NS_NFSE}\" versao=\"{VERSAO}\"><infNFSe Id=\"NFS{chave}\"><xLocEmi>{}</xLocEmi><ambGer>2</ambGer><nNFSe>{numero}</nNFSe><dhProc>{agora}</dhProc><cStat>100</cStat>{dps}</infNFSe></NFSe>",
            escape_text("SANDBOX - SEM VALIDADE JURÍDICA"),
        );
        let resposta = match gzip_base64(&xml) {
            Ok(zip) => json_resposta(
                201,
                &json!({
                    "tipoAmbiente": 2,
                    "versaoAplicativo": VER_APLIC,
                    "dataHoraProcessamento": agora,
                    "idDps": id_dps,
                    "chaveAcesso": chave,
                    "nfseXmlGZipB64": zip,
                    "alertas": [{ "Codigo": "SANDBOX", "Descricao": format!("{PREFIXO}NFS-e sem validade jurídica") }],
                }),
            ),
            Err(erro) => recusa(500, "SANDBOX-CONTEUDO", Some(erro.to_string())),
        };
        estado.dps_da_chave.insert(chave.clone(), id_dps.to_owned());
        estado.por_dps.insert(id_dps.to_owned(), Nota { chave, xml, cancelada: false });
        resposta
    }

    fn receber_evento(&self, http: &HttpRequest, chave: &str) -> HttpResponse {
        let conferido = (|| -> Result<HttpResponse, HttpResponse> {
            let (raiz, _) = Self::documento(http, "pedidoRegistroEventoXmlGZipB64")?;
            let leiaute = |motivo: &str| recusa(400, "SANDBOX-LEIAUTE", Some(motivo.to_owned()));
            if raiz.local_name() != "pedRegEvento" || raiz.attr("xmlns") != Some(NS_NFSE) {
                return Err(leiaute("raiz pedRegEvento no namespace do padrão nacional"));
            }
            let info = raiz.child("infPedReg").ok_or_else(|| leiaute("infPedReg ausente"))?;
            let id = info.attr("Id").unwrap_or_default().to_owned();
            if info.child_text("chNFSe").as_deref() != Some(chave) {
                return Err(leiaute("chNFSe difere da chave do caminho"));
            }
            if id != format!("PRE{chave}{CANCELAMENTO}") || info.child(&format!("e{CANCELAMENTO}")).is_none()
            {
                return Err(leiaute("só o cancelamento e101101, com Id PRE + chave + 101101"));
            }
            self.conferir_assinatura(&raiz, &id)?;
            let mut estado = self.lock();
            let Some(id_dps) = estado.dps_da_chave.get(chave).cloned() else {
                return Err(recusa(404, "SANDBOX-NAO-ENCONTRADA", None));
            };
            let agora = estado.agora.clone();
            let Some(nota) = estado.por_dps.get_mut(&id_dps) else {
                return Err(recusa(404, "SANDBOX-NAO-ENCONTRADA", None));
            };
            if nota.cancelada {
                return Err(recusa(400, "E0840", None));
            }
            nota.cancelada = true;
            let evento = format!(
                "<evento xmlns=\"{NS_NFSE}\" versao=\"{VERSAO}\"><infEvento Id=\"EVT{chave}{CANCELAMENTO}001\"><verAplic>{VER_APLIC}</verAplic><ambGer>2</ambGer><nSeqEvento>1</nSeqEvento><dhProc>{agora}</dhProc><chNFSe>{chave}</chNFSe><tpEvento>{CANCELAMENTO}</tpEvento></infEvento></evento>"
            );
            let zip = gzip_base64(&evento)
                .map_err(|erro| recusa(500, "SANDBOX-CONTEUDO", Some(erro.to_string())))?;
            Ok(json_resposta(
                201,
                &json!({ "tipoAmbiente": 2, "versaoAplicativo": VER_APLIC, "eventoXmlGZipB64": zip }),
            ))
        })();
        match conferido {
            Ok(resposta) | Err(resposta) => resposta,
        }
    }

    fn nao_tratado(&self, http: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let regra = self.lock().on_unhandled.clone();
        let acao = format!("{} {}", http.metodo.nome(), http.caminho());
        match regra {
            OnUnhandled::Error => Err(TransportError::NaoTratado { url: http.url.clone(), acao: Some(acao) }),
            OnUnhandled::Warn => {
                self.lock().avisos.push(format!("pedido não tratado: {acao}"));
                Ok(recusa(404, "SANDBOX-NAO-ENCONTRADA", Some(acao)))
            }
            OnUnhandled::Passthrough(transport) => transport.send(http),
        }
    }

    fn responder(&self, http: &HttpRequest) -> Result<HttpResponse, TransportError> {
        match self.rodar_handlers(http) {
            Some(Resposta::Http(resposta)) => Ok(resposta),
            Some(Resposta::Timeout) => Err(TransportError::Timeout),
            Some(Resposta::Erro(erro)) => Err(erro),
            Some(Resposta::ProcessarEPerder) => {
                drop(self.padrao(http));
                Err(TransportError::Timeout)
            }
            Some(Resposta::Proximo) | None => match self.padrao(http) {
                Some(resposta) => Ok(resposta),
                None => self.nao_tratado(http),
            },
        }
    }
}

impl Transport for SefinSandbox {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let resultado = self.responder(request);
        let registro = Registro {
            http: request.clone(),
            status: resultado.as_ref().ok().map(|resposta| resposta.status),
            codigos: resultado.as_ref().map(codigos).unwrap_or_default(),
        };
        self.lock().registros.push(registro);
        resultado
    }

    fn intercepta(&self) -> bool {
        true
    }
}
