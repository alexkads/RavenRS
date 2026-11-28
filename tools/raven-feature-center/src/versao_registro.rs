//! The version of a record (#241): two people open the same document, the first keeps it, and the
//! second, still on the version read before, is refused with `SpaceError::Outdated` instead of
//! overwriting in silence. Around it, what an `EntityInfo` and its `FieldInfo`s declare and work
//! out: keys, where each field shows, files, calculated fields, appearance by what a record holds.
use ravenrs::prelude::*;
use ravenrs_app::{
    AppearanceRule, AppearanceTarget, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, Object, ObjectSpace,
    SpaceError, Value, ViewInfo, ViewKind,
};
use ravenrs_criteria::{Criteria, validate};

/// The control registered in the center.
pub const ID: &str = "RowVersionDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Versão do registro";

/// The document both people edit.
fn document() -> EntityInfo {
    EntityInfo::new("Documento")
        .caption("Documentos")
        .form_columns(2)
        .list_criteria("[active] = True")
        .field(FieldInfo::new("code", FieldKind::Text).typed_key().in_lookup(true))
        .field(FieldInfo::new("title", FieldKind::Text).wide().autofocus().in_list(true))
        .field(FieldInfo::new("pages", FieldKind::Integer).in_detail(true))
        .field(FieldInfo::new("sheets", FieldKind::Integer).calculated("[pages] * 2"))
        .field(
            FieldInfo::new("file", FieldKind::File).accepted(["pdf"]).most_bytes(1_000_000).non_cloneable(),
        )
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("version", FieldKind::Integer).row_version())
        .appearance(AppearanceRule::new("revisado", ["title"]).criteria("[version] > 1").enabled(false))
}

fn database() -> MemoryDatabase {
    let database = MemoryDatabase::new(vec![document()]).unwrap_or_else(|e| panic!("{e}"));
    let mut space = database.space();
    if let Ok(mut record) = space.new_object("Documento") {
        record.set("code", "D-1");
        record.set("title", "Contrato");
        record.set("pages", Value::Number(3.0));
        record.set("active", true);
        let _ = space.save(record);
        let _ = space.commit();
    }
    database
}

fn key() -> Value {
    Value::Text("D-1".into())
}

fn read(database: &MemoryDatabase) -> Option<Object> {
    database.space().find("Documento", &key()).ok().flatten()
}

/// What the entity declares and works out for the record as it is kept now.
fn declared(database: &MemoryDatabase) -> Vec<String> {
    let entity = document();
    let mut lines = vec![
        format!(
            "Campo da versão (row_version_field): {}",
            entity.row_version_field().map_or("nenhum", |f| &f.name)
        ),
        format!(
            "Título (field_named): largo {}, foco {}, na lista {}",
            entity.field_named("title").is_some_and(|f| f.wide),
            entity.field_named("title").is_some_and(|f| f.autofocus),
            entity.field_named("title").is_some_and(|f| f.in_list),
        ),
        format!(
            "Arquivo: aceita {:?}, até {} bytes, fica ao clonar: {}",
            entity.field_named("file").map(|f| f.accepted.clone()).unwrap_or_default(),
            entity.field_named("file").and_then(|f| f.most_bytes).unwrap_or_default(),
            entity.field_named("file").is_some_and(|f| f.non_cloneable),
        ),
        format!(
            "Código digitado: {}, na escolha: {}; colunas do formulário: {}; lista só de {}",
            entity.field_named("code").is_some_and(|f| f.is_key && !f.read_only),
            entity.field_named("code").is_some_and(|f| f.in_lookup),
            entity.form_columns.unwrap_or_default(),
            entity.list_criteria.clone().unwrap_or_default(),
        ),
    ];
    let schema = entity.schema();
    let fits = |text: &str| Criteria::parse(text).is_ok_and(|c| validate(&c, &schema).is_empty());
    lines.push(format!(
        "[pages] > 1 cabe no esquema (schema): {}; [nada] > 1: {}",
        fits("[pages] > 1"),
        fits("[nada] > 1")
    ));
    if let Some(mut record) = read(database) {
        let version = entity.version_of(&record).unwrap_or_default();
        entity.compute(&mut record, &ravenrs_app::environment());
        lines.push(format!("Folhas calculadas (compute): {}", record.integer("sheets").unwrap_or_default()));
        let view = ViewInfo::new("Documento_DetailView", ViewKind::Detail, Some("Documento"));
        let look = entity.appearance_of(AppearanceTarget::Item, "title", &view, Some(&record));
        lines.push(format!("Título editável (appearance_of): {}", look.enabled.unwrap_or(true)));
        entity.strip_calculated(&mut record);
        lines.push(format!("Sem calculados (strip_calculated): {}", record.values().contains_key("sheets")));
        lines.insert(0, format!("Versão gravada (version_of): {version}"));
        lines.insert(1, format!("Título gravado: {}", record.text("title")));
    }
    lines
}

/// What two people keeping the same document says.
fn kept(result: Result<(), SpaceError>, who: &str) -> String {
    match result {
        Ok(()) => format!("{who}: gravado."),
        Err(SpaceError::Outdated { read, stored, .. }) => format!(
            "{who}: recusado, leu a versão {read} e a gravada é a {}. Nada foi gravado.",
            stored.unwrap_or_default()
        ),
        Err(other) => format!("{who}: {other}"),
    }
}

/// Every public feature of the row version, and what EntityInfo and FieldInfo declare, on one page.
#[derive(Debug, Default)]
pub struct RowVersionPage;

impl View for RowVersionPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let store = cx.use_state(database);
        // What Bia read when she opened the form, before Ana kept it.
        let opened = cx.use_state(|| None::<Object>);
        let said = cx.use_state(|| "Ana e Bia abrem o mesmo documento.".to_owned());
        let (open_store, open) = (store.clone(), opened.clone());
        let (ana_store, ana_said) = (store.clone(), said.clone());
        let (bia_store, bia_opened, bia_said) = (store.clone(), opened.clone(), said.clone());
        let (again_store, again_opened, again_said) = (store.clone(), opened.clone(), said.clone());
        let lines: Vec<Text> = declared(&store.get()).into_iter().map(Text::new).collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Cada gravação soma um na versão do registro. Quem grava sobre uma versão antiga é recusado, \
                     sem sobrescrever o que outra pessoa gravou.",
                )
                .foreground(Semantic::InkMuted),
                HStack::new((
                    Button::new("Bia abre").on_click(move || open.set(read(&open_store.get()))),
                    Button::new("Ana grava").on_click(move || {
                        let database = ana_store.get();
                        let mut space = database.space();
                        let result = match read(&database) {
                            Some(mut record) => {
                                record.set("title", "Contrato da Ana");
                                space.save(record).and_then(|()| space.commit())
                            }
                            None => Err(SpaceError::Conflict("o documento sumiu".into())),
                        };
                        ana_said.set(kept(result, "Ana"));
                        ana_store.set(database);
                    }),
                    Button::new("Bia grava").on_click(move || {
                        let database = bia_store.get();
                        let mut space = database.space();
                        let result = match bia_opened.get() {
                            Some(mut record) => {
                                record.set("title", "Contrato da Bia");
                                space.save(record).and_then(|()| space.commit())
                            }
                            None => Err(SpaceError::Conflict("a Bia ainda não abriu".into())),
                        };
                        bia_said.set(kept(result, "Bia"));
                        bia_store.set(database);
                    }),
                    Button::new("Recomeçar").on_click(move || {
                        again_store.set(database());
                        again_opened.set(None);
                        again_said.set("Ana e Bia abrem o mesmo documento.".to_owned());
                    }),
                ))
                .spacing(8.0),
                Text::new(said.get()),
                VStack::new(lines).spacing(4.0).align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
