//! Reminders that come up by themselves when they are due. Nothing runs in between: the screen asks
//! to be built again at the next time due, and reads again when records change.

use std::collections::BTreeSet;

use ravenrs_app::{EntityInfo, Message, MessageKind, Object, Value, local_now, reminders_of};
use ravenrs_criteria::DateTime;
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Dialog, HStack, NodeId, Text, VStack, View,
};

use crate::Session;
use crate::values::display;

const SNOOZE_SECONDS: i64 = 15 * 60;

fn seconds_of(when: DateTime) -> i64 {
    when.days * 86_400 + i64::from(when.seconds)
}

fn moment_at(seconds: i64) -> DateTime {
    DateTime { days: seconds.div_euclid(86_400), seconds: seconds.rem_euclid(86_400) as u32 }
}

/// A record with a time set, as this person reads it.
#[derive(Clone)]
struct Pending {
    entity: EntityInfo,
    record: Object,
    key: String,
    at: i64,
}

fn pending(session: &Session) -> Vec<Pending> {
    let mut all = Vec::new();
    let space = session.frame().space();
    let space = space.borrow();
    let security = session.frame().security();
    for entity in session.entities_with_reminders() {
        let Some(reminder) = &entity.reminder else { continue };
        let found = match &security {
            Some(security) => security.with(|p| reminders_of(&*space, &entity, p.environment())),
            None => reminders_of(&*space, &entity, &ravenrs_app::environment()),
        };
        for record in found {
            let Value::Date(when) = record.get(&reminder.field) else { continue };
            let key = entity
                .key_field()
                .and_then(|field| ravenrs_app::key_text(&record.get(&field.name)))
                .unwrap_or_default();
            all.push(Pending {
                key: format!("{}:{key}", entity.name),
                at: seconds_of(when),
                entity: entity.clone(),
                record,
            });
        }
    }
    all
}

/// Changes when a record reminds, in a commit of its own. A person who cannot write only stops
/// seeing it for now.
fn set_time(session: &Session, item: &Pending, to: Value, hidden: &State<BTreeSet<String>>) {
    let Some(reminder) = &item.entity.reminder else { return };
    let Some(field) = item.entity.key_field() else { return };
    let space = session.frame().space();
    let mut space = space.borrow_mut();
    let result = (|| {
        let key = item.record.get(&field.name);
        let Some(mut stored) = space.find(&item.entity.name, &key)? else { return Ok(()) };
        stored.set(&reminder.field, to);
        space.save(stored)?;
        space.commit()
    })();
    if let Err(error) = result {
        space.rollback();
        drop(space);
        session.show_message(Message::new(error.to_string(), MessageKind::Warning));
        let key = item.key.clone();
        hidden.update(|all| {
            all.insert(key);
        });
    } else {
        drop(space);
    }
    session.records_changed();
}

/// What is due now, in a window, and when to look again.
pub(crate) struct Reminders {
    session: Session,
}

impl Reminders {
    pub(crate) fn new(session: Session) -> Self {
        Self { session }
    }

    fn row(&self, item: &Pending, hidden: &State<BTreeSet<String>>) -> AnyView {
        let session = &self.session;
        let words = session.words();
        let reminder = item.entity.reminder.as_ref();
        let field = reminder
            .and_then(|reminder| reminder.message.as_deref())
            .and_then(|name| item.entity.field_named(name))
            .or_else(|| item.entity.default_property());
        let what = field.map_or_else(String::new, |field| display(field, &item.record.get(&field.name)));
        let (opener, opened) = (session.clone(), item.clone());
        let open = Button::new(words.open.clone()).size(ButtonSize::Small).on_click(move || {
            opener.reminders().update(|(_, open)| *open = false);
            if let Some(field) = opened.entity.key_field() {
                opener.open_detail(&opened.entity.name, &opened.record.get(&field.name));
            }
        });
        let (later, snoozed, keep) = (session.clone(), item.clone(), hidden.clone());
        let snooze = Button::new(words.snooze.clone())
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(move || {
                let at = seconds_of(local_now()) + SNOOZE_SECONDS;
                set_time(&later, &snoozed, Value::Date(moment_at(at)), &keep);
            });
        let (ending, ended, keep) = (session.clone(), item.clone(), hidden.clone());
        let dismiss = Button::new(words.dismiss.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || set_time(&ending, &ended, Value::Null, &keep));
        Box::new(
            HStack::new((
                VStack::new((Text::new(what), Text::new(item.entity.caption.clone()))).spacing(2.0).grow(1.0),
                open,
                snooze,
                dismiss,
            ))
            .spacing(8.0)
            .align(Align::Center),
        )
    }
}

impl View for Reminders {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let revision = session.revision();
        let read: State<Option<(u64, Vec<Pending>)>> = cx.use_state(|| None);
        let hidden: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let seen: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let known = read.peek().as_ref().filter(|(at, _)| *at == revision).map(|(_, all)| all.clone());
        let all = known.unwrap_or_else(|| {
            let all = pending(session);
            read.set(Some((revision, all.clone())));
            all
        });
        let now = seconds_of(local_now());
        let left = hidden.get();
        let due: Vec<&Pending> =
            all.iter().filter(|item| item.at <= now && !left.contains(&item.key)).collect();
        if let Some(next) = all.iter().map(|item| item.at).filter(|at| *at > now).min() {
            // A whole second later, so it is due when this comes back.
            cx.wake_at(cx.now() + (next - now) as f64 + 1.0);
        }
        let (count, open) = *session.reminders().peek();
        // Something new comes up by itself; what was closed stays closed.
        let keys: BTreeSet<String> = due.iter().map(|item| item.key.clone()).collect();
        let arrived = keys.iter().any(|key| !seen.peek().contains(key));
        let open = (open || arrived) && !due.is_empty();
        if count != due.len() || open != session.reminders().peek().1 {
            session.reminders().set((due.len(), open));
        }
        if *seen.peek() != keys {
            seen.set(keys);
        }
        if !open {
            return cx.empty();
        }
        let (reading, writing) = (session.reminders().clone(), session.reminders().clone());
        let binding = Binding::new(
            move || reading.get().1,
            move |open: bool| writing.update(|(_, showing)| *showing = open),
        );
        let rows: Vec<AnyView> = due.iter().map(|item| self.row(item, &hidden)).collect();
        let dialog = Dialog::new(binding, session.words().reminders.clone())
            .content(VStack::new(rows).spacing(12.0).align(Align::Stretch));
        cx.child(&dialog)
    }
}
