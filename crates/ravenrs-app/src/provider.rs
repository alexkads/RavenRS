//! What a provider outside the application says it can do, and the smallest contract for sending
//! events to one. Failures use the meanings of [`SpaceError`]: refused for good, not allowed,
//! capability unavailable, temporary, or an outcome still unknown.

use crate::SpaceError;

/// The capability of answering [`EventSink::status`].
pub const CAN_TELL_STATUS: &str = "events.status";

/// Something outside the application that the application talks to: a messenger, a payment
/// service, a printer, a store of files.
pub trait Provider {
    /// What it is called, for messages and logs.
    fn name(&self) -> &str;

    /// What it can do, by name. The interface shows what is here and never assumes more.
    fn capabilities(&self) -> Vec<String> {
        Vec::new()
    }

    /// Whether it says it can do this.
    fn can(&self, capability: &str) -> bool {
        self.capabilities().iter().any(|own| own == capability)
    }
}

/// Where events go.
///
/// A destination that already has an event under a key keeps one effect. When the answer to a
/// send is lost, the send returns [`SpaceError::Unknown`]; whoever sends asks [`status`](Self::status)
/// before sending again, and only when the provider has [`CAN_TELL_STATUS`].
pub trait EventSink: Provider {
    /// Sends an event under this key.
    fn send(&self, key: &str, payload: &str) -> Result<(), SpaceError>;

    /// Whether the destination already has an event under this key. One without
    /// [`CAN_TELL_STATUS`] answers [`SpaceError::Unavailable`].
    fn status(&self, key: &str) -> Result<bool, SpaceError> {
        let _ = key;
        Err(SpaceError::Unavailable(format!("{} cannot tell what it received", self.name())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mute;

    impl Provider for Mute {
        fn name(&self) -> &str {
            "mute"
        }
    }

    impl EventSink for Mute {
        fn send(&self, _key: &str, _payload: &str) -> Result<(), SpaceError> {
            Err(SpaceError::Unknown("no answer".to_owned()))
        }
    }

    #[test]
    fn a_provider_has_only_the_capabilities_it_declares() {
        assert!(!Mute.can(CAN_TELL_STATUS));
        assert!(matches!(Mute.status("k"), Err(SpaceError::Unavailable(_))));
    }

    #[test]
    fn each_meaning_says_whether_to_try_again() {
        let lost = Mute.send("k", "").expect_err("no answer");
        assert!(lost.may_have_happened() && !lost.is_temporary());
        assert!(SpaceError::Temporary("busy".to_owned()).is_temporary());
        for settled in [
            SpaceError::Unavailable("off".to_owned()),
            SpaceError::Refused("no".to_owned()),
            SpaceError::Denied("no".to_owned()),
            SpaceError::Conflict("taken".to_owned()),
        ] {
            assert!(!settled.is_temporary() && !settled.may_have_happened(), "{settled:?}");
            assert!(!settled.to_string().is_empty());
        }
    }
}
