//! `Backups` and `BackupStamp`, every feature, over a SQLite file in a folder of its own in the
//! temporary folder: a backup made and checked, restored with a copy of what was there kept first,
//! copies the application keeps by itself up to a limit, and a backup of another tenant refused.

use ravenrs::prelude::AnyView;

/// The control's name, as the session knows it.
pub const ID: &str = "BackupsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Cópias de segurança";

/// What the page shows.
pub fn page() -> AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    return Box::new(desktop::BackupsPage);
    #[cfg(target_arch = "wasm32")]
    super::desktop_only(
        "As cópias de segurança são de um banco SQLite num arquivo: no desktop esta tela faz uma, confere, restaura e guarda cópias numa pasta.",
    )
}

/// Where the page keeps its database, its backups and its copies, one folder for each run.
#[cfg(not(target_arch = "wasm32"))]
pub fn folder() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("raven-centro-copias-{}", std::process::id()))
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use ravenrs::prelude::*;
    use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query};
    use ravenrs_app_sqlite::{BackupInfo, BackupStamp, Backups, SqliteBackups, SqliteDatabase, TableMap};

    const CONTACT: &str = "Contact";
    const APPLICATION: &str = "Raven Centro de Recursos";
    /// How many copies the folder keeps; the oldest go first.
    const KEPT: usize = 3;

    fn contacts() -> EntityInfo {
        EntityInfo::new(CONTACT)
            .caption("Contatos")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
    }

    /// The database of the page and its backups, of the tenant "matriz".
    #[derive(Clone)]
    struct Kept {
        database: SqliteDatabase,
        backups: Rc<Backups>,
    }

    fn stamp(tenant: &str) -> BackupStamp {
        BackupStamp::new(APPLICATION).tenant(tenant).version(env!("CARGO_PKG_VERSION"))
    }

    fn open() -> Result<Kept, String> {
        let folder = super::folder();
        // Each run starts over: what an earlier page left is of no use here.
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let connection = rusqlite::Connection::open(folder.join("contatos.db")).map_err(|e| e.to_string())?;
        let database = SqliteDatabase::builder(Rc::new(connection))
            .managed(contacts(), TableMap::new("contatos"))
            .map_err(|e| e.0)?
            .build();
        let backups = database.backups(stamp("matriz")).copies_in(folder.join("copias")).keep(KEPT);
        Ok(Kept { database, backups: Rc::new(backups) })
    }

    fn names(database: &SqliteDatabase) -> Vec<String> {
        let found = database.space().query(CONTACT, &Query::all()).unwrap_or_default();
        found.iter().map(|contact| contact.get("name").to_text()).collect()
    }

    fn rows(info: &BackupInfo) -> String {
        let rows: Vec<String> = info.rows.iter().map(|(table, count)| format!("{table}: {count}")).collect();
        rows.join(", ")
    }

    fn backup_file(number: usize) -> PathBuf {
        super::folder().join(format!("backup-{number}.db"))
    }

    fn backups_made() -> usize {
        (1..).take_while(|number| backup_file(*number).exists()).count()
    }

    fn last_backup() -> Option<PathBuf> {
        Some(backups_made()).filter(|made| *made > 0).map(backup_file)
    }

    fn describe(backups: &Backups, file: &Path) -> String {
        match backups.inspect(file) {
            Ok(info) => format!(
                "Backup de {}, filial {}, versão {}: {}",
                info.stamp.application,
                info.stamp.tenant.as_deref().unwrap_or("nenhuma"),
                info.stamp.version.as_deref().unwrap_or("?"),
                rows(&info)
            ),
            Err(why) => format!("Não serve: {why}"),
        }
    }

    /// The page: a database in a file, its backups and its copies.
    #[derive(Debug, Default)]
    pub struct BackupsPage;

    impl View for BackupsPage {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let kept = cx.use_state(|| open().map_err(|why| format!("O banco não abriu: {why}")));
            let said = cx.use_state(|| "Acrescente um contato e faça um backup.".to_owned());
            let Ok(now) = kept.get() else {
                let why = kept.get().err().unwrap_or_default();
                return cx.child(&Text::new(why));
            };

            let adding = (now.clone(), said.clone(), kept.clone());
            let add = Button::new("Acrescentar contato").on_click(move || {
                let count = names(&adding.0.database).len() + 1;
                let mut space = adding.0.database.space();
                let added = space.new_object(CONTACT).and_then(|mut contact| {
                    contact.set("name", format!("Contato {count}"));
                    space.save(contact)?;
                    space.commit()
                });
                adding.1.set(match added {
                    Ok(()) => format!("Contato {count} gravado"),
                    Err(why) => format!("Não gravou: {why}"),
                });
                adding.2.set(Ok(adding.0.clone()));
            });
            let saving = (now.clone(), said.clone());
            let save = Button::new("Fazer backup").style(ButtonStyle::Secondary).on_click(move || {
                let number = backups_made() + 1;
                let file = backup_file(number);
                saving.1.set(match saving.0.backups.save(&file) {
                    Ok(info) => format!("Backup {number} feito e conferido: {}", rows(&info)),
                    Err(why) => format!("O backup não foi feito: {why}"),
                });
            });
            let inspecting = (now.clone(), said.clone());
            let inspect =
                Button::new("Conferir o backup").style(ButtonStyle::Secondary).on_click(move || {
                    inspecting.1.set(match last_backup() {
                        Some(file) => describe(&inspecting.0.backups, &file),
                        None => "Faça um backup antes".to_owned(),
                    });
                });
            let restoring = (now.clone(), said.clone(), kept.clone());
            let restore =
                Button::new("Restaurar o backup").style(ButtonStyle::Secondary).on_click(move || {
                    let Some(file) = last_backup() else {
                        restoring.1.set("Faça um backup antes".to_owned());
                        return;
                    };
                    restoring.1.set(match restoring.0.backups.restore(&file) {
                        Ok((info, Some(before))) => format!(
                            "Restaurado ({}); o que havia antes ficou em {}",
                            rows(&info),
                            before
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        ),
                        Ok((info, None)) => format!("Restaurado ({})", rows(&info)),
                        Err(why) => format!("Nada mudou: {why}"),
                    });
                    restoring.2.set(Ok(restoring.0.clone()));
                });
            let copying = (now.clone(), said.clone(), kept.clone());
            let copy = Button::new("Guardar uma cópia").style(ButtonStyle::Secondary).on_click(move || {
                copying.1.set(match copying.0.backups.keep_copy("manual") {
                    Ok(_) => format!("Cópia guardada; a pasta fica com no máximo {KEPT}"),
                    Err(why) => format!("A cópia não foi guardada: {why}"),
                });
                copying.2.set(Ok(copying.0.clone()));
            });
            let other = (now.clone(), said.clone());
            let elsewhere =
                Button::new("Restaurar na filial").style(ButtonStyle::Secondary).on_click(move || {
                    let Some(file) = last_backup() else {
                        other.1.set("Faça um backup antes".to_owned());
                        return;
                    };
                    // The same database, as the branch office would open it: a backup of the head
                    // office is not its own.
                    let branch = other.0.database.backups(stamp("filial"));
                    other.1.set(match branch.restore(&file) {
                        Ok(_) => "A filial restaurou o backup da matriz".to_owned(),
                        Err(why) => format!("A filial recusou: {why}"),
                    });
                });

            let copies = now.backups.copies();
            let shown = vec![
                format!("Contatos: {}", names(&now.database).len()),
                format!("Backups na pasta: {}", backups_made()),
                format!(
                    "Cópias guardadas: {} ({})",
                    copies.len(),
                    copies.iter().map(|copy| copy.reason.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ];
            let view = super::super::layout(
                cx,
                "Um banco SQLite num arquivo e seus backups, carimbados com a aplicação, a filial e a versão. Restaurar confere o carimbo e guarda antes uma cópia do que havia.",
                vec![
                    Box::new(add),
                    Box::new(save),
                    Box::new(inspect),
                    Box::new(restore),
                    Box::new(copy),
                    Box::new(elsewhere),
                ],
                shown,
                said.get(),
            );
            cx.child(view.as_ref())
        }
    }
}
