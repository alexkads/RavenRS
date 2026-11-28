//! The summary of what falls due, as a message when the session opens: one per entity with a
//! `DueDigest`, as each person may see it and as they chose (#860).

use std::rc::Rc;

use ravenrs_app::{DigestWords, Message, MessageKind, Operation, Query, digest_of, digest_preference};

use crate::Session;

/// Shows, for each entity with a due date summed up, what is overdue and what falls due in the
/// days the person chose, with a button that opens its list. Whom may not read the entity, or
/// turned it off, gets nothing; whom may not read the amount gets no amount.
pub fn show_due_digests(session: &Session) {
    let today = ravenrs_app::local_today();
    let words =
        if session.words().decimal == ',' { DigestWords::portuguese() } else { DigestWords::english() };
    let user = session.account().map(|account| account.name);
    for entity in session.every_entity() {
        let Some(digest) = &entity.due_digest else { continue };
        let preference = match (session.settings(), &user) {
            (Some(store), Some(user)) => digest_preference(&*store, user, &entity.name),
            _ => ravenrs_app::DigestPreference::default(),
        };
        if !preference.enabled {
            continue;
        }
        let security = session.frame().security();
        let can = |operation: Operation, field: Option<&str>| {
            security.as_ref().is_none_or(|security| {
                security.with(|p| match field {
                    Some(field) => p.can_field(operation, &entity.name, field, None),
                    None => p.can(operation, &entity.name, None),
                })
            })
        };
        if !can(Operation::Read, None) {
            continue;
        }
        let show_amounts = digest.amount.as_deref().is_some_and(|field| can(Operation::Read, Some(field)));
        let records = session.frame().space().borrow().query(&entity.name, &Query::all()).unwrap_or_default();
        let Some(found) = digest_of(&entity, &records, today, preference.advance_days, show_amounts, &words)
        else {
            continue;
        };
        let (opener, view) = (session.clone(), format!("{}_ListView", entity.name));
        let mut message = Message::new(format!("{} — {}", found.title, found.body), MessageKind::Warning);
        message.seconds = Some(12.0);
        message.action = Some((session.words().open.clone(), Rc::new(move || opener.open_view(&view))));
        session.show_message(message);
    }
}
