//! `SqlBuilder` and `TableMap`, every feature, over SQLite in memory: an entity on a table that
//! already exists, with columns of other names, a field the database works out, what is done to a
//! record before it is written and after it is read, a migration that runs once, and the words of
//! a refusal in Portuguese.

use ravenrs::prelude::AnyView;

/// The control's name, as the session knows it.
pub const ID: &str = "SqlMappingDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Mapeamento SQL";

/// What the page shows.
pub fn page() -> AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    return Box::new(desktop::SqlPage);
    #[cfg(target_arch = "wasm32")]
    super::desktop_only(
        "O mapeamento vale para qualquer banco SQL; esta tela usa um SQLite em memória, que só o desktop tem.",
    )
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::rc::Rc;

    use ravenrs::prelude::*;
    use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Value};
    use ravenrs_app_sqlite::{SqlWords, SqliteDatabase, TableMap};

    const CLIENT: &str = "LegacyClient";
    const TAG: &str = "ClientTag";

    /// The table of an older system, as it was found: its names, its data with spaces around.
    const LEGACY: &str = "
        CREATE TABLE clientes_legado (cod INTEGER PRIMARY KEY, nome_cli TEXT NOT NULL, uf TEXT);
        INSERT INTO clientes_legado VALUES (1, '  Ana Lima  ', 'PE'), (2, 'Bia Rocha', 'SP');
    ";

    fn clients() -> EntityInfo {
        EntityInfo::new(CLIENT)
            .caption("Clientes do sistema antigo")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
            .field(FieldInfo::new("state", FieldKind::Text).caption("UF"))
            .field(FieldInfo::new("letters", FieldKind::Integer).caption("Letras").read_only())
    }

    fn tags() -> EntityInfo {
        EntityInfo::new(TAG)
            .caption("Etiquetas")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Etiqueta").default_property())
    }

    #[derive(Clone)]
    struct Mapped {
        connection: Rc<rusqlite::Connection>,
        database: SqliteDatabase,
    }

    fn open() -> Result<Mapped, String> {
        let connection = Rc::new(rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?);
        connection.execute_batch(LEGACY).map_err(|e| e.to_string())?;
        let map = TableMap::new("clientes_legado")
            .column("id", "cod")
            .column("name", "nome_cli")
            .column("state", "uf")
            // Worked out by the database, never stored.
            .computed("letters", "length(trim(nome_cli))")
            // The old system wants the state in capitals, whatever is typed.
            .before_save(|client| {
                let state = client.get("state").to_text().to_uppercase();
                client.set("state", state);
            })
            // And it kept names with spaces around, which nobody wants to see.
            .after_load(|client| {
                let name = client.get("name").to_text().trim().to_owned();
                client.set("name", name);
            });
        let database = SqliteDatabase::builder(connection.clone())
            .words(SqlWords::pt_br())
            .mapped(clients(), map)
            .and_then(|builder| builder.managed(tags(), TableMap::new("etiquetas")))
            .and_then(|builder| {
                builder.migration(
                    "2026-11-etiquetas-unicas",
                    "CREATE UNIQUE INDEX etiquetas_nome ON etiquetas (name)",
                )
            })
            .map_err(|e| e.0)?
            .build();
        Ok(Mapped { connection, database })
    }

    fn listed(mapped: &Mapped) -> Vec<String> {
        let found = mapped.database.space().query(CLIENT, &Query::all().sort("id", true)).unwrap_or_default();
        found
            .iter()
            .map(|client| {
                format!(
                    "{} ({}, {} letras)",
                    client.get("name").to_text(),
                    client.get("state").to_text(),
                    client.get("letters").to_text()
                )
            })
            .collect()
    }

    fn raw_name(mapped: &Mapped) -> String {
        mapped
            .connection
            .query_row("SELECT nome_cli FROM clientes_legado WHERE cod = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap_or_default()
    }

    /// The page: the table of the old system through the model.
    #[derive(Debug, Default)]
    pub struct SqlPage;

    impl View for SqlPage {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let mapped = cx.use_state(|| open().map_err(|why| format!("O banco não abriu: {why}")));
            let said = cx.use_state(|| "Grave um cliente ou repita uma etiqueta.".to_owned());
            let Ok(now) = mapped.get() else {
                return cx.child(&Text::new(mapped.get().err().unwrap_or_default()));
            };

            let writing = (now.clone(), said.clone(), mapped.clone());
            let write = Button::new("Gravar cliente com uf minúscula").on_click(move || {
                let mut space = writing.0.database.space();
                let saved = space.new_object(CLIENT).and_then(|mut client| {
                    client.set("id", 3.0);
                    client.set("name", "Caio Dias");
                    client.set("state", "pe");
                    space.save(client)?;
                    space.commit()
                });
                let stored: String = writing
                    .0
                    .connection
                    .query_row("SELECT uf FROM clientes_legado WHERE cod = 3", [], |row| row.get(0))
                    .unwrap_or_default();
                writing.1.set(match saved {
                    Ok(()) => format!("Gravou Caio Dias; na tabela, uf = {stored}"),
                    Err(why) => format!("Não gravou: {why}"),
                });
                writing.2.set(Ok(writing.0.clone()));
            });
            let repeating = (now.clone(), said.clone());
            let repeat =
                Button::new("Repetir uma etiqueta").style(ButtonStyle::Secondary).on_click(move || {
                    let mut space = repeating.0.database.space();
                    let add = |space: &mut dyn ObjectSpace| -> Result<(), ravenrs_app::SpaceError> {
                        let mut tag = space.new_object(TAG)?;
                        tag.set("name", Value::Text("Atacado".into()));
                        space.save(tag)?;
                        space.commit()
                    };
                    let first = add(&mut space);
                    let second = add(&mut space);
                    repeating.1.set(match (first, second) {
                        (_, Err(why)) => format!("O banco recusou a segunda: {why}"),
                        (Err(why), _) => format!("O banco recusou: {why}"),
                        _ => "As duas foram gravadas".to_owned(),
                    });
                });

            let shown = vec![
                format!("Clientes: {}", listed(&now).join(", ")),
                format!(
                    "Na tabela, o nome do primeiro tem {} caracteres, com os espaços",
                    raw_name(&now).chars().count()
                ),
            ];
            let view = super::super::layout(
                cx,
                "Uma tabela de um sistema antigo, com outros nomes de coluna, lida pelo modelo: o número de letras vem do banco, a UF sai em maiúsculas e o nome volta sem espaços. As etiquetas ganham um índice único numa migração.",
                vec![Box::new(write), Box::new(repeat)],
                shown,
                said.get(),
            );
            cx.child(view.as_ref())
        }
    }
}
