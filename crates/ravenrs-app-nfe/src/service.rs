//! The sending: everything fiscal runs off the thread of the screens, and what came back is kept.

use std::rc::Rc;
use std::sync::Arc;

use ravenrs_app::{Object, ObjectSpace, Query, SpaceError, Value, next_number};
use ravenrs_app_ui::Session;
use ravenrs_nfe::sandbox::{SandboxSigner, SefazSandbox};
use ravenrs_nfe::{
    Ambiente, BetaAck, Emissao, Emissor, Emitente, Endereco, HttpRequest, HttpResponse, NfeError, Rascunho,
    Signer, Situacao, Transport, TransportError, Uf,
};
use rust_decimal::Decimal;

use crate::cupom::CUPOM_LINHA;
use crate::module::{NFE_DOCUMENTO, NFE_EMITENTE};

/// What a sale says about itself on the receipt, beyond the document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextoVenda {
    /// How it was paid, as people call it.
    pub pagamento: String,
    /// How much was handed over.
    pub recebido: Decimal,
    /// Who bought it, as the receipt says it.
    pub consumidor: String,
}

/// How a document ended.
#[derive(Clone, Debug, PartialEq)]
pub struct EmissaoFinal {
    /// The use was authorized, or simulated as such by the sandbox.
    pub autorizada: bool,
    /// The service is down or did not answer, so it is worth trying again.
    pub tentar_de_novo: bool,
    /// The code of the answer; 0 when there was none.
    pub c_stat: u32,
    /// What the service said, or what went wrong.
    pub motivo: String,
    /// The access key, when the document got that far.
    pub chave: String,
    /// The protocol, when it was authorized.
    pub protocolo: String,
    /// The number of the document.
    pub numero: u32,
    /// The total.
    pub total: Decimal,
    /// The change.
    pub troco: Decimal,
}

impl EmissaoFinal {
    fn falhou(motivo: String, numero: u32) -> Self {
        Self {
            autorizada: false,
            tentar_de_novo: false,
            c_stat: 0,
            motivo,
            chave: String::new(),
            protocolo: String::new(),
            numero,
            total: Decimal::ZERO,
            troco: Decimal::ZERO,
        }
    }
}

struct SharedSigner(Arc<dyn Signer>);

impl Signer for SharedSigner {
    fn certificate_der(&self) -> &[u8] {
        self.0.certificate_der()
    }
    fn sign_rsa_sha1(&self, data: &[u8]) -> Result<Vec<u8>, NfeError> {
        self.0.sign_rsa_sha1(data)
    }
}

struct SharedTransport(Arc<dyn Transport>);

impl Transport for SharedTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.send(request)
    }
    fn intercepta(&self) -> bool {
        self.0.intercepta()
    }
}

struct Inner {
    ambiente: Ambiente,
    signer: Arc<dyn Signer>,
    transport: Arc<dyn Transport>,
    sandbox: Option<SefazSandbox>,
}

/// How documents go out. A clone is the same service.
#[derive(Clone)]
pub struct NfeService(Rc<Inner>);

impl std::fmt::Debug for NfeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NfeService").field("ambiente", &self.0.ambiente).finish_non_exhaustive()
    }
}

impl NfeService {
    /// Everything stays on this computer: nothing reaches the SEFAZ.
    pub fn sandbox(_ack: BetaAck) -> Result<Self, NfeError> {
        let sandbox = SefazSandbox::new();
        Ok(Self(Rc::new(Inner {
            ambiente: Ambiente::Sandbox,
            signer: Arc::new(SandboxSigner::new()?),
            transport: Arc::new(sandbox.clone()),
            sandbox: Some(sandbox),
        })))
    }

    /// The real homologation servers, with this signer and way out. They have no fiscal value.
    pub fn homologacao(_ack: BetaAck, signer: impl Signer + 'static, transport: impl Transport + 'static) -> Self {
        Self(Rc::new(Inner {
            ambiente: Ambiente::Homologacao,
            signer: Arc::new(signer),
            transport: Arc::new(transport),
            sandbox: None,
        }))
    }

    /// Which environment it talks to.
    pub fn ambiente(&self) -> Ambiente {
        self.0.ambiente
    }

    /// The sandbox, to make it answer as asked.
    pub fn sandbox_ativo(&self) -> Option<&SefazSandbox> {
        self.0.sandbox.as_ref()
    }

    /// What the receipt says about the environment.
    pub fn aviso(&self) -> &'static str {
        match self.0.ambiente {
            Ambiente::Sandbox => "SANDBOX, SEM VALOR FISCAL",
            Ambiente::Homologacao => "EMITIDA EM HOMOLOGACAO, SEM VALOR FISCAL",
        }
    }

    pub(crate) fn emissor(&self, emitente: Emitente) -> Result<Emissor, NfeError> {
        Emissor::builder(BetaAck::i_understand_this_is_experimental())
            .ambiente(self.0.ambiente)
            .emitente(emitente)
            .signer(SharedSigner(self.0.signer.clone()))
            .transport(SharedTransport(self.0.transport.clone()))
            .build()
    }

    /// Issues a document out of the way of the screens, keeps what came back and then says how it
    /// ended. `monta` makes the draft, given the number and the series of the issuer.
    pub fn emitir(
        &self,
        session: &Session,
        monta: impl FnOnce(u32, u16) -> Rascunho,
        venda: TextoVenda,
        done: impl FnOnce(&Session, EmissaoFinal) + 'static,
    ) {
        let space = session.frame().space();
        let record = emitente_salvo(&*space.borrow());
        let Some(record) = record else {
            done(session, EmissaoFinal::falhou("Cadastre o emitente antes de emitir.".to_owned(), 0));
            return;
        };
        let (emitente, serie) = match emitente_do_registro(&record) {
            Ok(found) => found,
            Err(why) => {
                done(session, EmissaoFinal::falhou(why, 0));
                return;
            }
        };
        // Only a document that came back for real takes a number for good.
        let mut rascunho = monta(0, serie);
        let sequencia = format!("nfe-{}-{serie}", rascunho.modelo.codigo());
        let numero = {
            let mut space = space.borrow_mut();
            let next = next_number(&mut *space, &sequencia).unwrap_or(1);
            space.rollback();
            next.max(1) as u32
        };
        rascunho.numero = numero;
        rascunho.emitido_em = agora_com_fuso();
        let emissor = match self.emissor(emitente.clone()) {
            Ok(emissor) => emissor,
            Err(error) => {
                done(session, EmissaoFinal::falhou(error.to_string(), numero));
                return;
            }
        };
        let service = self.clone();
        let kept = rascunho.clone();
        session.run_background(
            "Enviando a nota",
            move |progress| {
                progress.report(0.2, "Assinando e enviando");
                emissor.emitir(&rascunho).map_err(|error| error.to_string())
            },
            move |session, sent| {
                let ended = guardar(session, &service, &emitente, &kept, &venda, &sequencia, sent);
                session.records_changed();
                done(session, ended);
            },
        );
    }
}

/// The issuer kept, if there is one.
pub(crate) fn emitente_salvo(space: &dyn ObjectSpace) -> Option<Object> {
    space.query(NFE_EMITENTE, &Query::all()).ok()?.into_iter().next()
}

/// The moment of issue as the layout writes it, in the hour of Brasília.
pub(crate) fn agora_com_fuso() -> String {
    let now = ravenrs_app::local_now();
    let (year, month, day) = now.ymd();
    let (hour, minute, second) = (now.seconds / 3600, now.seconds / 60 % 60, now.seconds % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}-03:00")
}

fn texto(record: &Object, field: &str) -> String {
    match record.get(field) {
        Value::Text(text) => text.trim().to_owned(),
        _ => String::new(),
    }
}

/// The issuer and its series from what people typed, or what is missing.
pub fn emitente_do_registro(record: &Object) -> Result<(Emitente, u16), String> {
    let uf = match texto(record, "uf").to_uppercase().as_str() {
        "SP" => Uf::SP,
        "RS" => Uf::RS,
        "PR" => Uf::PR,
        "MG" => Uf::MG,
        "SC" => Uf::SC,
        "RJ" => Uf::RJ,
        "ES" => Uf::ES,
        "DF" => Uf::DF,
        other => return Err(format!("UF \u{201c}{other}\u{201d} ainda não está nesta versão beta.")),
    };
    let serie = match record.get("serie") {
        Value::Number(serie) => serie as u16,
        _ => 1,
    };
    let fantasia = texto(record, "nome_fantasia");
    Ok((
        Emitente {
            cnpj: texto(record, "cnpj"),
            razao_social: texto(record, "razao_social"),
            nome_fantasia: (!fantasia.is_empty()).then_some(fantasia),
            inscricao_estadual: texto(record, "inscricao_estadual"),
            endereco: Endereco {
                logradouro: texto(record, "logradouro"),
                numero: texto(record, "numero"),
                bairro: texto(record, "bairro"),
                codigo_municipio: texto(record, "codigo_municipio"),
                municipio: texto(record, "municipio"),
                uf,
                cep: texto(record, "cep"),
            },
        },
        serie,
    ))
}

fn money(value: Decimal) -> String {
    format!("{:.2}", value.round_dp(2)).replace('.', ",")
}

fn grupos_da_chave(chave: &str) -> String {
    chave.as_bytes().chunks(4).map(String::from_utf8_lossy).collect::<Vec<_>>().join(" ")
}

/// Keeps what came back, and says how it ended.
fn guardar(
    session: &Session,
    service: &NfeService,
    emitente: &Emitente,
    rascunho: &Rascunho,
    venda: &TextoVenda,
    sequencia: &str,
    sent: Result<Emissao, String>,
) -> EmissaoFinal {
    let space = session.frame().space();
    let mut space = space.borrow_mut();
    let result = (|| -> Result<EmissaoFinal, SpaceError> {
        let mut record = space.new_object(NFE_DOCUMENTO)?;
        record.set("modelo", rascunho.modelo.codigo().to_owned());
        record.set("serie", f64::from(rascunho.serie));
        record.set("numero", f64::from(rascunho.numero));
        record.set("emitido_em", rascunho.emitido_em.clone());
        record.set("ambiente", service.aviso().to_owned());
        let ended = match &sent {
            Ok(emissao) => {
                let documento = &emissao.documento;
                let autorizacao = &emissao.autorizacao;
                let situacao = autorizacao.situacao();
                let autorizada = situacao == Situacao::Autorizada;
                let protocolo = autorizacao
                    .protocolo
                    .as_ref()
                    .and_then(|protocolo| protocolo.n_prot.clone())
                    .unwrap_or_default();
                record.set("situacao", format!("{situacao:?}"));
                record.set("chave", documento.chave.as_str().to_owned());
                record.set("protocolo", protocolo.clone());
                record.set("c_stat", f64::from(autorizacao.c_stat()));
                record.set("motivo", autorizacao.x_motivo().to_owned());
                record.set("total", money(documento.valor_total()));
                record.set("xml", autorizacao.nfe_proc.clone().unwrap_or_else(|| documento.xml.clone()));
                if autorizada {
                    next_number(&mut *space, sequencia)?;
                    guardar_cupom(&mut *space, service, emitente, rascunho, venda, emissao, &protocolo)?;
                }
                EmissaoFinal {
                    autorizada,
                    tentar_de_novo: situacao == Situacao::Paralisado,
                    c_stat: autorizacao.c_stat(),
                    motivo: autorizacao.x_motivo().to_owned(),
                    chave: documento.chave.as_str().to_owned(),
                    protocolo,
                    numero: rascunho.numero,
                    total: documento.valor_total(),
                    troco: documento.totais.v_troco,
                }
            }
            Err(why) => {
                // Nobody knows whether it got there: the number stays used until someone looks.
                next_number(&mut *space, sequencia)?;
                record.set("situacao", "Pendente".to_owned());
                record.set("motivo", why.clone());
                EmissaoFinal { tentar_de_novo: true, ..EmissaoFinal::falhou(why.clone(), rascunho.numero) }
            }
        };
        space.save(record)?;
        space.commit()?;
        Ok(ended)
    })();
    result.unwrap_or_else(|error| {
        space.rollback();
        EmissaoFinal::falhou(format!("A nota voltou, mas não foi guardada: {error}"), rascunho.numero)
    })
}

/// The lines of the receipt of an authorized document.
fn guardar_cupom(
    space: &mut dyn ObjectSpace,
    service: &NfeService,
    emitente: &Emitente,
    rascunho: &Rascunho,
    venda: &TextoVenda,
    emissao: &Emissao,
    protocolo: &str,
) -> Result<(), SpaceError> {
    let documento = &emissao.documento;
    let endereco = &emitente.endereco;
    let lugar = format!(
        "{}, {}, {}, {}/{}",
        endereco.logradouro,
        endereco.numero,
        endereco.bairro,
        endereco.municipio,
        endereco.uf.sigla()
    );
    for (index, item) in rascunho.itens.iter().enumerate() {
        let valor = documento.totais.itens.get(index).map_or(Decimal::ZERO, |line| line.v_prod);
        let mut line = space.new_object(CUPOM_LINHA)?;
        for (field, value) in [
            ("documento", documento.chave.as_str().to_owned()),
            ("ordem", (index + 1).to_string()),
            ("emitente", emitente.nome_fantasia.clone().unwrap_or_else(|| emitente.razao_social.clone())),
            ("cnpj", emitente.cnpj.clone()),
            ("ie", emitente.inscricao_estadual.clone()),
            ("endereco", lugar.clone()),
            ("titulo", "Documento Auxiliar da Nota Fiscal de Consumidor Eletrônica".to_owned()),
            ("aviso", service.aviso().to_owned()),
            ("codigo", item.codigo.clone()),
            ("descricao", item.descricao.clone()),
            ("quantidade", item.quantidade.normalize().to_string().replace('.', ",")),
            ("unidade", item.unidade.clone()),
            ("unitario", money(item.valor_unitario)),
            ("valor", money(valor)),
            ("itens", rascunho.itens.len().to_string()),
            ("total", money(documento.valor_total())),
            ("pagamento", venda.pagamento.clone()),
            ("recebido", money(venda.recebido)),
            ("troco", money(documento.totais.v_troco)),
            ("chave", grupos_da_chave(documento.chave.as_str())),
            ("numero", format!("NFC-e {:09}, série {}", rascunho.numero, rascunho.serie)),
            ("emitido", rascunho.emitido_em.replace('T', " ")),
            ("protocolo", protocolo.to_owned()),
            ("consumidor", venda.consumidor.clone()),
            ("qr_code", documento.qr_code.clone().unwrap_or_default()),
            ("observacao", rascunho.observacao.clone().unwrap_or_default()),
        ] {
            line.set(field, value);
        }
        space.save(line)?;
    }
    Ok(())
}
