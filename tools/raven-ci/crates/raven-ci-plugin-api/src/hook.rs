//! What happens to every build before and after it: the [`BuildHook`] point.

use serde_json::Value;

use crate::{BuildContext, BuildStatus, CiError};

/// Something done to every build, before it and after it: refusing a build when the disk is
/// almost full, keeping or removing its folder, measuring it.
///
/// **Its role.** A step does what a pipeline asks; a hook does what the server asks of every
/// build, whatever its pipeline says. It is how the plugin `workspace-cleanup` refuses a build
/// with no room on the disk, with no line of it in the core.
///
/// **How it works.** For every build, the core calls [`before`](Self::before) on each hook of the
/// plugins switched on, after the folder of the build is made and before the code is fetched,
/// with the job's settings under the hook's [`id`](Self::id) (`null` when the job says nothing). A
/// refusal ends the build as failed before any step, with the error as the reason, and the hooks
/// after it are not asked. When the publishers are done, the core calls [`after`](Self::after) on
/// each hook with how the build ended; what it fails at goes to the build's log.
///
/// **How to write one.** Keep `before` quick: every build waits for it. Say in the error what to
/// do (free space, change a setting), since it is what people read on the build.
pub trait BuildHook: Send + Sync {
    /// What the job's settings name its settings by: `workspace`.
    fn id(&self) -> &str;

    /// Before the code of `build` is fetched: `Ok` lets it go on, an error refuses it.
    fn before(&self, build: &BuildContext, settings: &Value) -> Result<(), CiError> {
        let _ = (build, settings);
        Ok(())
    }

    /// After the publishers of `build`, which ended as `status`.
    fn after(&self, build: &BuildContext, status: BuildStatus, settings: &Value) -> Result<(), CiError> {
        let _ = (build, status, settings);
        Ok(())
    }
}
