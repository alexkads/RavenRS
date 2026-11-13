//! The contract of `ravenrs-app-testkit`, in memory and in a file; `again` is a second connection.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::rc::Rc;

use ravenrs_app::{EntityInfo, ObjectSpace, SpaceMaker};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use ravenrs_app_testkit::contract::{self, Adapter, Capabilities, Store};
use rusqlite::Connection;

fn maker(connection: Rc<Connection>, entities: &[EntityInfo]) -> Result<SpaceMaker, String> {
    let mut builder = SqliteDatabase::builder(connection);
    for entity in entities {
        builder = builder
            .managed(entity.clone(), TableMap::new(entity.name.to_lowercase()))
            .map_err(|e| e.to_string())?;
    }
    let database = builder.build();
    Ok(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>))
}

/// In memory: one connection, so `again` is a second database over it, with its own numbering.
struct Memory;

impl Adapter for Memory {
    fn name(&self) -> &str {
        "ravenrs-app-sqlite (memory)"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::SQLITE
    }

    fn open(&self, _case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
        let connection = Rc::new(Connection::open_in_memory().map_err(|e| e.to_string())?);
        Ok(Store { space: maker(connection.clone(), entities)?, again: maker(connection, entities)? })
    }
}

/// In a file, with a second connection to it, as a second program would have.
struct File(PathBuf);

impl Adapter for File {
    fn name(&self) -> &str {
        "ravenrs-app-sqlite (file)"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::SQLITE
    }

    fn open(&self, case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
        let path = self.0.join(format!("{case}.db"));
        let _ = std::fs::remove_file(&path);
        let open = || Connection::open(&path).map(Rc::new).map_err(|e| e.to_string());
        Ok(Store { space: maker(open()?, entities)?, again: maker(open()?, entities)? })
    }
}

#[test]
fn sqlite_in_memory_keeps_the_contract() {
    contract::run(&Memory);
}

#[test]
fn sqlite_in_a_file_keeps_the_contract() {
    let folder = std::env::temp_dir().join(format!("ravenrs-contract-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder");
    contract::run(&File(folder.clone()));
    let _ = std::fs::remove_dir_all(&folder);
}
