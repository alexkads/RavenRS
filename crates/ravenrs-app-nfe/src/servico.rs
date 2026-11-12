//! The NFS-e issued from an application: the DPS is kept before it goes, so that an answer lost on
//! the way, or the program closed in the middle, never turns into a second number for the same
//! service. What is pending goes again as the same DPS, and the Sefin either finds the NFS-e it
//! already made or makes it now.

use std::rc::Rc;
use std::sync::Arc;

use ravenrs_app::{
    Action, ActionCategory, ActionInput, CriteriaMode, EntityInfo, FieldInfo, FieldKind, Message,
    MessageKind, Object, ObjectSpace, Query, SpaceError, TargetScope, Value, ViewController, next_number,
};
use ravenrs_app_report::{ReportWords, stored_reports_for};
use ravenrs_app_ui::Session;
use ravenrs_nfse::{
    ChaveNfse, Dps, Emissor, Emitida, Inscricao, MotivoCancelamento, NfseError, Prestador, Servico, Tomador,
};
use ravenrs_reactive::Binding;
use ravenrs_report::Document;
use ravenrs_report_viewer::{DocumentViewer, ViewerState};
use ravenrs_widgets::{AnyView, TextArea};
use rust_decimal::Decimal;

use crate::danfse::{MODELO_DANFSE, NFSE_LINHA, linha_da_nfse, renderizar_danfse};
use crate::service::{NfeService, agora_com_fuso, emitente_salvo, texto};

/// The entity of the NFS-e issued, which storage has to know.
pub const NFSE_DOCUMENTO: &str = "NfseDocumento";

/// Kept before the DPS goes: if the program closes now, it is still pending when it opens again.
pub const ENVIANDO: &str = "Enviando";
/// The Sefin made the NFS-e.
pub const AUTORIZADA: &str = "Autorizada";
/// Nobody knows whether it got there: the same DPS has to go again.
pub const PENDENTE: &str = "Pendente";
/// The Sefin refused it.
pub const REJEITADA: &str = "Rejeitada";
/// Canceled by an event.
pub const CANCELADA: &str = "Cancelada";

/// The NFS-e issued, with everything that makes its DPS again.
pub fn nfse_entity() -> EntityInfo {
    let shown =
        |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption).read_only();
    let hidden =
        |name: &str| FieldInfo::new(name, FieldKind::Text).read_only().in_list(false).in_detail(false);
    EntityInfo::new(NFSE_DOCUMENTO)
        .caption("NFS-e emitidas")
        .navigation("Fiscal")
        .icon("file-text")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(
            FieldInfo::new("numero_dps", FieldKind::Integer)
                .caption("Número da DPS")
                .read_only()
                .default_property(),
        )
        .field(FieldInfo::new("serie", FieldKind::Integer).caption("Série da DPS").read_only())
        .field(shown("situacao", "Situação"))
        .field(shown("numero_nfse", "Número da NFS-e"))
        .field(shown("valor", "Valor"))
        .field(shown("tomador", "Tomador"))
        .field(shown("tomador_documento", "CPF ou CNPJ do tomador"))
        .field(shown("servico", "Serviço"))
        .field(shown("codigo_servico", "Código de tributação nacional"))
        .field(shown("emitida_em", "Emissão da DPS"))
        .field(shown("ambiente", "Ambiente"))
        .field(shown("chave", "Chave de acesso"))
        .field(shown("id_dps", "Id da DPS"))
        .field(shown("motivo", "Motivo"))
        .field(hidden("competencia"))
        .field(hidden("municipio_prestacao"))
        .field(hidden("prestador_documento"))
        .field(hidden("prestador_im"))
        .field(hidden("prestador_nome"))
        .field(hidden("prestador_municipio"))
        .field(FieldInfo::new("xml", FieldKind::LongText).in_list(false).in_detail(false))
}

/// A service sold, as the counter knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServicoVendido {
    /// `cTribNac`, six digits.
    pub codigo_tributacao_nacional: String,
    /// What was done.
    pub descricao: String,
    /// The value, in reais.
    pub valor: Decimal,
    /// Who took it, if identified.
    pub tomador: Option<Tomador>,
}

/// How an NFS-e ended.
#[derive(Clone, Debug, PartialEq)]
pub struct NfseFinal {
    /// The Sefin made it, or the sandbox simulated it.
    pub autorizada: bool,
    /// Nobody knows whether it got there: send the same DPS again.
    pub pendente: bool,
    /// The record kept, to send again or print.
    pub registro: Value,
    /// The number of the DPS; 0 when nothing was kept.
    pub numero_dps: u64,
    /// The number of the NFS-e, when there is one.
    pub numero_nfse: String,
    /// The access key, when there is one.
    pub chave: String,
    /// It came by the Id of the DPS after a lost answer.
    pub recuperada: bool,
    /// What the Sefin said, or what went wrong.
    pub motivo: String,
}

impl NfseFinal {
    fn falhou(motivo: impl Into<String>) -> Self {
        Self {
            autorizada: false,
            pendente: false,
            registro: Value::Null,
            numero_dps: 0,
            numero_nfse: String::new(),
            chave: String::new(),
            recuperada: false,
            motivo: motivo.into(),
        }
    }
}

fn inscricao(documento: &str) -> Result<Inscricao, String> {
    let digitos: String = documento.chars().filter(char::is_ascii_digit).collect();
    match digitos.len() {
        11 => Ok(Inscricao::Cpf(digitos)),
        14 => Ok(Inscricao::Cnpj(digitos)),
        _ => Err(format!("\u{201c}{documento}\u{201d} não é CPF nem CNPJ.")),
    }
}

/// A CPF or CNPJ typed in, or that it is neither.
pub fn inscricao_de(documento: &str) -> Result<Inscricao, String> {
    inscricao(documento)
}

/// The provider and the series of the DPS from the issuer kept.
pub fn prestador_do_emitente(record: &Object) -> Result<(Prestador, u32), String> {
    let municipio = texto(record, "codigo_municipio");
    if municipio.len() != 7 {
        return Err("Cadastre o código IBGE do município do emitente.".to_owned());
    }
    let im = texto(record, "inscricao_municipal");
    let serie = match record.get("serie_dps") {
        Value::Number(serie) if serie >= 1.0 => serie as u32,
        _ => 1,
    };
    Ok((
        Prestador {
            inscricao: inscricao(&texto(record, "cnpj"))?,
            inscricao_municipal: (!im.is_empty()).then_some(im),
            razao_social: texto(record, "razao_social"),
            municipio,
        },
        serie,
    ))
}

fn valor_em_texto(valor: Decimal) -> String {
    format!("{:.2}", valor.round_dp(2)).replace('.', ",")
}

fn inteiro(record: &Object, campo: &str) -> u64 {
    match record.get(campo) {
        Value::Number(numero) if numero > 0.0 => numero as u64,
        _ => 0,
    }
}

/// The DPS a record was kept with, exactly as it went the first time.
pub fn dps_do_registro(record: &Object) -> Result<Dps, String> {
    let valor: Decimal =
        texto(record, "valor").replace('.', "").replace(',', ".").parse().map_err(|_| "Valor inválido.")?;
    let im = texto(record, "prestador_im");
    let documento = texto(record, "tomador_documento");
    let tomador = if documento.is_empty() {
        None
    } else {
        Some(Tomador { inscricao: inscricao(&documento)?, nome: texto(record, "tomador"), email: None })
    };
    Ok(Dps {
        serie: inteiro(record, "serie") as u32,
        numero: inteiro(record, "numero_dps"),
        emitida_em: texto(record, "emitida_em"),
        competencia: texto(record, "competencia"),
        prestador: Prestador {
            inscricao: inscricao(&texto(record, "prestador_documento"))?,
            inscricao_municipal: (!im.is_empty()).then_some(im),
            razao_social: texto(record, "prestador_nome"),
            municipio: texto(record, "prestador_municipio"),
        },
        tomador,
        servico: Servico {
            codigo_tributacao_nacional: texto(record, "codigo_servico"),
            descricao: texto(record, "servico"),
            municipio_prestacao: texto(record, "municipio_prestacao"),
            valor,
        },
    })
}

fn escrever_dps(record: &mut Object, dps: &Dps, ambiente: &str) {
    let tomador = dps.tomador.as_ref();
    for (campo, valor) in [
        ("situacao", ENVIANDO.to_owned()),
        ("id_dps", ravenrs_nfse::dps::id_dps(dps)),
        ("valor", valor_em_texto(dps.servico.valor)),
        ("tomador", tomador.map(|t| t.nome.clone()).unwrap_or_default()),
        ("tomador_documento", tomador.map(|t| t.inscricao.numero().to_owned()).unwrap_or_default()),
        ("servico", dps.servico.descricao.clone()),
        ("codigo_servico", dps.servico.codigo_tributacao_nacional.clone()),
        ("municipio_prestacao", dps.servico.municipio_prestacao.clone()),
        ("emitida_em", dps.emitida_em.clone()),
        ("competencia", dps.competencia.clone()),
        ("ambiente", ambiente.to_owned()),
        ("prestador_documento", dps.prestador.inscricao.numero().to_owned()),
        ("prestador_im", dps.prestador.inscricao_municipal.clone().unwrap_or_default()),
        ("prestador_nome", dps.prestador.razao_social.clone()),
        ("prestador_municipio", dps.prestador.municipio.clone()),
    ] {
        record.set(campo, valor);
    }
    record.set("serie", f64::from(dps.serie));
    record.set("numero_dps", dps.numero as f64);
}

/// The NFS-e kept that never got a sure answer, the oldest first: those to send again.
pub fn nfses_pendentes(space: &dyn ObjectSpace) -> Vec<Object> {
    let mut found: Vec<Object> = space
        .query(NFSE_DOCUMENTO, &Query::all())
        .unwrap_or_default()
        .into_iter()
        .filter(|record| matches!(texto(record, "situacao").as_str(), ENVIANDO | PENDENTE))
        .collect();
    found.sort_by_key(|record| inteiro(record, "numero_dps"));
    found
}

/// The row of the DANFSe of an NFS-e kept, or why there is none.
pub fn linha_danfse_do_registro(record: &Object) -> Result<Object, String> {
    if texto(record, "situacao") != AUTORIZADA {
        return Err("Só uma NFS-e autorizada tem DANFSe.".to_owned());
    }
    let dps = dps_do_registro(record)?;
    let emitida = Emitida {
        id_dps: texto(record, "id_dps"),
        chave: ChaveNfse::parse(&texto(record, "chave")).map_err(|error| error.to_string())?,
        xml: texto(record, "xml"),
        alertas: Vec::new(),
        recuperada: false,
    };
    Ok(linha_da_nfse(&emitida, &dps))
}

/// The DANFSe of an NFS-e kept, in the layout of that name or the first one for it.
pub fn documento_danfse(session: &Session, record: &Object, words: &ReportWords) -> Result<Document, String> {
    let linha = linha_danfse_do_registro(record)?;
    let modelos = stored_reports_for(session, NFSE_LINHA);
    let modelo = modelos
        .iter()
        .find(|modelo| texto(modelo, "name") == MODELO_DANFSE)
        .or_else(|| modelos.first())
        .ok_or("Não há modelo de DANFSe.")?;
    renderizar_danfse(session, modelo, &linha, words)
}

/// Shows the DANFSe of an NFS-e kept on the screen, from where it prints and goes out as a PDF.
pub fn mostrar_danfse(session: &Session, record: &Object, words: &ReportWords) -> Result<(), String> {
    let documento = Rc::new(documento_danfse(session, record, words)?);
    let state = ViewerState::new();
    session.present("DANFSe", move || Box::new(DocumentViewer::new(documento.clone(), &state)) as AnyView);
    Ok(())
}

fn competencia(emitida_em: &str) -> String {
    emitida_em.get(..10).unwrap_or(emitida_em).to_owned()
}

impl NfeService {
    /// The sandbox of the Sefin Nacional, to make it answer as asked.
    pub fn sefin_ativo(&self) -> Option<&ravenrs_nfse::sandbox::SefinSandbox> {
        self.0.sefin.as_ref()
    }

    fn ambiente_nfse(&self) -> ravenrs_nfse::Ambiente {
        match self.0.ambiente {
            ravenrs_nfe::Ambiente::Sandbox => ravenrs_nfse::Ambiente::Sandbox,
            ravenrs_nfe::Ambiente::Homologacao => ravenrs_nfse::Ambiente::ProducaoRestrita,
        }
    }

    fn texto_ambiente_nfse(&self) -> &'static str {
        match self.0.ambiente {
            ravenrs_nfe::Ambiente::Sandbox => "SANDBOX, SEM VALIDADE JURÍDICA",
            ravenrs_nfe::Ambiente::Homologacao => "PRODUÇÃO RESTRITA, SEM VALIDADE JURÍDICA",
        }
    }

    fn emissor_nfse(&self) -> Result<Emissor, NfseError> {
        Emissor::builder(ravenrs_nfse::BetaAck::i_understand_this_is_experimental())
            .ambiente(self.ambiente_nfse())
            .signer_compartilhado(Arc::clone(&self.0.signer))
            .transport_compartilhado(Arc::clone(&self.0.nfse_transport))
            .build()
    }

    /// Issues the NFS-e of a service sold, out of the way of the screens. The DPS takes its number
    /// and is kept before it goes; then whatever came back is kept on it, and `done` says how it
    /// ended. A DPS that does not get a sure answer stays pending, for [`Self::reenviar_nfse`].
    pub fn emitir_nfse(
        &self,
        session: &Session,
        servico: ServicoVendido,
        done: impl FnOnce(&Session, NfseFinal) + 'static,
    ) {
        let montado = (|| -> Result<(Emissor, Value, Dps), String> {
            let space = session.frame().space();
            let record = emitente_salvo(&*space.borrow()).ok_or("Cadastre o emitente antes de emitir.")?;
            let (prestador, serie) = prestador_do_emitente(&record)?;
            let emitida_em = agora_com_fuso();
            let mut dps = Dps {
                serie,
                numero: 1,
                competencia: competencia(&emitida_em),
                emitida_em,
                servico: Servico {
                    codigo_tributacao_nacional: servico.codigo_tributacao_nacional,
                    descricao: servico.descricao,
                    municipio_prestacao: prestador.municipio.clone(),
                    valor: servico.valor,
                },
                prestador,
                tomador: servico.tomador,
            };
            // What is wrong in it is said before it takes a number.
            dps.validar().map_err(|error| error.to_string())?;
            let emissor = self.emissor_nfse().map_err(|error| error.to_string())?;
            let mut space = space.borrow_mut();
            let kept = (|| -> Result<Value, SpaceError> {
                dps.numero = next_number(&mut *space, &format!("dps-{serie}"))?.max(1) as u64;
                let mut record = space.new_object(NFSE_DOCUMENTO)?;
                escrever_dps(&mut record, &dps, self.texto_ambiente_nfse());
                let id = record.get("id");
                space.save(record)?;
                space.commit()?;
                Ok(id)
            })();
            let id = kept.map_err(|error| {
                space.rollback();
                format!("A DPS não foi guardada, e nada foi enviado: {error}")
            })?;
            Ok((emissor, id, dps))
        })();
        match montado {
            Ok((emissor, id, dps)) => self.enviar_nfse(session, emissor, id, dps, done),
            Err(why) => done(session, NfseFinal::falhou(why)),
        }
    }

    /// Sends a pending DPS again, the same one, with the same number. If the first one did become an
    /// NFS-e, that is the one that comes back.
    pub fn reenviar_nfse(
        &self,
        session: &Session,
        registro: &Value,
        done: impl FnOnce(&Session, NfseFinal) + 'static,
    ) {
        let montado = (|| -> Result<(Emissor, Dps), String> {
            let space = session.frame().space();
            let record = space
                .borrow()
                .find(NFSE_DOCUMENTO, registro)
                .map_err(|error| error.to_string())?
                .ok_or("Essa NFS-e não existe mais.")?;
            if !matches!(texto(&record, "situacao").as_str(), ENVIANDO | PENDENTE) {
                return Err("Essa NFS-e não está pendente.".to_owned());
            }
            let dps = dps_do_registro(&record)?;
            Ok((self.emissor_nfse().map_err(|error| error.to_string())?, dps))
        })();
        match montado {
            Ok((emissor, dps)) => self.enviar_nfse(session, emissor, registro.clone(), dps, done),
            Err(why) => done(session, NfseFinal::falhou(why)),
        }
    }

    fn enviar_nfse(
        &self,
        session: &Session,
        emissor: Emissor,
        registro: Value,
        dps: Dps,
        done: impl FnOnce(&Session, NfseFinal) + 'static,
    ) {
        if let Some(sefin) = &self.0.sefin {
            // The sandbox has its own clock: it is now, or a DPS of now would come from the future.
            sefin.agora(&agora_com_fuso());
        }
        let numero = dps.numero;
        session.run_background(
            "Enviando a NFS-e",
            move |progress| {
                progress.report(0.2, "Assinando e enviando a DPS");
                emissor.emitir(&dps)
            },
            move |session, sent| {
                let ended = guardar_nfse(session, &registro, numero, sent);
                session.records_changed();
                done(session, ended);
            },
        );
    }

    /// Cancels an authorized NFS-e out of the way of the screens; `Ok` when the event was registered.
    pub fn cancelar_nfse(
        &self,
        session: &Session,
        registro: &Value,
        justificativa: &str,
        done: impl FnOnce(&Session, Result<(), String>) + 'static,
    ) {
        let montado = (|| -> Result<(Emissor, ChaveNfse, Inscricao), String> {
            let space = session.frame().space();
            let record = space
                .borrow()
                .find(NFSE_DOCUMENTO, registro)
                .map_err(|error| error.to_string())?
                .ok_or("Essa NFS-e não existe mais.")?;
            if texto(&record, "situacao") != AUTORIZADA {
                return Err("Só uma NFS-e autorizada pode ser cancelada.".to_owned());
            }
            let chave = ChaveNfse::parse(&texto(&record, "chave")).map_err(|error| error.to_string())?;
            let autor = inscricao(&texto(&record, "prestador_documento"))?;
            Ok((self.emissor_nfse().map_err(|error| error.to_string())?, chave, autor))
        })();
        let (emissor, chave, autor) = match montado {
            Ok(montado) => montado,
            Err(why) => return done(session, Err(why)),
        };
        if let Some(sefin) = &self.0.sefin {
            sefin.agora(&agora_com_fuso());
        }
        let (justificativa, quando, registro) =
            (justificativa.to_owned(), agora_com_fuso(), registro.clone());
        session.run_background(
            "Cancelando a NFS-e",
            move |_| emissor.cancelar(&chave, &autor, MotivoCancelamento::Outros, &justificativa, &quando),
            move |session, answer| {
                let ended = answer.map_err(|error| error.to_string()).and_then(|evento| {
                    atualizar(session, &registro, |record| {
                        record.set("situacao", CANCELADA.to_owned());
                        record.set("motivo", "Cancelada por evento 101101.".to_owned());
                        let xml = texto(record, "xml");
                        record.set("xml", format!("{xml}\n{evento}"));
                    })
                });
                session.records_changed();
                done(session, ended);
            },
        );
    }
}

fn atualizar(session: &Session, registro: &Value, muda: impl FnOnce(&mut Object)) -> Result<(), String> {
    let space = session.frame().space();
    let mut space = space.borrow_mut();
    let result = (|| -> Result<(), SpaceError> {
        if let Some(mut record) = space.find(NFSE_DOCUMENTO, registro)? {
            muda(&mut record);
            space.save(record)?;
        }
        space.commit()
    })();
    result.map_err(|error| {
        space.rollback();
        error.to_string()
    })
}

/// Keeps on the DPS what came back, and says how it ended. Only a refusal by the Sefin ends it as
/// refused; anything else that is not an NFS-e leaves it pending, to go again as it is.
fn guardar_nfse(
    session: &Session,
    registro: &Value,
    numero_dps: u64,
    sent: Result<Emitida, NfseError>,
) -> NfseFinal {
    let mut fim = NfseFinal { registro: registro.clone(), numero_dps, ..NfseFinal::falhou("") };
    match &sent {
        Ok(emitida) => {
            fim.autorizada = true;
            fim.chave = emitida.chave.as_str().to_owned();
            fim.numero_nfse = emitida.chave.numero().to_string();
            fim.recuperada = emitida.recuperada;
            let alertas: Vec<String> =
                emitida.alertas.iter().map(|m| format!("{}: {}", m.codigo, m.descricao)).collect();
            fim.motivo = if emitida.recuperada {
                "A resposta se perdeu, e a NFS-e foi recuperada pelo Id da DPS.".to_owned()
            } else {
                alertas.join("; ")
            };
        }
        Err(NfseError::Rejeitada(mensagens)) => {
            fim.motivo = mensagens
                .iter()
                .map(|m| format!("{}: {}", m.codigo, m.descricao))
                .collect::<Vec<_>>()
                .join("; ");
        }
        Err(error) => {
            fim.pendente = true;
            fim.motivo = error.to_string();
        }
    }
    let kept = atualizar(session, registro, |record| {
        match &sent {
            Ok(emitida) => {
                record.set("situacao", AUTORIZADA.to_owned());
                record.set("chave", fim.chave.clone());
                record.set("numero_nfse", fim.numero_nfse.clone());
                record.set("xml", emitida.xml.clone());
            }
            Err(NfseError::Rejeitada(_)) => record.set("situacao", REJEITADA.to_owned()),
            Err(_) => record.set("situacao", PENDENTE.to_owned()),
        }
        record.set("motivo", fim.motivo.clone());
    });
    if let Err(error) = kept {
        // The Sefin has it, the records do not: say so, the DPS stays pending and goes again.
        fim.motivo = format!("A NFS-e voltou, mas não foi guardada: {error}. Envie a mesma DPS de novo.");
        fim.pendente = true;
        fim.autorizada = false;
    }
    fim
}

const REENVIAR: &str = "Nfse.Reenviar";
const CANCELAR: &str = "Nfse.Cancelar";
const DANFSE: &str = "Nfse.Danfse";
const VER_XML: &str = "Nfse.VerXml";

fn dizer(session: &Session, fim: &NfseFinal) {
    let (texto, tipo) = if fim.autorizada {
        (format!("NFS-e {} autorizada.", fim.numero_nfse), MessageKind::Success)
    } else if fim.pendente {
        (format!("A DPS {} continua pendente: {}", fim.numero_dps, fim.motivo), MessageKind::Warning)
    } else {
        (fim.motivo.clone(), MessageKind::Error)
    };
    session.show_message(Message::new(texto, tipo));
}

/// What can be done with an NFS-e from its list.
pub(crate) struct NfsesEmitidas {
    pub(crate) service: NfeService,
}

impl ViewController for NfsesEmitidas {
    fn id(&self) -> &str {
        "NfseDocumentos"
    }

    fn scope(&self) -> TargetScope {
        TargetScope::any().entity(NFSE_DOCUMENTO)
    }

    fn actions(&self) -> Vec<Action> {
        let (reenviando, cancelando) = (self.service.clone(), self.service.clone());
        vec![
            Action::simple(VER_XML, "Ver XML").icon("code").category(ActionCategory::View).on_execute(|cx| {
                let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                let record = cx.selected.first().ok_or("selecione uma NFS-e")?;
                let xml = texto(record, "xml");
                session.present("XML da NFS-e", move || {
                    let shown = xml.clone();
                    Box::new(
                        TextArea::new(Binding::new(move || shown.clone(), |_: String| {}))
                            .read_only(true)
                            .rows(24),
                    ) as AnyView
                });
                Ok(())
            }),
            Action::simple(DANFSE, "DANFSe")
                .icon("printer")
                .category(ActionCategory::View)
                .target_criteria("[situacao] = 'Autorizada'", CriteriaMode::TrueForAll)
                .on_execute(|cx| {
                    let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                    let record = cx.selected.first().ok_or("selecione uma NFS-e")?;
                    mostrar_danfse(&session, record, &ReportWords::pt_br())
                }),
            Action::simple(REENVIAR, "Enviar de novo")
                .icon("refresh-cw")
                .category(ActionCategory::Edit)
                .target_criteria(
                    "[situacao] = 'Pendente' Or [situacao] = 'Enviando'",
                    CriteriaMode::TrueForAll,
                )
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                    let record = cx.selected.first().ok_or("selecione uma NFS-e")?;
                    reenviando
                        .reenviar_nfse(&session, &record.get("id"), |session, fim| dizer(session, &fim));
                    Ok(())
                }),
            Action::parametrized(CANCELAR, "Motivo do cancelamento da NFS-e", FieldKind::Text)
                .category(ActionCategory::View)
                .target_criteria("[situacao] = 'Autorizada'", CriteriaMode::TrueForAll)
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                    let record = cx.selected.first().ok_or("selecione uma NFS-e")?;
                    let ActionInput::Parameter(Value::Text(justificativa)) = &cx.input else {
                        return Err("diga o motivo do cancelamento".to_owned());
                    };
                    cancelando.cancelar_nfse(&session, &record.get("id"), justificativa, |session, ended| {
                        session.show_message(match ended {
                            Ok(()) => Message::new("NFS-e cancelada.", MessageKind::Success),
                            Err(why) => Message::new(why, MessageKind::Warning),
                        });
                    });
                    Ok(())
                }),
        ]
    }
}
