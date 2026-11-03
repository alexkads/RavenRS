//! The way into an application whose records are on a server.

use std::rc::Rc;

use ravenrs_app::{EntityInfo, Message, MessageKind, ModelError};
use ravenrs_app_ui::{App, Language, Session, Words};
use ravenrs_layout::{Align, Justify};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, Card, FormField, HStack, Localized, NodeId, Spinner, Text, TextField,
    Tone, VStack, View, ViewExt,
};

use crate::{RemoteClient, RemoteDatabase, RemoteSettings, SignInError};

const FORM_WIDTH: f32 = 400.0;

#[derive(Clone)]
enum Stage {
    SignIn,
    NewPassword,
    Loading,
    Ready(Rc<AnyView>),
    Broken(String),
}

struct Inner {
    title: String,
    words: Words,
    client: RemoteClient,
    entities: Vec<EntityInfo>,
    make: Box<dyn Fn(&RemoteDatabase) -> App>,
    stage: State<Stage>,
    name: State<String>,
    password: State<String>,
    new_password: State<String>,
    error: State<Option<String>>,
}

/// An application whose records are on a server: it asks who is there, reads what they can see, and
/// opens what `make` puts together over those records.
#[derive(Clone)]
pub struct RemoteApp(Rc<Inner>);

impl std::fmt::Debug for RemoteApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteApp").field("title", &self.0.title).finish_non_exhaustive()
    }
}

impl RemoteApp {
    /// `make` gets the records and gives the application, with those records as its storage.
    pub fn view(
        title: impl Into<String>,
        language: Language,
        client: RemoteClient,
        entities: Vec<EntityInfo>,
        make: impl Fn(&RemoteDatabase) -> App + 'static,
    ) -> Result<impl View, ModelError> {
        let mine = Self(Rc::new(Inner {
            title: title.into(),
            words: language.words(),
            client,
            entities,
            make: Box::new(make),
            stage: State::new(Stage::SignIn),
            name: State::new(String::new()),
            password: State::new(String::new()),
            new_password: State::new(String::new()),
            error: State::new(None),
        }));
        Ok(Localized::new(language.labels(), mine))
    }

    fn sign_in(&self) {
        let inner = &self.0;
        let (name, password) = (inner.name.peek().clone(), inner.password.peek().clone());
        let mine = self.clone();
        inner.client.sign_in(&name, &password, move |result| match result {
            Ok(signed) if signed.must_change_password => {
                mine.0.error.set(None);
                mine.0.password.set(String::new());
                mine.0.stage.set(Stage::NewPassword);
            }
            Ok(_) => mine.open(),
            Err(SignInError::Rejected) => mine.0.error.set(Some(mine.0.words.sign_in_failed.clone())),
            Err(SignInError::Other(why)) => mine.0.error.set(Some(why)),
        });
    }

    fn change_password(&self) {
        let inner = &self.0;
        let (current, new) = (inner.password.peek().clone(), inner.new_password.peek().clone());
        let mine = self.clone();
        inner.client.change_password(&current, &new, move |result| match result {
            Ok(()) => mine.open(),
            Err(why) => mine.0.error.set(Some(why)),
        });
    }

    fn open(&self) {
        let inner = &self.0;
        inner.error.set(None);
        // What was typed has no reason to stay around.
        inner.password.set(String::new());
        inner.new_password.set(String::new());
        inner.stage.set(Stage::Loading);
        let database = match RemoteDatabase::new(inner.client.clone(), inner.entities.clone()) {
            Ok(database) => database,
            Err(error) => {
                inner.stage.set(Stage::Broken(error.to_string()));
                return;
            }
        };
        database.denied_message(inner.words.access_denied.clone());
        let (mine, loaded) = (self.clone(), database.clone());
        database.load(move |result| {
            if let Err(why) = result {
                mine.0.stage.set(Stage::Broken(why));
                return;
            }
            let settings = RemoteSettings::new(mine.0.client.clone());
            let (keeping, reading) = (settings.clone(), mine.clone());
            settings.load(vec!["theme".to_owned()], Box::new(move || reading.enter(&loaded, keeping)));
        });
    }

    fn enter(&self, loaded: &RemoteDatabase, settings: RemoteSettings) {
        let (client, back) = (self.0.client.clone(), self.clone());
        let watching = loaded.clone();
        let app = (self.0.make)(loaded).settings(settings).signed_in_remotely(
            self.0.name.peek().clone(),
            move || {
                client.sign_out();
                back.0.stage.set(Stage::SignIn);
            },
        );
        let app = app.with_session(move |session: &Session| {
            let changed = session.clone();
            watching.on_changed(move || changed.records_changed());
            let told = session.clone();
            watching.on_problem(move |why| told.show_message(Message::new(why, MessageKind::Error)));
        });
        match app.build() {
            Ok(view) => self.0.stage.set(Stage::Ready(Rc::new(view))),
            Err(error) => self.0.stage.set(Stage::Broken(error.to_string())),
        }
    }

    fn form(&self, content: impl View + 'static) -> impl View {
        centered(Card::new(content).padding(24.0).width(FORM_WIDTH))
    }
}

fn centered(content: impl View + 'static) -> impl View {
    HStack::new((content,)).justify(Justify::Center).align(Align::Center).grow(1.0)
}

fn binding(state: &State<String>) -> Binding<String> {
    let (read, write) = (state.clone(), state.clone());
    Binding::new(move || read.get(), move |value| write.set(value))
}

impl View for RemoteApp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = &self.0;
        let words = &inner.words;
        let title = || Text::new(inner.title.clone()).style(TextStyle::Title);
        let error = inner.error.get().map(|why| Alert::message(why).tone(Tone::Danger));
        match inner.stage.get() {
            Stage::Ready(view) => view.build(cx),
            Stage::Loading => cx.child(&centered(Spinner::new())),
            Stage::Broken(why) => cx.child(
                &self.form(
                    VStack::new((title(), Alert::message(why).tone(Tone::Danger)))
                        .spacing(12.0)
                        .align(Align::Stretch),
                ),
            ),
            Stage::SignIn => {
                let (by_key, by_button) = (self.clone(), self.clone());
                cx.child(
                    &self.form(
                        VStack::new((
                            title(),
                            error,
                            FormField::new(
                                words.user_name.clone(),
                                TextField::new(binding(&inner.name)).autofocus(),
                            ),
                            FormField::new(
                                words.password.clone(),
                                TextField::new(binding(&inner.password))
                                    .secure()
                                    .on_submit(move |_| by_key.sign_in()),
                            ),
                            Button::new(words.sign_in.clone())
                                .full_width()
                                .on_click(move || by_button.sign_in()),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                )
            }
            Stage::NewPassword => {
                let by_button = self.clone();
                cx.child(
                    &self.form(
                        VStack::new((
                            title(),
                            Text::new(words.change_password.clone()).foreground(Semantic::InkMuted),
                            error,
                            FormField::new(
                                words.current_password.clone(),
                                TextField::new(binding(&inner.password)).secure(),
                            ),
                            FormField::new(
                                words.new_password.clone(),
                                TextField::new(binding(&inner.new_password)).secure(),
                            ),
                            Button::new(words.change_password.clone())
                                .full_width()
                                .on_click(move || by_button.change_password()),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                )
            }
        }
    }
}
