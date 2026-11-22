//! The people of the server and their roles, in `usuarios.sqlite`: the bottom that the API, the
//! panel and the agents all read.
//!
//! # Its place
//!
//! It names no other module of the core. [`api`](crate::api), [`panel`](crate::panel) and
//! [`agents`](crate::agents) use it, and `api` gives it again at its old paths
//! (`api::users`, `api::create_user`, `api::roles`, `api::USERS_FILE`), so nothing outside changes.

use std::path::Path;
use std::rc::Rc;

use raven_ci_plugin_api::{CiError, ROLE_ADMIN, ROLE_AGENT, ROLE_READ, ROLE_RUN};
use ravenrs_app::{
    ObjectSpace, Query, Role, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker, UserDirectory,
    Value, hash_password, security_entities,
};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};

/// The file of people and roles, in the folder of data.
pub const USERS_FILE: &str = "usuarios.sqlite";

/// The roles of the CI, as the server knows them.
pub fn roles() -> Vec<Role> {
    vec![
        Role::new(ROLE_ADMIN).administrative(),
        Role::new(ROLE_RUN),
        Role::new(ROLE_READ),
        Role::new(ROLE_AGENT),
    ]
}

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(format!("os usuários: {e}"))
}

/// What makes units of work over the people in `file`: each thread that answers opens it.
pub fn users(file: &Path) -> Result<SpaceMaker, CiError> {
    let connection = Rc::new(rusqlite::Connection::open(file).map_err(failed)?);
    let builder =
        security_entities().into_iter().try_fold(SqliteDatabase::builder(connection), |builder, entity| {
            let table = TableMap::new(format!("ci_{}", entity.name.to_lowercase()));
            builder.managed(entity, table)
        });
    let database = builder.map_err(failed)?.build();
    Ok(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>))
}

/// Makes the person `name` with `password` and the role `role` in `file`. The first person is an
/// administrator whatever `role` says; after that, `role` has to be one of the CI's, and a name
/// already there is refused.
pub fn create_user(file: &Path, name: &str, password: &str, role: &str) -> Result<String, CiError> {
    let maker = users(file)?;
    let directory = UserDirectory::new(maker.clone(), roles()).map_err(failed)?;
    if directory.is_empty().map_err(failed)? {
        directory.create_first_administrator(name, password).map_err(failed)?;
        return Ok(ROLE_ADMIN.to_owned());
    }
    if !roles().iter().any(|known| known.name() == role) {
        return Err(CiError::settings(
            "papel",
            format!("{role} não existe; use {ROLE_ADMIN}, {ROLE_RUN} ou {ROLE_READ}"),
        ));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(CiError::settings("nome", "um usuário tem nome"));
    }
    if directory.identity_of(name).map_err(failed)?.is_some() {
        return Err(CiError::settings("nome", format!("já há o usuário {name}")));
    }
    let mut space = maker();
    let records = space.query(SECURITY_ROLE, &Query::all()).map_err(failed)?;
    let role_key = records
        .iter()
        .find(|record| record.get("name") == Value::Text(role.to_owned()))
        .map(|record| record.get("id"))
        .ok_or_else(|| failed(format!("o papel {role} não está no arquivo")))?;
    let mut user = space.new_object(SECURITY_USER).map_err(failed)?;
    user.set("name", name);
    user.set("password_hash", hash_password(password).map_err(failed)?);
    user.set("must_change_password", false);
    let key = user.get("id");
    space.save(user).map_err(failed)?;
    let mut link = space.new_object(SECURITY_USER_ROLE).map_err(failed)?;
    link.set("user_id", key);
    link.set("role_id", role_key);
    space.save(link).map_err(failed)?;
    space.commit().map_err(failed)?;
    Ok(role.to_owned())
}
