//! A kind of credential: the [`CredentialKind`] point.

use ravenrs_app::EntityInfo;
use serde_json::Value;

/// A kind of credential: user and password, a token, an SSH key, the login of a registry, a
/// kubeconfig.
///
/// **Its role.** The vault keeps credentials cyphered; what a credential holds depends on its
/// kind, and so does what has to be hidden in a log.
///
/// **How it works.** The form of a new credential is [`fields`](Self::fields), checked by
/// [`check_settings`](crate::check_settings). When a build reads a credential, the core adds
/// what [`hidden`](Self::hidden) says to what the build's log hides.
///
/// **How to write one.** Mark the private fields as `secret` in the entity, so no screen shows
/// them, and return every value that could leak into a log from `hidden`, including derived
/// ones (a token in `user:token` form, for example).
pub trait CredentialKind: Send + Sync {
    /// What a credential names its kind by: `token`.
    fn id(&self) -> &str;

    /// What people see when they choose the kind.
    fn caption(&self) -> String;

    /// What a credential of this kind holds.
    fn fields(&self) -> EntityInfo;

    /// The values of `credential` that a log must never show.
    fn hidden(&self, credential: &Value) -> Vec<String>;
}
