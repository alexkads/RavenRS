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
/// [`check_settings`](crate::check_settings). The core seals what was filled in its vault, with
/// what [`secret`](Self::secret) gives a plugin that asks for it by name and what
/// [`hidden`](Self::hidden) says a log must hide; when a plugin reads the credential, both go into
/// what every log hides.
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

    /// What [`Host::secret`](crate::Host::secret) gives a plugin that asks for a credential of this
    /// kind by its name: a token as it is, `user:password`, a private key. By default, the first
    /// field of [`fields`](Self::fields) marked `secret`, as text.
    fn secret(&self, credential: &Value) -> String {
        self.fields()
            .fields
            .iter()
            .find(|field| field.secret)
            .and_then(|field| credential.get(&field.name))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }
}
