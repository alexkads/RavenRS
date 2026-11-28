//! Moving a tenant to another database (#243): a stamped backup of the tenant T1, read as an
//! export and written into a new database in one transaction, compared table by table, refused
//! to someone who is not an administrator and to the database of another tenant, and written in
//! the audit trail. Both ends are SQLite files here; the same `TenantImport` writes to PostgreSQL.
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    AuditSettings, AuditTrail, EntityInfo, FieldInfo, FieldKind, Identity, ObjectSpace, Permissions, Query,
    Role, RoleMerge, SECURITY_AUDIT_ENTRY, SpaceMaker, Value, audit_entity,
};
use ravenrs_app_sqlite::{BackupStamp, Backups, SqliteBackups, SqliteDatabase, TableMap, TenantImport};

/// The control registered in the center.
pub const ID: &str = "TenantMigrationDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Mover para outro banco";

/// Where the page keeps its databases, its backup and its copies.
fn folder() -> PathBuf {
    std::env::temp_dir().join(format!("raven-feature-center-migracao-{}", std::process::id()))
}

fn backup_file() -> PathBuf {
    folder().join("t1-backup.db")
}

fn model() -> [(EntityInfo, &'static str); 3] {
    let unit = EntityInfo::new("Unit")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text));
    let member = EntityInfo::new("Member")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("unit", FieldKind::Reference("Unit".into())));
    [(unit, "units"), (member, "members"), (audit_entity(), "audit")]
}

fn open(file: &Path) -> Result<SqliteDatabase, String> {
    let connection = Rc::new(rusqlite::Connection::open(file).map_err(|e| e.to_string())?);
    let mut builder = SqliteDatabase::builder(connection);
    for (entity, table) in model() {
        builder = builder.managed(entity, TableMap::new(table)).map_err(|e| e.to_string())?;
    }
    Ok(builder.migration("0001 membros", "SELECT 1").map_err(|e| e.to_string())?.build())
}

fn stamp(tenant: &str) -> BackupStamp {
    BackupStamp::new("Igreja").tenant(tenant).version("1.0")
}

/// T1, with two units and three members, made the first time it is asked for.
fn source() -> Result<SqliteDatabase, String> {
    let file = folder().join("t1.db");
    let new = !file.exists();
    std::fs::create_dir_all(folder()).map_err(|e| e.to_string())?;
    let database = open(&file)?;
    if new {
        let mut space = database.space();
        for (id, name) in [(1.0, "Centro"), (2.0, "Bairro")] {
            let mut unit = space.new_object("Unit").map_err(|e| e.to_string())?;
            unit.set("id", Value::Number(id));
            unit.set("name", name);
            space.insert(unit).map_err(|e| e.to_string())?;
        }
        for (id, name, unit) in [(1.0, "Ana", 1.0), (2.0, "Bia", 1.0), (3.0, "Caio", 2.0)] {
            let mut member = space.new_object("Member").map_err(|e| e.to_string())?;
            member.set("id", Value::Number(id));
            member.set("name", name);
            member.set("unit", Value::Number(unit));
            space.insert(member).map_err(|e| e.to_string())?;
        }
        space.commit().map_err(|e| e.to_string())?;
    }
    Ok(database)
}

fn backups(database: &SqliteDatabase) -> Backups {
    database.backups(stamp("T1")).copies_in(folder().join("copias")).keep(3)
}

fn administrator(database: &SqliteDatabase) -> Permissions {
    let identity = Identity::new(1, "ana", vec![Role::new("Administradores").administrative()]);
    Permissions::new(identity, database.entities(), RoleMerge::AnyRole)
}

fn operator(database: &SqliteDatabase) -> Permissions {
    Permissions::new(
        Identity::new(2, "bia", vec![Role::new("Operadores")]),
        database.entities(),
        RoleMerge::AnyRole,
    )
}

/// Saves the backup of T1 and says what its stamp holds.
fn save() -> Result<String, String> {
    let database = source()?;
    let backups = backups(&database);
    let _ = std::fs::remove_file(backup_file());
    backups.save(&backup_file()).map_err(|e| e.to_string())?;
    let info = backups.inspect(&backup_file()).map_err(|e| e.to_string())?;
    let rows: Vec<String> = info.rows.iter().map(|(table, n)| format!("{table} {n}")).collect();
    Ok(format!(
        "Backup de {} · {} · versão {} · {} migração(ões) · {}",
        info.stamp.application,
        info.stamp.tenant.unwrap_or_default(),
        info.stamp.version.unwrap_or_default(),
        info.stamp.migrations.len(),
        rows.join(", ")
    ))
}

/// Keeps a copy of T1 in the folder, which keeps the newest three.
fn keep_copy() -> Result<String, String> {
    let database = source()?;
    let backups = backups(&database);
    backups.keep_copy("manual").map_err(|e| e.to_string())?;
    Ok(format!("Cópias guardadas: {} (no máximo 3)", backups.copies().len()))
}

/// Moves the backup into a new database of `tenant`, as `who`.
fn migrate(tenant: &str, as_admin: bool) -> Result<String, String> {
    let database = source()?;
    if !backup_file().exists() {
        return Err("faça o backup primeiro".to_owned());
    }
    let export = backups(&database).export(&backup_file()).map_err(|e| e.to_string())?;
    let file = folder().join(format!("{}-novo.db", tenant.to_lowercase()));
    let _ = std::fs::remove_file(&file);
    let target = open(&file)?;
    let writer = target.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(writer.space()) as Box<dyn ObjectSpace>);
    let trail = Rc::new(AuditTrail::new(AuditSettings::default(), || "ana".to_owned()));
    trail.write_with(maker);
    let who = if as_admin { administrator(&target) } else { operator(&target) };
    let report = TenantImport::new(&target, stamp(tenant))
        .audit(trail)
        .run(&export, &who)
        .map_err(|e| format!("Recusado: {e}"))?;
    let tables: Vec<String> = report
        .tables
        .iter()
        .map(|t| {
            let ok = if t.matches() { "igual" } else { "diferente" };
            format!("{} {}→{} {ok}", t.table, t.source_rows, t.target_rows)
        })
        .collect();
    let audited = target.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).map_or(0, |lines| lines.len());
    Ok(format!(
        "Movido: {} linhas, {} referências conferidas · {} · auditoria: {audited} linha",
        report.rows(),
        report.references_checked,
        tables.join(", ")
    ))
}

/// Puts the backup back into T1, keeping a copy of what was there first.
fn restore() -> Result<String, String> {
    let database = source()?;
    let (info, copy) = backups(&database).restore(&backup_file()).map_err(|e| e.to_string())?;
    Ok(format!(
        "Restaurado o backup de {} · cópia de antes: {}",
        info.stamp.tenant.unwrap_or_default(),
        if copy.is_some() { "guardada" } else { "nenhuma" }
    ))
}

fn said(result: Result<String, String>) -> String {
    result.unwrap_or_else(|why| if why.starts_with("Recusado") { why } else { format!("Erro: {why}") })
}

/// BackupStamp, Backups and TenantImport on one page.
#[derive(Debug, Default)]
pub struct TenantMigrationPage;

impl View for TenantMigrationPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let message = cx.use_state(|| "Nada feito ainda.".to_owned());
        let button = |label: &'static str, primary: bool, run: fn() -> Result<String, String>| {
            let message = message.clone();
            Button::new(label)
                .style(if primary { ButtonStyle::Primary } else { ButtonStyle::Secondary })
                .on_click(move || message.set(said(run())))
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Um tenant que cresceu troca de banco sem reimplantar: o backup com carimbo vira uma exportação, que entra no banco novo numa transação só e é conferida tabela a tabela. SQLite e PostgreSQL usam o mesmo código; aqui os dois lados são arquivos SQLite.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                Text::new("Backup").style(TextStyle::Headline),
                HStack::new((
                    button("Fazer o backup de T1", true, save),
                    button("Guardar uma cópia", false, keep_copy),
                    button("Restaurar o backup", false, restore),
                ))
                .spacing(8.0),
                Text::new("Mover").style(TextStyle::Headline),
                HStack::new((
                    button("Mover para o banco novo", true, || migrate("T1", true)),
                    button("Mover como operador", false, || migrate("T1", false)),
                    button("Mover para o banco de T2", false, || migrate("T2", true)),
                ))
                .spacing(8.0),
                Text::new(message.get()).test_id("resultado"),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
