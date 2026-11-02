//! The window of an application: navigation, commands and the view.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    ActionInput, Application, Frame, FrameContext, ModelNode, ObjectSpace, Value, ViewInfo, ViewKind,
};
use ravenrs_core::Insets;
use ravenrs_criteria::DateTime;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, Breadcrumb, BuildCx, Button, ButtonStyle, Date, Dialog, Divider, Keyed, NodeId, Paint,
    ScrollView, Sidebar, SidebarCollapse, SidebarItem, SidebarSection, Text, Tone, VStack, View, ViewExt,
};

use crate::container::{ActionContainer, icon_named};
use crate::detail_view::DetailView;
use crate::editors::EditorRegistry;
use crate::list_view::ListView;
use crate::session::{Session, Words};
use crate::values::display;

const NARROW_WINDOW: f32 = 720.0;

/// The pieces of a window, for a template to arrange.
#[non_exhaustive]
pub struct TemplateParts {
    /// The navigation.
    pub navigation: AnyView,
    /// Where the view is, and the commands.
    pub header: AnyView,
    /// The view, or what went wrong above it.
    pub content: AnyView,
}

impl std::fmt::Debug for TemplateParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateParts").finish_non_exhaustive()
    }
}

/// Arranges a window. Replacing it changes the whole look of an application, and nothing else.
pub trait FrameTemplate {
    /// Puts the pieces where they go.
    fn arrange(&self, parts: TemplateParts) -> AnyView;
}

/// Navigation on the side, commands above the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct MainTemplate;

impl FrameTemplate for MainTemplate {
    fn arrange(&self, parts: TemplateParts) -> AnyView {
        let main =
            VStack::new((parts.header, Divider::horizontal(), parts.content)).align(Align::Stretch).grow(1.0);
        Box::new(ravenrs_widgets::HStack::new((parts.navigation, main)).align(Align::Stretch).grow(1.0))
    }
}

fn hash_of(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

/// A whole application window, generated from its model.
pub struct AppShell {
    session: Session,
    template: Rc<dyn FrameTemplate>,
}

impl std::fmt::Debug for AppShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppShell").field("session", &self.session).finish_non_exhaustive()
    }
}

impl AppShell {
    /// A window over this application. `space` makes the unit of work of each view.
    pub fn new<S: ObjectSpace + 'static>(application: Application, space: impl Fn() -> S + 'static) -> Self {
        Self::with_options(application, space, Words::default(), EditorRegistry::default())
    }

    /// Like [`new`](Self::new), with the words of its screens and controls of its own.
    pub fn with_options<S: ObjectSpace + 'static>(
        application: Application,
        space: impl Fn() -> S + 'static,
        words: Words,
        editors: EditorRegistry,
    ) -> Self {
        let factory: Rc<dyn Fn() -> Rc<RefCell<dyn ObjectSpace>>> =
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>);
        let today = Date::from_days(DateTime::now().days);
        let session = Session::new(application, factory, words, editors, today);
        if let Some(first) = session.first_view() {
            session.show_view_now(&first);
        }
        Self { session, template: Rc::new(MainTemplate) }
    }

    /// Arranges the window with this.
    pub fn template(mut self, template: impl FrameTemplate + 'static) -> Self {
        self.template = Rc::new(template);
        self
    }

    /// Hands exported files over to this, with their name and bytes: saving them is up to
    /// the platform.
    pub fn on_file(self, handler: impl Fn(String, Vec<u8>) + 'static) -> Self {
        self.session.set_on_file(Rc::new(handler));
        self
    }

    /// Hands addresses to open over to this.
    pub fn on_url(self, handler: impl Fn(String) + 'static) -> Self {
        self.session.set_on_url(Rc::new(handler));
        self
    }

    /// What its screens share.
    pub fn session(&self) -> Session {
        self.session.clone()
    }

    fn navigation(&self, current: Option<&ViewInfo>) -> AnyView {
        let model = self.session.model();
        let mut content: Vec<AnyView> = Vec::new();
        if let Some(root) = model.get("NavigationItems") {
            for group in root.in_order() {
                if let Some(view) = group.text("View") {
                    content.push(self.navigation_item(group, view, current));
                    continue;
                }
                content.push(Box::new(SidebarSection::new(
                    group.text("Caption").unwrap_or(&group.id).to_owned(),
                )));
                for item in group.in_order() {
                    if let Some(view) = item.text("View") {
                        content.push(self.navigation_item(item, view, current));
                    }
                }
            }
        }
        Box::new(
            Sidebar::new(content)
                .collapsible(SidebarCollapse::Icons)
                .narrow_below(NARROW_WINDOW)
                .header(Text::new(self.session.title()).font_weight(Weight::SEMIBOLD)),
        )
    }

    fn navigation_item(&self, item: &ModelNode, view: &str, current: Option<&ViewInfo>) -> AnyView {
        let model = self.session.model();
        let entity_of = |id: &str| model.at(&["Views", id]).and_then(|n| n.text("Entity"));
        let selected = current.is_some_and(|c| c.entity.as_deref() == entity_of(view));
        let icon = item.text("Icon").and_then(icon_named).unwrap_or(ravenrs_widgets::IconName::File);
        let (session, target) = (self.session.clone(), view.to_owned());
        Box::new(
            SidebarItem::new(icon, item.text("Caption").unwrap_or(&item.id).to_owned())
                .selected(selected)
                .on_click(move || session.open_view(&target)),
        )
    }

    fn trail(&self, view: Option<&ViewInfo>) -> Vec<String> {
        let Some(view) = view else { return vec![self.session.title()] };
        let Some(entity) = view.entity.as_deref().and_then(|name| self.session.entity(name)) else {
            return vec![view.id.clone()];
        };
        let mut trail = vec![entity.caption.clone()];
        if view.kind == ViewKind::Detail
            && let Some(form) = self.session.form()
        {
            trail.push(if form.is_new {
                self.session.words().new_record.clone()
            } else {
                entity
                    .default_property()
                    .map(|field| display(field, &form.original.get(&field.name)))
                    .unwrap_or_default()
            });
        }
        trail
    }

    fn header(&self, view: Option<&ViewInfo>) -> AnyView {
        Box::new(
            VStack::new((Breadcrumb::new(self.trail(view)), ActionContainer::new(self.session.clone(), [])))
                .spacing(8.0)
                .align(Align::Stretch)
                .padding(Insets::symmetric(10.0, 16.0)),
        )
    }

    fn content(&self, view: Option<&ViewInfo>) -> AnyView {
        let session = &self.session;
        let body: AnyView = match view {
            Some(view) if view.kind == ViewKind::List => Box::new(Keyed::new(
                hash_of(&view.id),
                ListView::new(session.clone(), session.frame(), view.clone())
                    .padding(12.0)
                    .stretch()
                    .grow(1.0),
            )),
            Some(view) if view.kind == ViewKind::Detail => {
                let form = session.form().map_or(0, |form| Rc::as_ptr(&form) as usize as u64);
                Box::new(ScrollView::new((Keyed::new(
                    form,
                    DetailView::new(session.clone(), view.clone()).padding(16.0),
                ),)))
            }
            Some(view) => Box::new(Text::new(view.id.clone())),
            None => Box::new(Text::new(session.title()).style(TextStyle::Body)),
        };
        Box::new(
            VStack::new((
                session.error().map(|error| {
                    Alert::new(session.title(), error)
                        .tone(Tone::Danger)
                        .padding(Insets::symmetric(8.0, 16.0))
                }),
                body,
            ))
            .align(Align::Stretch)
            .grow(1.0),
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
        let (closing, canceling) = (main.clone(), main.clone());
        let open = Binding::new(
            move || closing.open_popup().is_some(),
            move |open: bool| {
                if !open {
                    canceling.cancel_popup();
                }
            },
        );
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

impl View for AppShell {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let view = session.frame().view();
        let view = view.as_ref();
        let window = self.template.arrange(TemplateParts {
            navigation: self.navigation(view),
            header: self.header(view),
            content: self.content(view),
        });
        let mut parts = vec![cx.child(&window)];
        parts.push(cx.child(session.confirm()));
        // The window of an action that asks for a record, made once for each time it opens.
        let popup_frame: State<Option<(String, Frame)>> = cx.use_state(|| None);
        match session.frame().open_popup() {
            Some((action, target)) => {
                let model = session.model();
                if let Ok(info) = ViewInfo::from_model(&model, &target) {
                    let kept = popup_frame.peek().clone().filter(|(id, _)| *id == action);
                    let frame = kept.map_or_else(
                        || {
                            let frame = session.new_frame(FrameContext::Popup);
                            frame.set_view(Some(info.clone()));
                            popup_frame.set(Some((action.clone(), frame.clone())));
                            frame
                        },
                        |(_, frame)| frame,
                    );
                    let caption = session
                        .frame()
                        .action(&action)
                        .map_or_else(|| action.clone(), |a| a.caption().to_owned());
                    parts.push(cx.child(&PopupList {
                        session: session.clone(),
                        action: caption,
                        view: info,
                        frame,
                    }));
                }
            }
            None => {
                if popup_frame.peek().is_some() {
                    popup_frame.set(None);
                }
            }
        }
        cx.container(
            LayoutStyle {
                grow: 1.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::None,
        )
    }
}
