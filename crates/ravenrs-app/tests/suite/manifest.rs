//! What modules say about themselves, checked when an application is put together.
#![allow(clippy::expect_used)]

use ravenrs_app::{
    Application, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, Module, ModuleManifest, ObjectSpace,
    SpaceError, Target, Value, Version, VersionRange,
};

struct Plain {
    name: &'static str,
    manifest: ModuleManifest,
    entity: Option<&'static str>,
}

impl Module for Plain {
    fn name(&self) -> &str {
        self.name
    }

    fn entities(&self) -> Vec<EntityInfo> {
        self.entity
            .map(|name| {
                EntityInfo::new(name)
                    .field(FieldInfo::new("id", FieldKind::Integer).key())
                    .field(FieldInfo::new("name", FieldKind::Text))
            })
            .into_iter()
            .collect()
    }

    fn manifest(&self) -> ModuleManifest {
        self.manifest.clone()
    }
}

fn module(name: &'static str, manifest: ModuleManifest) -> Plain {
    Plain { name, manifest, entity: None }
}

fn versioned(version: &str) -> ModuleManifest {
    ModuleManifest::new().version(version).expect("a version")
}

fn refused(builder: ravenrs_app::ApplicationBuilder, words: &[&str]) {
    let error = builder.build().expect_err("refused").0;
    for word in words {
        assert!(error.contains(word), "{error:?} does not say {word}");
    }
}

#[test]
fn versions_read_and_compare() {
    assert_eq!(Version::parse("1.2").expect("reads"), Version { major: 1, minor: 2, patch: 0 });
    assert!(Version::parse("one").is_err() && Version::parse("1.2.3.4").is_err());
    let range = VersionRange::compatible("1.2").expect("reads");
    assert!(range.accepts(Version::parse("1.9.9").expect("reads")));
    assert!(!range.accepts(Version::parse("2.0").expect("reads")));
    assert!(!range.accepts(Version::parse("1.1").expect("reads")));
}

#[test]
fn a_dependency_outside_the_accepted_range_is_refused_naming_both() {
    let needs = ModuleManifest::new().requires("Base", VersionRange::compatible("1.0").expect("reads"));
    refused(
        Application::builder("App").module(module("Base", versioned("2.0"))).module(module("Extra", needs)),
        &["Extra", "Base", "2.0.0"],
    );
}

#[test]
fn a_dependency_in_range_is_accepted_and_ordered_first() {
    let needs = ModuleManifest::new().requires("Base", VersionRange::compatible("1.0").expect("reads"));
    let app = Application::builder("App")
        .module(module("Extra", needs))
        .module(module("Base", versioned("1.4")))
        .build()
        .expect("fits");
    let names: Vec<String> = app.manifests().into_iter().map(|(name, _)| name).collect();
    assert_eq!(&names[..2], ["Base", "Extra"]);
}

#[test]
fn a_module_that_does_not_run_on_the_target_is_refused_and_no_target_checks_nothing() {
    let server = || module("Billing", ModuleManifest::new().target(Target::Server));
    refused(Application::builder("App").target(Target::Web).module(server()), &["Billing", "Web"]);
    assert!(Application::builder("App").module(server()).build().is_ok());
    assert!(Application::builder("App").target(Target::Server).module(server()).build().is_ok());
}

#[test]
fn storage_and_capabilities_the_host_lacks_are_refused() {
    let sqlite_only = || module("Local", ModuleManifest::new().database("sqlite"));
    refused(Application::builder("App").database("postgres").module(sqlite_only()), &["Local", "postgres"]);
    let needs_status = || module("Events", ModuleManifest::new().capability("events.status"));
    refused(Application::builder("App").module(needs_status()), &["Events", "events.status"]);
    assert!(Application::builder("App").capability("events.status").module(needs_status()).build().is_ok());
}

#[test]
fn the_same_migration_or_job_in_two_modules_is_refused() {
    let a = || module("A", ModuleManifest::new().migration("001-people").job("send"));
    refused(
        Application::builder("App")
            .module(a())
            .module(module("B", ModuleManifest::new().migration("001-people"))),
        &["001-people", "A", "B"],
    );
    refused(
        Application::builder("App").module(a()).module(module("B", ModuleManifest::new().job("send"))),
        &["job", "send"],
    );
}

#[test]
fn a_module_that_says_nothing_works_as_before() {
    assert!(
        Application::builder("App")
            .target(Target::Web)
            .database("sqlite")
            .module(module("Old", ModuleManifest::new()))
            .build()
            .is_ok()
    );
}

#[test]
fn a_switched_off_module_keeps_its_records_to_read_and_refuses_changes() {
    let notes = Plain { name: "Notes", manifest: ModuleManifest::new(), entity: Some("Note") };
    let app = Application::builder("App").module(notes).switch_off("Notes").build().expect("fits");
    assert_eq!(app.switched_off_entities(), ["Note".to_owned()]);
    let entity = app.entities().iter().find(|e| e.name == "Note").expect("kept");
    assert!(entity.fields.iter().all(|field| field.read_only));

    let database = MemoryDatabase::new(app.entities().to_vec()).expect("entities fit");
    let mut raw = database.space();
    let mut note = raw.new_object("Note").expect("entity");
    note.set("id", 1.0);
    note.set("name", "kept from before");
    raw.save(note).expect("waits");
    raw.commit().expect("kept");

    let mut guarded = app.guard(Box::new(database.space()));
    let kept = guarded.find("Note", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(kept.get("name"), Value::Text("kept from before".to_owned()));
    assert!(matches!(guarded.new_object("Note"), Err(SpaceError::Unavailable(_))));
    assert!(matches!(guarded.save(kept), Err(SpaceError::Unavailable(_))));
    assert!(matches!(guarded.delete("Note", &Value::Number(1.0)), Err(SpaceError::Unavailable(_))));
}

#[test]
fn switching_off_a_module_that_was_not_added_is_refused() {
    refused(Application::builder("App").switch_off("Ghost"), &["Ghost"]);
}
