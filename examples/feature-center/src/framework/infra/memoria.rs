//! `MemoryDatabase`, every feature: records kept in a storage and read back when it opens again,
//! exported and imported as JSON, replaced by a copy from somewhere else, and a storage that fails
//! telling what went wrong instead of losing it quietly.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, MemoryDatabase, MemoryStorage, Object, ObjectSpace, Query, Storage,
    Value,
};

/// The control's name, as the session knows it.
pub const ID: &str = "MemoryDatabaseDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Banco em memória";

/// The name the records are kept under in the storage.
const KEPT_AS: &str = "anotacoes";
const NOTE: &str = "Note";

fn notes() -> EntityInfo {
    EntityInfo::new(NOTE)
        .caption("Anotações")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("text", FieldKind::Text).caption("Texto").default_property())
}

/// A storage that cannot write, as a full disk or a browser that refuses would be.
struct Full;

impl Storage for Full {
    fn read(&self, _name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }

    fn write(&self, _name: &str, _bytes: &[u8]) -> Result<(), String> {
        Err("o disco está cheio".to_owned())
    }

    fn remove(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Opens the database over this storage: what was kept there comes back.
fn open(storage: impl Storage + 'static) -> Result<MemoryDatabase, String> {
    let database = MemoryDatabase::new(vec![notes()]).map_err(|e| e.0)?;
    database.stored_in(storage, KEPT_AS).map_err(|e| e.to_string())
}

/// The notes over the storage of the page. It is in memory and its records are always ours, so
/// reading them back does not fail.
fn reopened(storage: &Rc<MemoryStorage>) -> MemoryDatabase {
    open(storage.clone()).unwrap_or_else(|why| panic!("the notes read back: {why}"))
}

fn texts(database: &MemoryDatabase) -> Vec<String> {
    let found = database.space().query(NOTE, &Query::all()).unwrap_or_default();
    found.iter().map(|note| note.get("text").to_text()).collect()
}

fn add(database: &MemoryDatabase, text: &str) -> Result<(), String> {
    let mut space = database.space();
    let mut note = space.new_object(NOTE).map_err(|e| e.to_string())?;
    note.set("text", text.to_owned());
    space.save(note).map_err(|e| e.to_string())?;
    space.commit().map_err(|e| e.to_string())
}

/// The page: a database kept in a storage, with what can be done to it.
#[derive(Debug, Default)]
pub struct MemoryPage;

impl View for MemoryPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let storage = cx.use_state(|| Rc::new(MemoryStorage::default()));
        let database = cx.use_state({
            let storage = storage.get();
            move || reopened(&storage)
        });
        let exported = cx.use_state(String::new);
        let said = cx.use_state(|| "Acrescente uma anotação e abra o banco de novo.".to_owned());

        let adding = (database.clone(), said.clone());
        let add_note = Button::new("Acrescentar anotação").on_click(move || {
            let count = texts(&adding.0.get()).len() + 1;
            match add(&adding.0.get(), &format!("Anotação {count}")) {
                Ok(()) => adding.1.set(format!("Anotação {count} gravada no armazenamento")),
                Err(why) => adding.1.set(format!("Não gravou: {why}")),
            }
        });
        let reopening = (database.clone(), storage.clone(), said.clone());
        let reopen = Button::new("Abrir de novo").style(ButtonStyle::Secondary).on_click(move || {
            let again = reopened(&reopening.1.get());
            let line = if again.was_restored() {
                format!("Reaberto: {} anotações voltaram do armazenamento", texts(&again).len())
            } else {
                "Reaberto vazio: não havia nada guardado".to_owned()
            };
            reopening.0.set(again);
            reopening.2.set(line);
        });
        let exporting = (database.clone(), exported.clone(), said.clone());
        let export = Button::new("Exportar JSON").style(ButtonStyle::Secondary).on_click(move || {
            let text = exporting.0.get().export_json();
            exporting.2.set(format!("Exportou {} caracteres de JSON", text.len()));
            exporting.1.set(text);
        });
        let importing = (database.clone(), exported.clone(), said.clone());
        let import = Button::new("Importar o exportado").style(ButtonStyle::Secondary).on_click(move || {
            let text = importing.1.get();
            if text.is_empty() {
                importing.2.set("Exporte antes de importar".to_owned());
                return;
            }
            match importing.0.get().import_json(&text) {
                Ok(count) => importing.2.set(format!("Importou {count} registros, no lugar dos que havia")),
                Err(why) => importing.2.set(format!("Recusou a importação: {why}")),
            }
        });
        let replacing = (database.clone(), said.clone());
        let replace =
            Button::new("Trocar pela cópia de fora").style(ButtonStyle::Secondary).on_click(move || {
                // A copy of what is kept somewhere else, put in with no rules to go through.
                let copy: Vec<Object> = ["Da matriz", "Da filial"]
                    .iter()
                    .enumerate()
                    .map(|(at, text)| {
                        let values = BTreeMap::from([
                            ("id".to_owned(), Value::Number(at as f64 + 100.0)),
                            ("text".to_owned(), Value::Text((*text).to_owned())),
                        ]);
                        Object::from_values(NOTE, values)
                    })
                    .collect();
                replacing.0.get().replace_records(NOTE, copy);
                replacing.1.set("Anotações trocadas pela cópia: Da matriz, Da filial".to_owned());
            });
        let failing = said.clone();
        let fail = Button::new("Gravar num disco cheio").style(ButtonStyle::Secondary).on_click(move || {
            let line = match open(Full) {
                Ok(database) => {
                    let _ = add(&database, "Não cabe");
                    match database.storage_problem() {
                        Some(why) => format!("O armazenamento recusou: {why}"),
                        None => "Gravou, sem problema".to_owned(),
                    }
                }
                Err(why) => format!("Não abriu: {why}"),
            };
            failing.set(line);
        });

        let now = database.get();
        let mut shown = vec![format!(
            "Anotações: {}",
            match texts(&now) {
                found if found.is_empty() => "nenhuma".to_owned(),
                found => found.join(", "),
            }
        )];
        shown.push(match now.storage_problem() {
            Some(why) => format!("Armazenamento: {why}"),
            None => "Armazenamento: cada gravação vai para ele".to_owned(),
        });
        let first = exported.get();
        if !first.is_empty() {
            shown.push(format!("JSON exportado: {}", first.lines().take(2).collect::<Vec<_>>().join(" ")));
        }
        let view = super::layout(
            cx,
            "Um banco em memória guardado num armazenamento: o que foi gravado volta quando ele abre de novo, sai e entra como JSON e pode ser trocado por uma cópia de fora.",
            vec![
                Box::new(add_note),
                Box::new(reopen),
                Box::new(export),
                Box::new(import),
                Box::new(replace),
                Box::new(fail),
            ],
            shown,
            said.get(),
        );
        cx.child(view.as_ref())
    }
}
