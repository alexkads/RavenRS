//! The window of an application: navigation, commands and the view.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    ActionInput, Application, Frame, FrameContext, ModelNode, ObjectSpace, SettingScope, Value, ViewInfo,
    ViewKind,
};
use ravenrs_core::Insets;
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_widgets::{
    Alert, AnyView, Breadcrumb, BuildCx, Button, ButtonSize, ButtonStyle, Date, Dialog, Divider, IconButton,
    IconName, KeyBar, KeyBarItem, Keyed, NodeId, Paint, ScrollView, Sidebar, SidebarCollapse, SidebarItem,
    SidebarSection, SidebarState, SidebarTrigger, Text, TextField, ThemePicker, Tone, VStack, View, ViewExt,
};

use crate::container::{ActionContainer, icon_named};
use crate::detail_view::DetailView;
use crate::editors::EditorRegistry;
use crate::list_view::ListView;
use crate::session::{Session, Words};
use crate::values::display;

const NARROW_WINDOW: f32 = 720.0;
/// The name the theme someone chose goes under.
const THEME_SETTING: &str = "theme";
/// How wide a window of something shown over the screens is.
const PRESENTED_WIDTH: f32 = 960.0;
/// How far everything on a page stands from its edges, so it all starts on one line.
const PAGE_GUTTER: f32 = 16.0;
/// The longest line of a description that is still comfortable to read.
const READING_WIDTH: f32 = 760.0;

/// The pieces of a window, for a template to arrange.
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TemplateParts {
    /// The navigation.
    pub navigation: AnyView,
    /// Where the view is, and the commands.
    pub header: AnyView,
    /// The view, or what went wrong above it.
    pub content: AnyView,
    /// A bar of the keys of what can be done here. Only a template made to be used from the
    /// keyboard shows it.
    pub keys: AnyView,
    /// The kind of view in the window, if any.
    pub kind: Option<ViewKind>,
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MainTemplate;

impl FrameTemplate for MainTemplate {
    fn arrange(&self, parts: TemplateParts) -> AnyView {
        let main = VStack::new((parts.header, parts.content)).spacing(0.0).align(Align::Stretch).grow(1.0);
        Box::new(
            ravenrs_widgets::HStack::new((parts.navigation, main))
                .spacing(0.0)
                .align(Align::Stretch)
                .grow(1.0),
        )
    }
}

/// A window for use at a counter: the view takes it all, with the keys of what can be done always in
/// sight below it. Going elsewhere is up to the actions.
#[derive(Clone, Copy, Debug, Default)]
pub struct CounterTemplate;

impl FrameTemplate for CounterTemplate {
    fn arrange(&self, parts: TemplateParts) -> AnyView {
        // A dashboard is a screen of its own; a list or a form still needs its commands.
        let mut rows: Vec<AnyView> = Vec::with_capacity(3);
        if parts.kind != Some(ViewKind::Dashboard) {
            rows.push(parts.header);
        }
        rows.push(Box::new(parts.content.grow(1.0)));
        rows.push(parts.keys);
        Box::new(VStack::new(rows).spacing(0.0).align(Align::Stretch).grow(1.0))
    }
}

/// The keys of what can be done here, from the actions that have one.
struct ActionKeys {
    session: Session,
}

impl View for ActionKeys {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let actions = session.frame().actions();
        let mut bar = KeyBar::new();
        for action in actions.iter().filter(|action| action.is_active()) {
            let Some(hotkey) = action.hotkey_text().and_then(crate::container::hotkey_of) else { continue };
            let (run, session) = (action.clone(), session.clone());
            let mut item = KeyBarItem::new(hotkey, action.caption().to_owned(), move || {
                session.run_action(&run, ActionInput::None);
            })
            .disabled(!action.is_enabled());
            if *action.action_category() == ravenrs_app::ActionCategory::Save {
                item = item.primary();
            }
            bar = bar.item(item);
        }
        cx.child(&bar.padding(8.0))
    }
}

/// What a locked window shows.
struct LockedWindow {
    session: Session,
    title: String,
    check: Rc<dyn Fn(&str) -> bool>,
}

impl View for LockedWindow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let typed: State<String> = cx.use_state(String::new);
        let wrong: State<bool> = cx.use_state(|| false);
        let words = self.session.words().clone();
        let submit = {
            let (session, check, wrong, typed) =
                (self.session.clone(), self.check.clone(), wrong.clone(), typed.clone());
            move |text: String| {
                if check(text.trim()) {
                    wrong.set(false);
                    session.locked().set(None);
                } else {
                    wrong.set(true);
                }
                typed.set(String::new());
            }
        };
        let (read, write) = (typed.clone(), typed.clone());
        let field = TextField::new(Binding::new(move || read.get(), move |value: String| write.set(value)))
            .secure()
            .autofocus()
            .width(280.0)
            .placeholder(words.unlock.clone())
            .on_submit(submit);
        let mut lines: Vec<AnyView> =
            vec![Box::new(Text::new(self.title.clone()).style(TextStyle::Title)), Box::new(field)];
        if wrong.get() {
            lines.push(Box::new(Text::new(words.unlock_wrong.clone()).foreground(Semantic::Danger)));
        }
        cx.child(
            &VStack::new(vec![Box::new(VStack::new(lines).spacing(12.0).align(Align::Center)) as AnyView])
                .align(Align::Center)
                .justify(ravenrs_layout::Justify::Center)
                .grow(1.0),
        )
    }
}

fn hash_of(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

/// A whole application window, generated from its model.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AppShell {
    session: Session,
    template: Rc<dyn FrameTemplate>,
    theme_menu: bool,
    theme: Option<ravenrs_style::Theme>,
    theme_applied: std::cell::Cell<bool>,
    kept_theme: RefCell<Option<String>>,
    sidebar: SidebarState,
}

impl std::fmt::Debug for AppShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppShell").field("session", &self.session).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
        Self::from_factory(application, factory, words, editors)
    }

    pub(crate) fn from_factory(
        application: Application,
        factory: crate::session::SpaceFactory,
        words: Words,
        editors: EditorRegistry,
    ) -> Self {
        let today = Date::from_days(ravenrs_app::local_now().days);
        let session = Session::new(application, factory, words, editors, today);
        if let Some(first) = session.first_view() {
            session.show_view_now(&first);
        }
        Self {
            session,
            template: Rc::new(MainTemplate),
            theme_menu: true,
            theme: None,
            theme_applied: std::cell::Cell::new(false),
            kept_theme: RefCell::new(None),
            sidebar: SidebarState::new(true),
        }
    }

    /// Starts on this theme, the look of an application with a look of its own.
    pub fn theme(mut self, theme: ravenrs_style::Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Arranges the window with this.
    pub fn template(mut self, template: impl FrameTemplate + 'static) -> Self {
        self.template = Rc::new(template);
        self
    }

    /// Keeps people on the theme the application started with, for one that has a look of its own.
    pub fn without_theme_menu(mut self) -> Self {
        self.theme_menu = false;
        self
    }

    /// Hands exported files over to this, with their name and bytes, instead of asking the
    /// person where to save them.
    pub fn on_file(self, handler: impl Fn(String, Vec<u8>) + 'static) -> Self {
        self.session.set_on_file(Rc::new(handler));
        self
    }

    /// Hands addresses to open over to this.
    pub fn on_url(self, handler: impl Fn(String) + 'static) -> Self {
        self.session.set_on_url(Rc::new(handler));
        self
    }

    /// Whether the navigation is open, shared with the button that opens it in a narrow window.
    /// An application that saves or drives it passes a [`SidebarState::controlled`] one.
    pub fn sidebar(mut self, state: SidebarState) -> Self {
        self.sidebar = state;
        self
    }

    /// Whether the navigation is open.
    pub fn sidebar_state(&self) -> SidebarState {
        self.sidebar.clone()
    }

    /// What its screens share.
    pub fn session(&self) -> Session {
        self.session.clone()
    }

    /// Brings back the theme this person left, the first time, and keeps it whenever it changes.
    fn keep_theme(&self, cx: &mut BuildCx<'_>) {
        let Some(store) = self.session.settings() else { return };
        let scope = match self.session.account() {
            Some(account) => SettingScope::User(account.name),
            None => SettingScope::Global,
        };
        let state = cx.theme_state();
        let mut kept = self.kept_theme.borrow_mut();
        if kept.is_none() {
            let stored = store.get(&scope, THEME_SETTING).ok().flatten();
            // What is kept comes from outside the program, and what makes no sense is left out.
            if let Some(theme) = stored
                .as_deref()
                .and_then(|text| Theme::from_tokens_json(text).ok())
                .filter(Theme::is_readable)
            {
                state.set(theme);
                *kept = stored;
                return;
            }
            *kept = Some(state.with(Theme::to_tokens_json));
            return;
        }
        let now = state.with(Theme::to_tokens_json);
        if kept.as_deref() != Some(now.as_str()) {
            if let Err(why) = store.set(&scope, THEME_SETTING, &now) {
                self.session.show_message(ravenrs_app::Message::new(why, ravenrs_app::MessageKind::Error));
            }
            *kept = Some(now);
        }
    }

    fn navigation(&self, current: Option<&ViewInfo>) -> AnyView {
        let model = self.session.model();
        let mut content: Vec<AnyView> = Vec::new();
        if let Some(root) = model.get("NavigationItems") {
            for group in root.in_order() {
                if let Some(view) = group.text("View") {
                    if self.session.can_open(None, &group.id, view) {
                        content.push(self.navigation_item(group, view, current));
                    }
                    continue;
                }
                let items: Vec<AnyView> = group
                    .in_order()
                    .into_iter()
                    .filter_map(|item| {
                        let view = item.text("View")?;
                        self.session
                            .can_open(Some(&group.id), &item.id, view)
                            .then(|| self.navigation_item(item, view, current))
                    })
                    .collect();
                if !items.is_empty() {
                    content.push(Box::new(SidebarSection::new(
                        group.text("Caption").unwrap_or(&group.id).to_owned(),
                    )));
                    content.extend(items);
                }
            }
        }
        let sidebar = Sidebar::new(content)
            .collapsible(SidebarCollapse::Icons)
            .narrow_below(NARROW_WINDOW)
            .state(self.sidebar.clone())
            .header(Text::new(self.session.title()).style(TextStyle::Headline).font_weight(Weight::SEMIBOLD));
        let (due, _) = self.session.reminders().get();
        let reminders = (due > 0).then(|| {
            let showing = self.session.reminders().clone();
            Button::new(format!("{} ({due})", self.session.words().reminders))
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || showing.update(|(_, open)| *open = true))
        });
        let account = self.session.account();
        let running = crate::jobs::running(&self.session);
        if running.is_none() && reminders.is_none() && account.is_none() && !self.theme_menu {
            return Box::new(sidebar);
        }
        let last: AnyView = match account {
            Some(account) => self.account_of(account),
            None => Box::new(self.theme_menu.then_some(ThemePicker::new())),
        };
        Box::new(sidebar.footer(VStack::new((running, reminders, last)).spacing(4.0).align(Align::Stretch)))
    }

    fn account_of(&self, account: crate::session::Account) -> AnyView {
        let words = self.session.words();
        let (changing, leaving, sign_out) =
            (self.session.clone(), self.session.clone(), account.sign_out.clone());
        Box::new(
            ravenrs_widgets::HStack::new((
                Text::new(account.name).style(TextStyle::Callout).grow(1.0),
                self.theme_menu.then_some(ThemePicker::new()),
                // Only where passwords are kept here: on a server, its own way in does this.
                self.session.frame().security().map(|_| {
                    IconButton::new(IconName::Key)
                        .label(words.change_password.clone())
                        .size(ButtonSize::Small)
                        .on_click(move || crate::security::show_change_password(&changing))
                }),
                IconButton::new(IconName::LogOut)
                    .label(words.sign_out.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        let sign_out = sign_out.clone();
                        leaving.leaving(move || sign_out());
                    }),
            ))
            .spacing(4.0)
            .align(Align::Center),
        )
    }

    fn navigation_item(&self, item: &ModelNode, view: &str, current: Option<&ViewInfo>) -> AnyView {
        let model = self.session.model();
        let entity_of = |id: &str| model.at(&["Views", id]).and_then(|n| n.text("Entity"));
        let selected = current
            .is_some_and(|c| c.id == view || (c.entity.is_some() && c.entity.as_deref() == entity_of(view)));
        let icon = item.text("Icon").and_then(icon_named).unwrap_or(ravenrs_widgets::IconName::File);
        let (session, target) = (self.session.clone(), view.to_owned());
        let key = item.text("ObjectKey").map(str::to_owned);
        Box::new(
            SidebarItem::new(icon, item.text("Caption").unwrap_or(&item.id).to_owned())
                .selected(selected)
                .on_click(move || session.open_navigation(&target, key.as_deref())),
        )
    }

    fn trail(&self, view: Option<&ViewInfo>) -> Vec<String> {
        let Some(view) = view else { return vec![self.session.title()] };
        let Some(entity) = view.entity.as_deref().and_then(|name| self.session.entity(name)) else {
            let model = self.session.model();
            let caption = model.at(&["Views", &view.id]).and_then(|node| node.text("Caption"));
            return vec![caption.unwrap_or(&view.id).to_owned()];
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

    fn header(&self, view: Option<&ViewInfo>, narrow: bool) -> AnyView {
        // In a narrow window the navigation is a sheet, and this is the way to it.
        let trigger = narrow.then(|| SidebarTrigger::new(self.sidebar.clone()));
        // What is showing in place of the view has its own commands.
        if self.session.full().is_some() {
            return match trigger {
                Some(trigger) => Box::new(
                    VStack::new((
                        VStack::new((trigger,))
                            .align(Align::Start)
                            .padding(Insets::symmetric(8.0, PAGE_GUTTER)),
                        Divider::horizontal(),
                    ))
                    .spacing(0.0)
                    .align(Align::Stretch),
                ),
                None => Box::new(VStack::new(Vec::<AnyView>::new())),
            };
        }
        // One place is a title; only going deeper makes it a way back.
        let trail = self.trail(view);
        let place: AnyView = match trail.as_slice() {
            [only] => {
                Box::new(Text::new(only.clone()).style(TextStyle::Headline).font_weight(Weight::SEMIBOLD))
            }
            _ => Box::new(Breadcrumb::new(trail)),
        };
        Box::new(
            VStack::new((
                VStack::new((
                    ravenrs_widgets::HStack::new((trigger, place)).spacing(8.0).align(Align::Center),
                    ActionContainer::new(self.session.clone(), []),
                ))
                .spacing(8.0)
                .align(Align::Stretch)
                .padding(Insets::symmetric(12.0, PAGE_GUTTER)),
                Divider::horizontal(),
            ))
            .spacing(0.0)
            .align(Align::Stretch),
        )
    }

    fn content(&self, view: Option<&ViewInfo>) -> AnyView {
        let session = &self.session;
        let body: AnyView = match view {
            _ if session.full().is_some() => session
                .full()
                .map_or_else(|| Box::new(Text::new(session.title())) as AnyView, |content| content()),
            Some(view) if view.kind == ViewKind::List => Box::new(Keyed::new(
                hash_of(&view.id),
                ListView::new(session.clone(), session.frame(), view.clone())
                    .padding(PAGE_GUTTER)
                    .stretch()
                    .grow(1.0),
            )),
            Some(view) if view.kind == ViewKind::Detail => {
                let form = session.form().map_or(0, |form| Rc::as_ptr(&form) as usize as u64);
                Box::new(ScrollView::new((Keyed::new(
                    form,
                    DetailView::new(session.clone(), view.clone()).padding(PAGE_GUTTER),
                ),)))
            }
            Some(view) if view.kind == ViewKind::Dashboard => Box::new(ScrollView::new((Keyed::new(
                hash_of(&view.id),
                crate::DashboardView::new(session.clone(), view.clone()).padding(PAGE_GUTTER),
            ),))),
            Some(view) => Box::new(Text::new(view.id.clone())),
            None => Box::new(Text::new(session.title()).style(TextStyle::Body)),
        };
        let description = view
            .filter(|_| session.full().is_none())
            .filter(|view| view.kind == ViewKind::List)
            .and_then(|view| {
                session
                    .model()
                    .at(&["Views", &view.id])
                    .and_then(|node| node.text("Description"))
                    .map(str::to_owned)
            })
            .map(|text| {
                VStack::new((Text::new(text)
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted)
                    .stretch()
                    .max_width(READING_WIDTH),))
                .align(Align::Stretch)
                .padding(Insets { top: PAGE_GUTTER, right: PAGE_GUTTER, bottom: 0.0, left: PAGE_GUTTER })
            });
        Box::new(
            VStack::new((
                description,
                session.error().map(|error| {
                    Alert::message(error).tone(Tone::Danger).padding(Insets {
                        top: PAGE_GUTTER,
                        right: PAGE_GUTTER,
                        bottom: 0.0,
                        left: PAGE_GUTTER,
                    })
                }),
                body,
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
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

impl View for AppShell {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The application's own look first, so a theme the person kept still wins over it.
        if let Some(theme) = self.theme
            && !self.theme_applied.replace(true)
        {
            cx.theme_state().set(theme);
        }
        self.keep_theme(cx);
        let session = &self.session;
        session.jobs().connect(cx.ui_waker());
        session.poll_background();
        let view = session.frame().view();
        let view = view.as_ref();
        // Above whatever the template does, so none can leave it out.
        let mut parts: Vec<NodeId> = session
            .notices()
            .into_iter()
            .enumerate()
            .map(|(index, notice)| {
                let tone = match notice.kind {
                    ravenrs_app::NoticeKind::Info => Tone::Accent,
                    ravenrs_app::NoticeKind::Danger => Tone::Danger,
                    _ => Tone::Warning,
                };
                cx.keyed_child(index as u64, &Alert::new(notice.title, notice.message).tone(tone))
            })
            .collect();
        if let Some(lock) = session.locked().get() {
            let locked = LockedWindow { session: session.clone(), title: lock.title, check: lock.check };
            parts.push(cx.child(&locked));
            return cx.container(
                LayoutStyle {
                    grow: 1.0,
                    min_height: Some(0.0),
                    align: Align::Stretch,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &parts,
                Paint::None,
            );
        }
        let narrow = cx.viewport().width < NARROW_WINDOW;
        let window = self.template.arrange(TemplateParts {
            navigation: self.navigation(view),
            header: self.header(view, narrow),
            content: self.content(view),
            keys: Box::new(ActionKeys { session: session.clone() }),
            kind: view.map(|view| view.kind),
        });
        parts.push(cx.child(&window));
        parts.push(cx.child(session.confirm()));
        parts.push(cx.child(&PopupHost::new(session.clone())));
        parts.push(cx.child(&session.toaster()));
        // What changed, for screen readers; it takes no room.
        parts.push(cx.child(&session.announcer().region()));
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::None,
        )
    }
}
