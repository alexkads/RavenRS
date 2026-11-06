//! The module: the issuer, the documents issued and what can be done with them.

use ravenrs_app::{
    Action, ActionCategory, ActionInput, CriteriaMode, EntityInfo, FieldInfo, FieldKind, Message,
    MessageKind, Module, Notice, Rule, TargetScope, Value, ViewController,
};
use ravenrs_app_ui::Session;
use ravenrs_nfe::{Ambiente, ChaveAcesso};
use ravenrs_reactive::Binding;
use ravenrs_widgets::{AnyView, TextArea};

use crate::cupom::cupom_entity;
use crate::service::{NfeService, emitente_do_registro, emitente_salvo};

/// The entity of the issuer, which storage has to know.
pub const NFE_EMITENTE: &str = "NfeEmitente";
/// The entity of the documents issued, which storage has to know.
pub const NFE_DOCUMENTO: &str = "NfeDocumento";

/// The issuer: the one record of who is issuing.
pub fn emitente_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(NFE_EMITENTE)
        .caption("Emitente")
        .navigation("Fiscal")
        .icon("building")
        .singleton()
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("razao_social", "Razão social").default_property())
        .field(text("nome_fantasia", "Nome fantasia"))
        .field(text("cnpj", "CNPJ"))
        .field(text("inscricao_estadual", "Inscrição estadual"))
        .field(text("logradouro", "Logradouro"))
        .field(text("numero", "Número"))
        .field(text("bairro", "Bairro"))
        .field(text("codigo_municipio", "Código IBGE do município"))
        .field(text("municipio", "Município"))
        .field(text("uf", "UF"))
        .field(text("cep", "CEP"))
        .field(FieldInfo::new("serie", FieldKind::Integer).caption("Série").default_value(1.0))
        .field(
            FieldInfo::new("ultimo_nsu", FieldKind::Integer)
                .caption("Último NSU das notas recebidas")
                .default_value(0.0)
                .read_only(),
        )
        .field(
            FieldInfo::new("proxima_consulta", FieldKind::Integer)
                .caption("Próxima consulta de notas recebidas (segundos desde 1970)")
                .default_value(0.0)
                .read_only(),
        )
        .rule(Rule::required("razao_social"))
        .rule(Rule::required("cnpj"))
}

/// The documents issued, each one with where it came from, so a test never passes for a real one.
pub fn documento_entity() -> EntityInfo {
    let shown =
        |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption).read_only();
    EntityInfo::new(NFE_DOCUMENTO)
        .caption("Notas emitidas")
        .navigation("Fiscal")
        .icon("file-text")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(shown("modelo", "Modelo"))
        .field(FieldInfo::new("serie", FieldKind::Integer).caption("Série").read_only())
        .field(FieldInfo::new("numero", FieldKind::Integer).caption("Número").read_only().default_property())
        .field(shown("situacao", "Situação"))
        .field(shown("total", "Total"))
        .field(shown("ambiente", "Ambiente"))
        .field(shown("emitido_em", "Emissão"))
        .field(shown("chave", "Chave de acesso"))
        .field(shown("protocolo", "Protocolo"))
        .field(FieldInfo::new("c_stat", FieldKind::Integer).caption("Código").read_only())
        .field(shown("motivo", "Motivo"))
        .field(FieldInfo::new("xml", FieldKind::LongText).in_list(false).in_detail(false))
}

const CONSULTAR: &str = "Nfe.Consultar";
const CANCELAR: &str = "Nfe.Cancelar";
const VER_XML: &str = "Nfe.VerXml";

struct Documentos {
    service: NfeService,
}

fn texto(record: &ravenrs_app::Object, field: &str) -> String {
    match record.get(field) {
        Value::Text(text) => text,
        _ => String::new(),
    }
}

fn dizer(session: &Session, text: String, kind: MessageKind) {
    session.show_message(Message::new(text, kind));
}

impl ViewController for Documentos {
    fn id(&self) -> &str {
        "NfeDocumentos"
    }

    fn scope(&self) -> TargetScope {
        TargetScope::any().entity(NFE_DOCUMENTO)
    }

    fn actions(&self) -> Vec<Action> {
        let consulting = self.service.clone();
        let canceling = self.service.clone();
        vec![
            Action::simple(VER_XML, "Ver XML").icon("code").category(ActionCategory::View).on_execute(|cx| {
                let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                let record = cx.selected.first().ok_or("selecione uma nota")?;
                let xml = texto(record, "xml");
                session.present("XML da nota", move || {
                    let shown = xml.clone();
                    Box::new(
                        TextArea::new(Binding::new(move || shown.clone(), |_: String| {}))
                            .read_only(true)
                            .rows(24),
                    ) as AnyView
                });
                Ok(())
            }),
            Action::simple(CONSULTAR, "Consultar na SEFAZ")
                .icon("search")
                .category(ActionCategory::View)
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                    let record = cx.selected.first().ok_or("selecione uma nota")?;
                    let chave = ChaveAcesso::parse(&texto(record, "chave")).map_err(|e| e.to_string())?;
                    let found = emitente_salvo(&*cx.space.borrow()).ok_or("cadastre o emitente")?;
                    let (emitente, _) = emitente_do_registro(&found)?;
                    let emissor = consulting.emissor(emitente).map_err(|e| e.to_string())?;
                    session.run_background(
                        "Consultando a nota",
                        move |_| emissor.consultar(&chave).map_err(|e| e.to_string()),
                        |session, answer| match answer {
                            Ok(answer) => dizer(
                                session,
                                format!("{}: {}", answer.retorno.c_stat, answer.retorno.x_motivo),
                                MessageKind::Information,
                            ),
                            Err(why) => dizer(session, why, MessageKind::Error),
                        },
                    );
                    Ok(())
                }),
            Action::parametrized(CANCELAR, "Motivo do cancelamento", FieldKind::Text)
                .category(ActionCategory::View)
                .target_criteria("[situacao] = 'Autorizada'", CriteriaMode::TrueForAll)
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("esta ação precisa das telas")?;
                    let record = cx.selected.first().ok_or("selecione uma nota")?;
                    let ActionInput::Parameter(Value::Text(justificativa)) = &cx.input else {
                        return Err("diga o motivo do cancelamento".to_owned());
                    };
                    canceling.cancelar(
                        &session,
                        &texto(record, "chave"),
                        &texto(record, "protocolo"),
                        justificativa,
                        |session, ended| match ended {
                            Ok(()) => dizer(session, "Nota cancelada.".to_owned(), MessageKind::Success),
                            Err(why) => dizer(session, why, MessageKind::Warning),
                        },
                    );
                    Ok(())
                }),
        ]
    }
}

/// Issuing NF-e and NFC-e, in beta. Storage has to know what [`NfeModule::entidades`] gives.
#[derive(Debug)]
pub struct NfeModule {
    service: NfeService,
}

impl NfeModule {
    /// A module that issues through this service.
    pub fn new(service: NfeService) -> Self {
        Self { service }
    }

    /// The entities it brings, for storage.
    pub fn entidades() -> Vec<EntityInfo> {
        vec![emitente_entity(), documento_entity(), cupom_entity()]
    }
}

impl Module for NfeModule {
    fn name(&self) -> &str {
        "Nfe"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        Self::entidades()
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(Documentos { service: self.service.clone() })]
    }

    fn notices(&self) -> Vec<Notice> {
        let mut notices = vec![Notice::warning(
            "BETA",
            "Emissor experimental em prova de conceito, não homologado. Não use para vender de verdade.",
        )];
        notices.push(match self.service.ambiente() {
            Ambiente::Sandbox => Notice::info(
                "SANDBOX",
                "Nenhuma nota é enviada à SEFAZ. Tudo aqui é simulado e sem valor fiscal.",
            ),
            Ambiente::Homologacao => Notice::danger(
                "HOMOLOGAÇÃO",
                "As notas vão aos servidores de teste da SEFAZ com o seu certificado, sem valor fiscal.",
            ),
        });
        notices
    }
}
