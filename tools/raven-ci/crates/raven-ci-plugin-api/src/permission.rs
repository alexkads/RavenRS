//! What a plugin may ask the host for.

use ravenrs_app::ModuleManifest;
use serde::{Deserialize, Serialize};

/// Something a plugin may ask the host for. A plugin declares the ones it needs in its manifest,
/// as [`ModuleManifest::capabilities`] (`ci.network`, `ci.process`, `ci.secrets`, `ci.disk`), so
/// there is one place that says it. The host grants what was declared and the administrator did
/// not take away; each service of the [`Host`](crate::Host) checks its permission, and the
/// answer goes to the audit.
///
/// For a plugin compiled in, a permission is a contract checked at every service of the host and
/// a list a reviewer reads, not a sandbox: its code runs in the server's process. For an external
/// plugin, what it asks the host for over the protocol is checked the same way, and its process
/// starts with only the basic variables of the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Talking to other machines.
    Network,
    /// Starting programs.
    Process,
    /// Reading credentials and other secrets.
    Secrets,
    /// Its own folder and database on disk.
    Disk,
}

impl Permission {
    /// Every permission, in the order they are listed.
    pub const ALL: [Self; 4] = [Self::Network, Self::Process, Self::Secrets, Self::Disk];

    /// As a manifest declares it: `ci.network`.
    pub fn capability(self) -> &'static str {
        match self {
            Self::Network => "ci.network",
            Self::Process => "ci.process",
            Self::Secrets => "ci.secrets",
            Self::Disk => "ci.disk",
        }
    }

    /// Its name, as a `plugin.toml` writes it: `network`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Process => "process",
            Self::Secrets => "secrets",
            Self::Disk => "disk",
        }
    }

    /// What it lets a plugin do, in words for people.
    pub fn words(self) -> &'static str {
        match self {
            Self::Network => "rede",
            Self::Process => "rodar programas",
            Self::Secrets => "ler segredos",
            Self::Disk => "pasta e banco próprios",
        }
    }

    /// The permission of a capability such as `ci.disk`, or of a name such as `disk`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.capability() == text || p.name() == text)
    }

    /// The permissions a manifest declares, in the order of [`ALL`](Self::ALL).
    pub fn declared(manifest: &ModuleManifest) -> Vec<Self> {
        Self::ALL.into_iter().filter(|p| manifest.capabilities.iter().any(|c| c == p.capability())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_says_its_permissions_as_capabilities() {
        let manifest = ModuleManifest::new()
            .capability(Permission::Secrets.capability())
            .capability("events.status")
            .capability("ci.disk");
        assert_eq!(Permission::declared(&manifest), [Permission::Secrets, Permission::Disk]);
        assert_eq!(Permission::parse("network"), Some(Permission::Network));
        assert_eq!(Permission::parse("ci.process"), Some(Permission::Process));
        assert_eq!(Permission::parse("root"), None);
    }
}
