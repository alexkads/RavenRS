//! Settings kept by scope: for everyone, or for one person. Unlike a custom field, a setting is not
//! part of any record and adds no column: it is JSON under a name, as a chosen theme is.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::security::SpaceMaker;
use crate::{EntityInfo, FieldInfo, FieldKind, Value};

/// The entity that keeps settings, for storage to know.
pub const APP_SETTING: &str = "AppSetting";

/// The entity that keeps settings.
pub fn settings_entity() -> EntityInfo {
    EntityInfo::new(APP_SETTING)
        .caption("Settings")
        .field(FieldInfo::new("id", FieldKind::Text).typed_key())
        .field(FieldInfo::new("value", FieldKind::LongText))
}

/// Whose a setting is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum SettingScope {
    /// Everyone's.
    Global,
    /// This person's.
    User(String),
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SettingScope {
    fn slot(&self, name: &str) -> String {
        match self {
            Self::Global => format!("global/{name}"),
            Self::User(user) => format!("user/{user}/{name}"),
        }
    }
}

/// Where settings are kept.
pub trait SettingsStore {
    /// What this setting holds, if anything.
    fn get(&self, scope: &SettingScope, name: &str) -> Result<Option<String>, String>;
    /// Keeps this.
    fn set(&self, scope: &SettingScope, name: &str, value: &str) -> Result<(), String>;
    /// Forgets it.
    fn remove(&self, scope: &SettingScope, name: &str) -> Result<(), String>;
}

/// Settings for as long as the program runs.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MemorySettings(Rc<RefCell<BTreeMap<String, String>>>);

impl SettingsStore for MemorySettings {
    fn get(&self, scope: &SettingScope, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.borrow().get(&scope.slot(name)).cloned())
    }

    fn set(&self, scope: &SettingScope, name: &str, value: &str) -> Result<(), String> {
        self.0.borrow_mut().insert(scope.slot(name), value.to_owned());
        Ok(())
    }

    fn remove(&self, scope: &SettingScope, name: &str) -> Result<(), String> {
        self.0.borrow_mut().remove(&scope.slot(name));
        Ok(())
    }
}

/// Settings as records of [`settings_entity`], in whatever storage the application has.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RecordSettings(SpaceMaker);

impl std::fmt::Debug for RecordSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordSettings").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RecordSettings {
    /// Over what this makes, with no permissions around it: who may reach which setting is for
    /// whoever calls this to decide.
    pub fn new(space: SpaceMaker) -> Self {
        Self(space)
    }
}

impl SettingsStore for RecordSettings {
    fn get(&self, scope: &SettingScope, name: &str) -> Result<Option<String>, String> {
        let found =
            (self.0)().find(APP_SETTING, &Value::Text(scope.slot(name))).map_err(|e| e.to_string())?;
        Ok(found.and_then(|record| match record.get("value") {
            Value::Text(text) => Some(text),
            _ => None,
        }))
    }

    fn set(&self, scope: &SettingScope, name: &str, value: &str) -> Result<(), String> {
        let wrong = |error: crate::SpaceError| error.to_string();
        let mut space = (self.0)();
        let mut record = space.new_object(APP_SETTING).map_err(wrong)?;
        record.set("id", scope.slot(name));
        record.set("value", value);
        space.save(record).map_err(wrong)?;
        space.commit().map_err(wrong)
    }

    fn remove(&self, scope: &SettingScope, name: &str) -> Result<(), String> {
        let wrong = |error: crate::SpaceError| error.to_string();
        let mut space = (self.0)();
        space.delete(APP_SETTING, &Value::Text(scope.slot(name))).map_err(wrong)?;
        space.commit().map_err(wrong)
    }
}
