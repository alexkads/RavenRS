//! The records of another entity that point to the one on the form.

use std::rc::Rc;

use ravenrs_app::{Criteria, EntityInfo, FrameContext, Value, ViewInfo};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_widgets::{BuildCx, DataGrid, NodeId, Text, VStack, View, ViewExt};

use crate::container::ActionContainer;
use crate::detail_view::DetailView;
use crate::list_view::ListView;
use crate::session::Session;
use crate::session::back_reference;
use crate::session::list_types::ListSource;
use ravenrs_app::{ActionInput, Frame, ViewKind};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_widgets::{Alert, Button, ButtonStyle, Dialog, Keyed, Paint, Tone};

/// How wide a window of something shown over the screens is.
pub(crate) const PRESENTED_WIDTH: f32 = 960.0;

/// How tall a list inside a form is.
const NESTED_HEIGHT: f32 = 240.0;

/// Which record a list inside a form belongs to, for the actions that link and unlink.
pub(crate) struct NestedOwner {
    /// The entity of the records listed.
    pub target: String,
    /// Their field that points to the owner.
    pub field: String,
    /// The owner's key.
    pub key: Value,
    /// What the list to pick from has to meet.
    pub link_filter: Option<Criteria>,
}

/// A list inside a form, in a frame of its own. What links and unlinks is kept right away.
pub(crate) struct NestedList {
    parent: Session,
    owner: Option<Value>,
    target: String,
    field: Option<String>,
    link_filter: Option<String>,
    source: Option<ListSource>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NestedList {
    /// A list of records no storage keeps, given by this. There is nothing to link or unlink.
    pub fn given(parent: Session, target: &str, source: ListSource) -> Self {
        Self {
            parent,
            owner: None,
            target: target.to_owned(),
            field: None,
            link_filter: None,
            source: Some(source),
        }
    }

    pub fn new(
        parent: Session,
        owner: &EntityInfo,
        key: Option<Value>,
        target: &str,
        named: Option<&str>,
        link_filter: Option<&str>,
    ) -> Self {
        let field = back_reference(&parent, owner, target, named);
        Self {
            parent,
            owner: key,
            target: target.to_owned(),
            field,
            link_filter: link_filter.map(str::to_owned),
            source: None,
        }
    }
}

impl View for NestedList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = self.parent.words();
        let id = format!("{}_ListView", self.target);
        let Ok(view) = ViewInfo::from_model(&self.parent.model(), &id) else {
            return cx.child(&Text::new(id));
        };
        let held: State<Option<Session>> = cx.use_state(|| None);
        if let Some(source) = &self.source {
            let kept = held.peek().clone();
            let child = kept.unwrap_or_else(|| {
                let child = self.parent.child(FrameContext::Nested);
                child.frame().set_view(Some(view.clone()));
                held.set(Some(child.clone()));
                child
            });
            let list = ListView::new(child.clone(), child.frame(), view)
                .source(source.clone())
                .plain_toolbar()
                .grid_shape(|grid: DataGrid| grid.toolbar(false).filter_row(false).footer(false));
            return cx.child(&list.height(NESTED_HEIGHT));
        }
        let (Some(key), Some(field)) = (self.owner.clone(), self.field.clone()) else {
            return cx.child(&Text::new(words.save_first.clone()));
        };
        let kept = held.peek().clone();
        let child = kept.unwrap_or_else(|| {
            let child = self.parent.child(FrameContext::Nested);
            child.frame().set_view(Some(view.clone()));
            held.set(Some(child.clone()));
            child
        });
        child.frame().set_extension(Rc::new(NestedOwner {
            target: self.target.clone(),
            field: field.clone(),
            key: key.clone(),
            link_filter: self
                .link_filter
                .as_deref()
                .and_then(|text| crate::editors::filter_for(text, self.parent.form_record().as_ref())),
        }));
        let opener = self.parent.clone();
        let target = self.target.clone();
        let list = ListView::new(child.clone(), child.frame(), view)
            .filter(Some(Criteria::property(&field).equals(Criteria::value(key))))
            .implied(field.clone())
            .plain_toolbar()
            .on_open(Rc::new(move |key: &Value| opener.open_detail(&target, key)))
            .grid_shape(|grid: DataGrid| grid.toolbar(false).filter_row(false).footer(false));
        cx.child(
            &VStack::new((
                ActionContainer::new(child.clone(), []),
                list.height(NESTED_HEIGHT),
                child.confirmations(),
                PopupHost::new(child),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
    }
}

/// A list in a window of its own, for an action that asks for a record.
struct PopupList {
    session: Session,
    action: String,
    view: ViewInfo,
    frame: Frame,
}

impl View for PopupList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let main = self.session.frame();
        let open = popup_open(&main);
        let key_of = {
            let session = self.session.clone();
            move |frame: &Frame| {
                frame.selected().first().and_then(|object| {
                    let entity = session.entity(object.entity())?;
                    entity.key_field().map(|field| object.get(&field.name))
                })
            }
        };
        let accept: Rc<dyn Fn(Option<Value>)> = {
            let (session, main) = (self.session.clone(), main.clone());
            Rc::new(move |key| {
                let input = key.map_or(ActionInput::None, ActionInput::Parameter);
                if let Err(error) = main.accept_popup(input) {
                    session.set_error(Some(error.to_string()));
                }
            })
        };
        let (ok, frame) = (accept.clone(), self.frame.clone());
        let on_open = accept;
        let list = ListView::new(self.session.clone(), self.frame.clone(), self.view.clone())
            .filter(self.session.popup_filter())
            .on_open(Rc::new(move |key: &Value| on_open(Some(key.clone()))));
        let words = self.session.words();
        let cancel_frame = main;
        cx.child(
            &Dialog::new(open, self.action.clone())
                .content(list.height(420.0))
                .action(
                    Button::new(words.cancel.clone())
                        .style(ButtonStyle::Secondary)
                        .on_click(move || cancel_frame.cancel_popup()),
                )
                .action(Button::new("OK").on_click(move || ok(key_of(&frame))))
                .width(720.0),
        )
    }
}

/// A form in a window of its own, for an action that asks for a record to be filled in. It runs
/// with the record, once that meets its rules.
struct PopupForm {
    /// Where the action is.
    owner: Session,
    /// Where the form is.
    form: Session,
    action: String,
    view: ViewInfo,
}

impl View for PopupForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let main = self.owner.frame();
        let open = popup_open(&main);
        let (form, owner_frame) = (self.form.clone(), main.clone());
        let words = self.owner.words();
        let cancel_frame = main;
        let key = self.form.form().map_or(0, |form| Rc::as_ptr(&form) as usize as u64);
        let content = VStack::new((
            self.form.error().map(|error| Alert::new(self.action.clone(), error).tone(Tone::Danger)),
            Keyed::new(key, DetailView::new(self.form.clone(), self.view.clone())),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        cx.child(
            &Dialog::new(open, self.action.clone())
                .width(480.0)
                .content(content)
                .action(
                    Button::new(words.cancel.clone())
                        .style(ButtonStyle::Secondary)
                        .on_click(move || cancel_frame.cancel_popup()),
                )
                .action(Button::new("OK").on_click(move || {
                    let Some((record, _)) = form.check_form() else { return };
                    if let Err(error) = owner_frame.accept_popup(ActionInput::Record(record)) {
                        form.set_error(Some(error.to_string()));
                    }
                })),
        )
    }
}

/// What a popup holds while it is open.
#[derive(Clone)]
enum PopupParts {
    List(Frame),
    Form(Session),
}

/// The windows of a frame's actions: what they ask for, and what they show.
pub(crate) struct PopupHost {
    session: Session,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PopupHost {
    pub(crate) fn new(session: Session) -> Self {
        Self { session }
    }

    fn parts_for(&self, info: &ViewInfo) -> PopupParts {
        if info.kind != ViewKind::Detail {
            let frame = self.session.new_frame(FrameContext::Popup);
            frame.set_view(Some(info.clone()));
            return PopupParts::List(frame);
        }
        let form = self.session.child(FrameContext::Popup);
        if let Some(entity) = info.entity.as_deref().and_then(|name| self.session.entity(name)) {
            let mut record = entity.new_object();
            self.session.frame().prepare_popup(&mut record);
            form.open_object(&entity, record, true);
        }
        PopupParts::Form(form)
    }
}

impl View for PopupHost {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        crate::jobs::keep(cx, session);
        let mut parts = Vec::new();
        // Made once for each time it opens.
        let held: State<Option<(String, PopupParts)>> = cx.use_state(|| None);
        match session.frame().open_popup() {
            Some((action, target)) => {
                if let Ok(info) = ViewInfo::from_model(&session.model(), &target) {
                    let kept = held.peek().clone().filter(|(id, _)| *id == action);
                    let made = kept.map_or_else(
                        || {
                            let made = self.parts_for(&info);
                            held.set(Some((action.clone(), made.clone())));
                            made
                        },
                        |(_, made)| made,
                    );
                    let caption = session
                        .frame()
                        .action(&action)
                        .map_or_else(|| action.clone(), |a| a.caption().to_owned());
                    parts.push(match made {
                        PopupParts::List(frame) => cx.child(&PopupList {
                            session: session.clone(),
                            action: caption,
                            view: info,
                            frame,
                        }),
                        PopupParts::Form(form) => {
                            cx.child(&PopupForm { owner: session.clone(), form, action: caption, view: info })
                        }
                    });
                }
            }
            None => {
                if held.peek().is_some() {
                    held.set(None);
                }
            }
        }
        parts.push(cx.child(&crate::reminders::Reminders::new(session.clone())));
        if let Some((title, content)) = session.presented().get() {
            let (read, write) = (session.presented().clone(), session.presented().clone());
            let open = Binding::new(
                move || read.get().is_some(),
                move |open: bool| {
                    if !open {
                        write.set(None);
                    }
                },
            );
            parts.push(cx.child(&Dialog::new(open, title).width(PRESENTED_WIDTH).content(content())));
        }
        cx.container(LayoutStyle::stack(Axis::Vertical), &parts, Paint::None)
    }
}

/// What closes the popup of a frame, when the window closes.
fn popup_open(main: &Frame) -> Binding<bool> {
    let (closing, canceling) = (main.clone(), main.clone());
    Binding::new(
        move || closing.open_popup().is_some(),
        move |open: bool| {
            if !open {
                canceling.cancel_popup();
            }
        },
    )
}
