//! The screens of access for a while: granting it with a reason and hours, the password shown
//! once, and revoking a user, with a confirmation (#857).


use ravenrs_app::{
    AccessError, Action, ActionCategory, Message, MessageKind, SECURITY_USER, Selection,
    TemporaryAccessPolicy, Value,
};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, FormField, NodeId, Text, TextField, Tone, VStack, View,
};

use crate::Session;
use crate::password::text_binding;
use crate::session::Words;

/// The id of the action that grants access for a while.
pub(crate) const GRANT_ACCESS: &str = "GrantTemporaryAccess";
/// The id of the action that revokes a user.
pub(crate) const REVOKE_USER: &str = "RevokeUser";

fn said(words: &Words, error: &AccessError) -> String {
    match error {
        AccessError::OwnUser => words.revoke_own.clone(),
        other => other.to_string(),
    }
}

/// The actions on the list of users: granting is there only with a policy.
pub(crate) fn actions(words: &Words, policy: Option<&TemporaryAccessPolicy>) -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(policy) = policy {
        let (words, policy) = (words.clone(), policy.clone());
        actions.push(
            Action::simple(GRANT_ACCESS, words.grant_access.clone())
                .category(ActionCategory::Edit)
                .target_entity(SECURITY_USER)
                .icon("clock")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the application's screens")?;
                    let form = GrantForm::new(session.clone(), words.clone(), policy.clone());
                    session.present(words.grant_access.clone(), move || Box::new(form.clone()) as AnyView);
                    Ok(())
                }),
        );
    }
    let words = words.clone();
    actions.push(
        Action::simple(REVOKE_USER, words.revoke_access.clone())
            .category(ActionCategory::Edit)
            .target_entity(SECURITY_USER)
            .selection(Selection::RequireSingle)
            .icon("user-x")
            .confirmation(words.revoke_question.clone())
            .confirmation_words(words.revoke_detail.clone(), words.revoke_answer.clone())
            .on_execute(move |cx| {
                let session = Session::of(cx).ok_or("this action needs the application's screens")?;
                let user = cx.selected.first().map_or(Value::Null, |user| user.get("id"));
                let context = session.frame().security().ok_or(words.revoke_own.clone())?;
                context.revoke_user(&user).map_err(|error| said(&words, &error))?;
                session.show_message(Message::new(words.revoked.clone(), MessageKind::Success));
                session.records_changed();
                Ok(())
            }),
    );
    actions
}

/// Asks the reason and the hours, then shows who to sign in as and the password, once.
#[derive(Clone)]
struct GrantForm {
    session: Session,
    words: Words,
    policy: TemporaryAccessPolicy,
    reason: State<String>,
    hours: State<String>,
    error: State<Option<String>>,
    granted: State<Option<String>>,
}

impl GrantForm {
    fn new(session: Session, words: Words, policy: TemporaryAccessPolicy) -> Self {
        Self {
            session,
            words,
            policy,
            reason: State::new(String::new()),
            hours: State::new("2".to_owned()),
            error: State::new(None),
            granted: State::new(None),
        }
    }

    fn send(&self) {
        let Some(context) = self.session.frame().security() else {
            self.error.set(Some(AccessError::NotAllowed.to_string()));
            return;
        };
        let hours = self.hours.peek().trim().parse::<u32>().unwrap_or(0);
        match context.grant_temporary_access(&self.policy, &self.reason.peek(), hours) {
            Ok(access) => {
                let (year, month, day) = access.expires_at.ymd();
                let (hour, minute, _) = access.expires_at.hms();
                let until = format!("{day:02}/{month:02}/{year} {hour:02}:{minute:02}");
                let text = self
                    .words
                    .access_granted
                    .replace("{0}", &access.user_name)
                    .replace("{1}", &until)
                    .replace("{2}", &access.password);
                self.error.set(None);
                self.granted.set(Some(text));
                self.session.records_changed();
            }
            Err(error) => self.error.set(Some(said(&self.words, &error))),
        }
    }
}

impl View for GrantForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = &self.words;
        if let Some(granted) = self.granted.get() {
            let closing = self.session.clone();
            let content = VStack::new((
                Alert::new(words.grant_access.clone(), granted).tone(Tone::Success),
                Text::new(words.password_once.clone()),
                Button::new(words.done.clone()).on_click(move || closing.close_presented()),
            ))
            .spacing(12.0)
            .align(Align::Stretch);
            return cx.child(&content);
        }
        let sending = self.clone();
        let error: Option<AnyView> = self
            .error
            .get()
            .map(|text| Box::new(Alert::new(words.grant_access.clone(), text).tone(Tone::Danger)) as AnyView);
        let content = VStack::new((
            error,
            FormField::new(
                words.access_reason.clone(),
                TextField::new(text_binding(&self.reason)).autofocus(),
            ),
            FormField::new(
                words.access_hours.replace("{0}", &self.policy.max_hours.to_string()),
                TextField::new(text_binding(&self.hours)),
            ),
            Button::new(words.grant_access.clone()).on_click(move || sending.send()),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        cx.child(&content)
    }
}
