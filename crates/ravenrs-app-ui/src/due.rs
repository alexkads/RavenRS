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
    let words = DigestWords::from_texts(&session.texts());
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
        // The list opens on the records the notice is about, by their keys.
        let narrowed = entity.key_field().and_then(|key| {
            found
                .keys
                .iter()
                .filter_map(|value| {
                    ravenrs_app::Criteria::parse(&format!("[{}] = {}", key.name, literal(value))).ok()
                })
                .reduce(ravenrs_app::Criteria::or)
        });
        let mut message = Message::new(format!("{} — {}", found.title, found.body), MessageKind::Warning);
        message.seconds = Some(12.0);
        message.action = Some((
            session.words().open.clone(),
            Rc::new(move || match &narrowed {
                Some(filter) => opener.open_view_filtered(&view, filter.clone()),
                None => opener.open_view(&view),
            }),
        ));
        session.show_message(message);
    }
}

/// The button, beside the account, that lets each person choose the notices of what falls due:
/// off, on the day, or one, three or seven days before, for each entity with a `DueDigest`. It
/// keeps the choice in the settings, for that person only. `None` when there is no entity with a
/// summary, no one signed in or nowhere to keep it.
pub(crate) fn choice_button(session: &Session) -> Option<ravenrs_widgets::IconButton> {
    let entities: Vec<ravenrs_app::EntityInfo> =
        session.every_entity().into_iter().filter(|entity| entity.due_digest.is_some()).collect();
    if entities.is_empty() || session.settings().is_none() || session.account().is_none() {
        return None;
    }
    let texts = session.texts();
    let title = texts.get("ui.due_choice");
    let opener = session.clone();
    Some(
        ravenrs_widgets::IconButton::new(ravenrs_widgets::IconName::Bell)
            .label(title.clone())
            .size(ravenrs_widgets::ButtonSize::Small)
            .on_click(move || {
                let (session, entities) = (opener.clone(), entities.clone());
                opener.present(title.clone(), move || choices(&session, &entities));
            }),
    )
}

/// One row for each entity: its name and the choice, read and kept in the person's settings.
fn choices(session: &Session, entities: &[ravenrs_app::EntityInfo]) -> ravenrs_widgets::AnyView {
    use ravenrs_app::{DIGEST_ADVANCE_DAYS, DigestPreference, set_digest_preference};
    use ravenrs_widgets::{HStack, SegmentedControl, Text, VStack, ViewExt};
    let texts = session.texts();
    let labels: Vec<String> = std::iter::once(texts.get("ui.due_off"))
        .chain(DIGEST_ADVANCE_DAYS.iter().map(|days| match days {
            0 => texts.get("ui.due_on_the_day"),
            1 => texts.get("ui.due_day_before"),
            n => format!("{n} {}", texts.get("ui.due_days_before")),
        }))
        .collect();
    let rows: Vec<ravenrs_widgets::AnyView> = entities
        .iter()
        .map(|entity| {
            let (reader, writer) = (session.clone(), session.clone());
            let (read_name, write_name) = (entity.name.clone(), entity.name.clone());
            let chosen = ravenrs_reactive::Binding::new(
                move || {
                    let (Some(store), Some(user)) = (reader.settings(), reader.account()) else { return 0 };
                    let kept = digest_preference(&*store, &user.name, &read_name);
                    if !kept.enabled {
                        return 0;
                    }
                    1 + DIGEST_ADVANCE_DAYS.iter().position(|days| *days == kept.advance_days).unwrap_or(2)
                },
                move |at: usize| {
                    let (Some(store), Some(user)) = (writer.settings(), writer.account()) else { return };
                    let kept = digest_preference(&*store, &user.name, &write_name);
                    let wanted = match at.checked_sub(1).and_then(|at| DIGEST_ADVANCE_DAYS.get(at)) {
                        Some(days) => DigestPreference::new(true, *days),
                        None => DigestPreference::new(false, kept.advance_days),
                    };
                    match set_digest_preference(&*store, &user.name, &user.name, &write_name, wanted) {
                        Ok(()) => writer.touch(),
                        Err(why) => writer.show_message(Message::new(why, MessageKind::Error)),
                    }
                },
            );
            Box::new(
                HStack::new((
                    Text::new(entity.caption.clone()).grow(1.0),
                    SegmentedControl::new(chosen, labels.clone()).label(entity.caption.clone()),
                ))
                .spacing(12.0),
            ) as ravenrs_widgets::AnyView
        })
        .collect();
    Box::new(VStack::new(rows).spacing(8.0).padding(16.0))
}

/// A key as the expressions write it: a number as is, anything else quoted.
fn literal(value: &ravenrs_app::Value) -> String {
    match value {
        ravenrs_app::Value::Number(number) => number.to_string(),
        other => format!("'{}'", ravenrs_app::key_text(other).unwrap_or_default().replace('\'', "''")),
    }
}
