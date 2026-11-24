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
    create(file, name, Some(password), role)
}

/// What a user with no password keeps where the hash goes: no hash reads it, so no password
/// ever matches, and it signs in only with a key.
const NO_PASSWORD: &str = "!sem-senha";

/// Makes the user of an agent (#916): it comes in with its key only, so it has no password, and
/// the server spends no hash on one. Argon2 asks about 19 MiB for each hash, and the allocator kept
/// that much in each thread of HTTP that let an agent in: 20 MiB more for the second agent and the
/// third, for good.
pub(crate) fn create_keyed_user(file: &Path, name: &str, role: &str) -> Result<String, CiError> {
    create(file, name, None, role)
}

fn create(file: &Path, name: &str, password: Option<&str>, role: &str) -> Result<String, CiError> {
    let maker = users(file)?;
    let directory = UserDirectory::new(maker.clone(), roles()).map_err(failed)?;
    if directory.is_empty().map_err(failed)? {
        let password = password.ok_or_else(|| failed("o primeiro usuário, o administrador, tem senha"))?;
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
    let hash = match password {
        Some(password) => hash_password(password).map_err(failed)?,
        None => NO_PASSWORD.to_owned(),
    };
    user.set("password_hash", hash);
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

#[cfg(test)]
mod tests {
    use ravenrs_app::{Authenticator, Credentials};

    use super::*;

    #[test]
    fn the_user_of_an_agent_has_no_password_and_no_password_lets_it_in() {
        let dir = std::env::temp_dir().join(format!("raven-ci-pessoas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let file = dir.join(USERS_FILE);
        create_user(&file, "ana", "senha-da-ana-123", ROLE_ADMIN).expect("the administrator");
        // The first user must have a password: it is the administrator.
        let empty = dir.join("vazio.sqlite");
        assert!(create_keyed_user(&empty, "agente:x", ROLE_AGENT).is_err());

        create_keyed_user(&file, "agente:linux-1", ROLE_AGENT).expect("the agent's user");
        let directory = UserDirectory::new(users(&file).expect("users"), roles()).expect("a directory");
        for password in ["", "!sem-senha", "qualquer"] {
            let tried = Credentials::new("agente:linux-1", password);
            assert!(directory.authenticate(&tried).is_err(), "{password:?} let it in");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
