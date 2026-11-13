//! The contract of `ravenrs-app-testkit` against a real PostgreSQL, when `RAVENRS_POSTGRES_URL`
//! says where one is: each case in a schema of its own, and `again` on a second connection.
//! `RAVENRS_POSTGRES_STATEMENTS=unprepared` runs it with statements as they go to a pooler.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::rc::Rc;

use postgres::{Client, NoTls};
use ravenrs_app::{EntityInfo, ObjectSpace, SpaceMaker};
use ravenrs_app_postgres::{PostgresBackend, PostgresDatabase, Statements, TableMap};
use ravenrs_app_testkit::contract::{self, Adapter, Capabilities, Store};

struct Postgres(String);

impl Postgres {
    fn connection(&self, schema: &str) -> Result<PostgresBackend, String> {
        let mut client = Client::connect(&self.0, NoTls).map_err(|e| e.to_string())?;
        client.batch_execute(&format!("SET search_path TO {schema}")).map_err(|e| e.to_string())?;
        let unprepared = std::env::var("RAVENRS_POSTGRES_STATEMENTS").is_ok_and(|mode| mode == "unprepared");
        Ok(PostgresBackend::from(client).with_statements(if unprepared {
            Statements::Unprepared
        } else {
            Statements::Prepared
        }))
    }

    fn maker(&self, schema: &str, entities: &[EntityInfo]) -> Result<SpaceMaker, String> {
        let mut builder = PostgresDatabase::builder(self.connection(schema)?);
        for entity in entities {
            builder = builder
                .managed(entity.clone(), TableMap::new(entity.name.to_lowercase()))
                .map_err(|e| e.to_string())?;
        }
        let database = builder.build();
        Ok(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>))
    }
}

impl Adapter for Postgres {
    fn name(&self) -> &str {
        "ravenrs-app-postgres"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::POSTGRES
    }

    fn open(&self, case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
        let schema = format!("contract_{case}");
        let mut admin = Client::connect(&self.0, NoTls).map_err(|e| e.to_string())?;
        admin
            .batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}"))
            .map_err(|e| e.to_string())?;
        Ok(Store { space: self.maker(&schema, entities)?, again: self.maker(&schema, entities)? })
    }
}

#[test]
fn postgres_keeps_the_contract() {
    let Some(url) = std::env::var("RAVENRS_POSTGRES_URL").ok() else { return };
    contract::run(&Postgres(url));
}
