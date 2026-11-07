//! A list beside its form, as a piece of a screen an application made itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_app::{Application, EntityInfo, ModelError, Object, ObjectSpace, Value, ViewInfo};
use ravenrs_layout::Align;
use ravenrs_style::TextStyle;
use ravenrs_widgets::grid::Row;
use ravenrs_widgets::{
    BuildCx, Button, ButtonStyle, Card, GridRowAction, HStack, IconName, Keyed, NodeId, Text, VStack, View,
    ViewExt,
};

use crate::list_view::CellText;
use crate::session::{Session, SpaceFactory, Words};
use crate::values::key_from_text;
use crate::{DetailView, ListView, SystemModule};

/// What a pane says about its records.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PaneWords {
    /// Over the form of a new record.
    pub new_title: String,
    /// Over the form of a record being changed.
    pub edit_title: String,
    /// The button that keeps a new record.
    pub add: String,
    /// The button that keeps changes.
    pub save_changes: String,
    /// The button that gives up on changes.
    pub cancel: String,
    /// The button of a row that opens its form, before the record's name.
    pub edit_row: String,
    /// The button of a row that takes it away, before the record's name.
    pub delete_row: String,
    /// The question before taking it away.
    pub delete_question: String,
    /// What taking it away does.
    pub delete_detail: String,
    /// The answer that takes it away.
    pub delete_answer: String,
}

impl PaneWords {
    /// What a pane says about its records, in Brazilian Portuguese where it does not say otherwise.
    pub fn new(
        new_title: impl Into<String>,
        edit_title: impl Into<String>,
        add: impl Into<String>,
        delete_question: impl Into<String>,
        delete_detail: impl Into<String>,
        delete_answer: impl Into<String>,
    ) -> Self {
        let answer: String = delete_answer.into();
        let texts = ravenrs_app_i18n::Texts::new(ravenrs_app_i18n::Language::BrazilianPortuguese);
        Self {
            new_title: new_title.into(),
            edit_title: edit_title.into(),
            add: add.into(),
            save_changes: String::new(),
            cancel: String::new(),
            edit_row: String::new(),
            delete_row: answer.clone(),
            delete_question: delete_question.into(),
            delete_detail: delete_detail.into(),
            delete_answer: answer,
        }
        .with_texts(&texts)
    }

    /// The words that are the same for every pane, from a catalog of texts in its language.
    pub fn with_texts(mut self, texts: &ravenrs_app_i18n::Texts) -> Self {
        self.save_changes = texts.get("pane.save_changes");
        self.cancel = texts.get("pane.cancel");
        self.edit_row = texts.get("pane.edit_row");
        self
    }
}

type DoneHandler = Rc<dyn Fn(&'static str)>;
type FailHandler = Rc<dyn Fn(String)>;

/// What happened, for the application to say in its own way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Saved,
    Deleted,
}

/// A list of an entity's records beside a form that adds or changes them, with a button on each
/// row to open it and one to take it away. It fits inside a screen an application made itself.
pub struct EntityPane {
    session: Session,
    entity: EntityInfo,
    words: PaneWords,
    told: RefCell<Option<DoneHandler>>,
    failed: RefCell<Option<FailHandler>>,
    cell_text: Option<CellText>,
    seen: Cell<u64>,
    beside_from: f32,
    form_width: f32,
    version: Option<Rc<dyn Fn() -> u64>>,
}

impl std::fmt::Debug for EntityPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityPane").field("entity", &self.entity.name).finish_non_exhaustive()
    }
}

impl EntityPane {
    /// A pane over this entity. `space` makes a unit of work for each record opened.
    pub fn new<S: ObjectSpace + 'static>(
        entity: EntityInfo,
        space: impl Fn() -> S + 'static,
        words: Words,
        pane: PaneWords,
    ) -> Result<Self, ModelError> {
        let application = Application::builder(entity.caption.clone())
            .module(SystemModule::with_words(words.clone()))
            .entity(entity.clone())
            .build()?;
        let factory: SpaceFactory =
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>);
        let session = Session::standalone(application, factory, words);
        session.open_new(&entity.name);
        Ok(Self {
            session,
            entity,
            words: pane,
            told: RefCell::default(),
            failed: RefCell::default(),
            cell_text: None,
            seen: Cell::new(0),
            beside_from: 760.0,
            form_width: 340.0,
            version: None,
        })
    }

    /// Runs with a word of what was kept or taken away.
    pub fn on_done(self, handler: impl Fn(&'static str) + 'static) -> Self {
        *self.told.borrow_mut() = Some(Rc::new(handler));
        self
    }

    /// Runs with what went wrong, in words for people.
    pub fn on_failed(self, handler: impl Fn(String) + 'static) -> Self {
        *self.failed.borrow_mut() = Some(Rc::new(handler));
        self
    }

    /// Says what a cell shows, when its value alone is not it.
    pub fn cell_text(mut self, text: impl Fn(&str, &Object) -> Option<String> + 'static) -> Self {
        self.cell_text = Some(Rc::new(text));
        self
    }

    /// The window width from which the form goes beside the list. 760 by default.
    pub fn beside_from(mut self, width: f32) -> Self {
        self.beside_from = width;
        self
    }

    /// Reads this while building, and reads the records again when it changes, for what
    /// the rest of the application does to them.
    pub fn version(mut self, version: impl Fn() -> u64 + 'static) -> Self {
        self.version = Some(Rc::new(version));
        self
    }

    fn view_of(&self, kind: &str) -> Option<ViewInfo> {
        ViewInfo::from_model(&self.session.model(), &format!("{}_{kind}", self.entity.name)).ok()
    }

    fn body(&self, beside: bool) -> impl View + use<> {
        let session = self.session.clone();
        let form = session.form();
        let editing = form.as_ref().is_some_and(|form| !form.is_new);
        let words = self.words.clone();
        let list_view = self.view_of("ListView").map(|view| {
            let (opener, entity) = (session.clone(), self.entity.clone());
            let key_of = {
                let entity = entity.clone();
                move |row: &Row| {
                    let field = entity.key_field()?;
                    let at = row_key_position(&entity, field.name.as_str());
                    row.get(at).map(|cell| key_from_text(&entity, &cell.key_text()))
                }
            };
            let (asker, remover) = (session.clone(), self.clone_handles());
            let open_key = key_of.clone();
            let question = words.clone();
            let open_entity = entity.clone();
            let mut list = ListView::new(session.clone(), session.frame(), view)
                .compact()
                .row_action(GridRowAction::new(words.edit_row.clone(), IconName::Pencil, move |row| {
                    if let Some(key) = open_key(row) {
                        opener.open_detail(&open_entity.name, &key);
                    }
                }))
                .row_action(GridRowAction::new(words.delete_row.clone(), IconName::Trash2, move |row| {
                    let Some(key) = key_of(row) else { return };
                    let (session, remover) = (asker.clone(), remover.clone());
                    let name = entity.name.clone();
                    asker.confirm().ask(
                        question.delete_question.clone(),
                        question.delete_detail.clone(),
                        question.delete_answer.clone(),
                        move || remover.delete(&session, &name, &key),
                    );
                }));
            if let Some(text) = &self.cell_text {
                list = list.cell_text(text.clone());
            }
            list
        });
        let detail = self.view_of("DetailView").map(|view| DetailView::new(session.clone(), view));
        let key = form.as_ref().map_or(0, |form| Rc::as_ptr(form) as usize as u64);
        let title = if editing { words.edit_title.clone() } else { words.new_title.clone() };
        let handles = self.clone_handles();
        let (saver, canceler, entity_name) = (session.clone(), session.clone(), self.entity.name.clone());
        let again = entity_name.clone();
        let buttons = HStack::new((
            editing.then(|| {
                Button::new(words.cancel.clone())
                    .style(ButtonStyle::Secondary)
                    .on_click(move || canceler.open_new(&again))
            }),
            Button::new(if editing { words.save_changes.clone() } else { words.add.clone() })
                .icon(if editing { IconName::Check } else { IconName::Plus })
                .on_click(move || handles.save(&saver, &entity_name)),
        ))
        .spacing(8.0);
        let form_card = Card::new(
            VStack::new((Text::new(title).style(TextStyle::Headline), Keyed::new(key, detail), buttons))
                .spacing(12.0)
                .align(Align::Stretch),
        );
        let list_card = Card::new(VStack::new((list_view,)).align(Align::Stretch));
        let arranged: ravenrs_widgets::AnyView = if beside {
            Box::new(
                HStack::new((list_card.grow(1.0), form_card.width(self.form_width)))
                    .spacing(16.0)
                    .align(Align::Start),
            )
        } else {
            Box::new(VStack::new((list_card, form_card)).spacing(16.0).align(Align::Stretch))
        };
        arranged
    }

    fn clone_handles(&self) -> Handles {
        Handles { told: self.told.borrow().clone(), failed: self.failed.borrow().clone() }
    }
}

/// Where the key is among a list's columns: after those showing, unless it shows too.
fn row_key_position(entity: &EntityInfo, key: &str) -> usize {
    let shown: Vec<&str> = entity.fields.iter().filter(|f| f.in_list).map(|f| f.name.as_str()).collect();
    shown.iter().position(|name| *name == key).unwrap_or(shown.len())
}

/// What a pane tells the application.
#[derive(Clone)]
struct Handles {
    told: Option<DoneHandler>,
    failed: Option<FailHandler>,
}

impl Handles {
    fn say(&self, outcome: Outcome) {
        if let Some(told) = &self.told {
            told(match outcome {
                Outcome::Saved => "saved",
                Outcome::Deleted => "deleted",
            });
        }
    }

    fn fail(&self, session: &Session, message: String) {
        session.set_error(None);
        if let Some(failed) = &self.failed {
            failed(message);
        }
    }

    fn save(&self, session: &Session, entity: &str) {
        let (saved, name) = (self.clone(), entity.to_owned());
        let done = session.save_then(move |session| {
            session.open_new(&name);
            saved.say(Outcome::Saved);
        });
        match done {
            Ok(()) => {}
            // What fails a rule is on the form already.
            Err(_) if session.is_checking() => {}
            Err(message) => self.fail(session, message),
        }
    }

    fn delete(&self, session: &Session, entity: &str, key: &Value) {
        let found = session.frame().space().borrow().find(entity, key);
        let Ok(Some(object)) = found else { return };
        match session.delete(&[object]) {
            Ok(()) => {
                session.open_new(entity);
                self.say(Outcome::Deleted);
            }
            Err(message) => self.fail(session, message),
        }
    }
}

impl View for EntityPane {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if let Some(version) = &self.version {
            let now = version();
            if self.seen.replace(now) != now {
                self.session.records_changed();
            }
        }
        let from = self.beside_from;
        let beside = cx.viewport().width >= from;
        let body = self.body(beside);
        let parts = cx.child(&body);
        let confirm = cx.child(self.session.confirm());
        cx.container(
            ravenrs_layout::LayoutStyle {
                align: Align::Stretch,
                ..ravenrs_layout::LayoutStyle::stack(ravenrs_layout::Axis::Vertical)
            },
            &[parts, confirm],
            ravenrs_widgets::Paint::None,
        )
    }
}
