//! Signing in through an identity provider outside the application, linked to local users by the
//! provider's issuer and subject, never by e-mail.
//!
//! The provider only says who someone is. What they can do always comes from the local user they
//! are linked to: a claim never grants a role. Unlinking ends what that sign-in reaches, also in a
//! session already open, on its next [`SecurityContext::reload`](crate::SecurityContext::reload).

use std::fmt;
use std::rc::Rc;

use crate::{
    AuthError, Authenticator, Credentials, EntityInfo, FieldInfo, FieldKind, Identity, ObjectSpace, Provider,
    Query, SECURITY_USER, SpaceError, UserDirectory, Value,
};

/// The entity of links between an outside identity and a local user.
pub const SECURITY_EXTERNAL_LOGIN: &str = "SecurityExternalLogin";

pub(crate) fn external_login_entity() -> EntityInfo {
    EntityInfo::new(SECURITY_EXTERNAL_LOGIN)
        .caption("External sign-ins")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("user_id", FieldKind::Reference(SECURITY_USER.to_owned())).caption("User"))
        .field(FieldInfo::new("issuer", FieldKind::Text).read_only())
        .field(FieldInfo::new("subject", FieldKind::Text).read_only())
}

/// Who a provider says someone is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Claims {
    /// Who vouches for it, such as `https://login.example.org`.
    pub issuer: String,
    /// The person's id at that issuer, which never changes.
    pub subject: String,
    /// Their e-mail, as the issuer says; never used to link.
    pub email: Option<String>,
    /// Their name, as the issuer says.
    pub name: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Claims {
    /// Someone known to this issuer by this subject.
    pub fn new(issuer: impl Into<String>, subject: impl Into<String>) -> Self {
        Self { issuer: issuer.into(), subject: subject.into(), email: None, name: None }
    }

    /// With the e-mail the issuer gives.
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

/// A provider that checks what someone gave to sign in and says who they are.
///
/// When it cannot be reached it answers [`SpaceError::Temporary`] or [`SpaceError::Unavailable`];
/// anything else it refuses is [`SpaceError::Denied`] or [`SpaceError::Refused`].
pub trait IdentityProvider: Provider {
    /// Who these credentials belong to.
    fn verify(&self, credentials: &Credentials) -> Result<Claims, SpaceError>;
}

/// Signing in through an [`IdentityProvider`]: only issuers it accepts, only identities linked to
/// an active local user, with that user's roles.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ExternalSignIn {
    directory: UserDirectory,
    provider: Rc<dyn IdentityProvider>,
    issuers: Vec<String>,
}

impl fmt::Debug for ExternalSignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalSignIn")
            .field("provider", &self.provider.name())
            .field("issuers", &self.issuers)
            .finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ExternalSignIn {
    /// Through this provider, over the users of this directory. It accepts no issuer until told.
    pub fn new(directory: UserDirectory, provider: Rc<dyn IdentityProvider>) -> Self {
        Self { directory, provider, issuers: Vec::new() }
    }

    /// Accepts identities from this issuer.
    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuers.push(issuer.into());
        self
    }
}

impl Authenticator for ExternalSignIn {
    fn authenticate(&self, credentials: &Credentials) -> Result<Identity, AuthError> {
        let claims = self.provider.verify(credentials).map_err(|error| {
            if error.is_temporary() || matches!(error, SpaceError::Unavailable(_)) {
                AuthError::Unavailable(format!("{} cannot be reached now", self.provider.name()))
            } else {
                AuthError::Rejected
            }
        })?;
        if !self.issuers.contains(&claims.issuer) {
            return Err(AuthError::Rejected);
        }
        let failed = |error: SpaceError| AuthError::Storage(error.to_string());
        let user = self.directory.linked_user(&claims.issuer, &claims.subject).map_err(failed)?;
        let user = user.ok_or(AuthError::Rejected)?;
        self.directory.identity_by_key(&user).map_err(failed)?.ok_or(AuthError::Rejected)
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl UserDirectory {
    /// The local user an outside identity is linked to, if any.
    pub fn linked_user(&self, issuer: &str, subject: &str) -> Result<Option<Value>, SpaceError> {
        let space = self.raw();
        Ok(find_link(space.as_ref(), issuer, subject)?.map(|link| link.get("user_id")))
    }

    /// Links an outside identity to a local user. Linking it again to the same user changes
    /// nothing; one already linked to someone else is a conflict. Who may do this is up to the
    /// caller.
    pub fn link_external(&self, user: &Value, issuer: &str, subject: &str) -> Result<(), SpaceError> {
        let mut space = self.raw();
        if let Some(link) = find_link(space.as_ref(), issuer, subject)? {
            return if link.get("user_id") == *user {
                Ok(())
            } else {
                Err(SpaceError::Conflict("This outside identity is linked to another user.".to_owned()))
            };
        }
        let mut link = space.new_object(SECURITY_EXTERNAL_LOGIN)?;
        link.set("user_id", user.clone());
        link.set("issuer", issuer);
        link.set("subject", subject);
        space.save(link)?;
        space.commit()
    }

    /// Unlinks an outside identity: it no longer signs in, and a session it opened loses its
    /// permissions on its next reload.
    pub fn unlink_external(&self, issuer: &str, subject: &str) -> Result<(), SpaceError> {
        let mut space = self.raw();
        if let Some(link) = find_link(space.as_ref(), issuer, subject)? {
            space.delete(SECURITY_EXTERNAL_LOGIN, &link.get("id"))?;
            space.commit()?;
        }
        Ok(())
    }
}

/// Compared here, so an issuer or a subject never becomes part of an expression.
fn find_link(
    space: &dyn ObjectSpace,
    issuer: &str,
    subject: &str,
) -> Result<Option<crate::Object>, SpaceError> {
    Ok(space.query(SECURITY_EXTERNAL_LOGIN, &Query::all())?.into_iter().find(|link| {
        link.get("issuer") == Value::Text(issuer.to_owned())
            && link.get("subject") == Value::Text(subject.to_owned())
    }))
}
