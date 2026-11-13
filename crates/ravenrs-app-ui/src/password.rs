//! The form that changes a password, and the action that shows it for the person signed in.

use std::rc::Rc;

use crate::session::Session;
use crate::session::Words;
use ravenrs_app::{Message, MessageKind, PasswordError};
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{Alert, AnyView, BuildCx, Button, FormField, NodeId, TextField, Tone, VStack, View};

pub(crate) type Submit = Rc<dyn Fn(Option<&str>, &str) -> Result<(), PasswordError>>;

/// Asks for a new password, twice, and for the current one when that is what it takes.
#[derive(Clone)]
pub(crate) struct PasswordForm {
    words: Words,
    current: Option<State<String>>,
    new: State<String>,
    repeated: State<String>,
    error: State<Option<String>>,
    submit: Submit,
    done: Rc<dyn Fn()>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PasswordForm {
    pub(crate) fn new(words: Words, asks_current: bool, submit: Submit, done: Rc<dyn Fn()>) -> Self {
        Self {
            words,
            current: asks_current.then(|| State::new(String::new())),
            new: State::new(String::new()),
            repeated: State::new(String::new()),
            error: State::new(None),
            submit,
            done,
        }
    }

    fn send(&self) {
        let (new, repeated) = (self.new.peek().clone(), self.repeated.peek().clone());
        if new != repeated {
            self.error.set(Some(self.words.passwords_differ.clone()));
            return;
        }
        let current = self.current.as_ref().map(|state| state.peek().clone());
        match (self.submit)(current.as_deref(), &new) {
            Ok(()) => {
                for state in
                    [Some(&self.new), Some(&self.repeated), self.current.as_ref()].into_iter().flatten()
                {
                    state.set(String::new());
                }
                self.error.set(None);
                (self.done)();
            }
            Err(error) => self.error.set(Some(message_of(&self.words, &error))),
        }
    }
}

impl View for PasswordForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = &self.words;
        let sending = self.clone();
        let current: Option<AnyView> = self.current.as_ref().map(|state| {
            Box::new(FormField::new(
                words.current_password.clone(),
                TextField::new(text_binding(state)).secure().autofocus(),
            )) as AnyView
        });
        let form = self.clone();
        let content = VStack::new((
            self.error.get().map(|error| Alert::new(words.change_password.clone(), error).tone(Tone::Danger)),
            current,
            FormField::new(words.new_password.clone(), TextField::new(text_binding(&self.new)).secure()),
            FormField::new(
                words.repeat_password.clone(),
                TextField::new(text_binding(&self.repeated)).secure().on_submit(move |_| form.send()),
            ),
            Button::new(words.save_password.clone()).on_click(move || sending.send()),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        cx.child(&content)
    }
}

/// Shows the person's own way of changing their password.
pub(crate) fn show_change_password(session: &Session) {
    let words = session.words().clone();
    let (context, shown) = (session.frame().security(), session.clone());
    let submit: Submit = Rc::new(move |current, password| match &context {
        Some(context) => context.change_own_password(current.unwrap_or(""), password),
        None => Err(PasswordError::NotAllowed),
    });
    let told = words.password_saved.clone();
    let done: Rc<dyn Fn()> = Rc::new(move || {
        shown.close_presented();
        shown.show_message(Message::new(told.clone(), MessageKind::Success));
    });
    let form = PasswordForm::new(words.clone(), true, submit, done);
    session.present(words.change_password, move || Box::new(form.clone()) as AnyView);
}

pub(crate) fn message_of(words: &Words, error: &PasswordError) -> String {
    match error {
        PasswordError::TooShort => words.password_too_short.clone(),
        PasswordError::WrongCurrent => words.password_wrong_current.clone(),
        PasswordError::SameAsCurrent => words.password_same.clone(),
        other => other.to_string(),
    }
}

pub(crate) fn text_binding(state: &State<String>) -> Binding<String> {
    let (read, write) = (state.clone(), state.clone());
    Binding::new(move || read.get(), move |value| write.set(value))
}
