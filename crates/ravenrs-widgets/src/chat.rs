//! A conversation with a field to write in, and a panel that floats over an app.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use ravenrs_core::{
    Border, Color, CursorIcon, Effect, ImageData, Insets, Key, Point, Quad, Rect, Role, Scene, Size,
};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, Drag, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::conversation::file_name_and_size;
use crate::files::size_text;
use crate::modifiers::ViewExt;
use crate::motion::Motion;
use crate::view::{AnyView, View};
use crate::{
    AudioPlayerButton, Button, ButtonSize, ButtonStyle, HStack, Icon, IconButton, IconName, Image, RichText,
    Spinner, Text, TextField, VStack, util,
};

/// The widest a message gets, in logical pixels.
const MESSAGE_WIDTH: f32 = 320.0;
/// The narrowest a recording gets in a message.
const MEDIA_AUDIO_WIDTH: f32 = 240.0;

/// What a [`ChatMessage`](crate::ChatMessage) carries in its bubble, above its text: a photo,
/// a document or a recording. The file stays with the app, which asks to open it from
/// [`Self::on_open`].
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Chat, ChatMedia, ChatMessage, Ui};
///
/// let messages = State::new(vec![
///     ChatMessage::person("The *signed* contract")
///         .formatted()
///         .media(ChatMedia::document("contract.pdf").size(240_000).on_open(|| {})),
/// ]);
/// let mut ui = Ui::new(Chat::new(messages.binding()), ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone)]
pub struct ChatMedia {
    kind: MediaKind,
    on_open: Option<Rc<dyn Fn()>>,
}

#[derive(Clone)]
enum MediaKind {
    Photo { image: Option<ImageData>, side: f32, alt: Option<String> },
    Document { name: String, size: Option<u64>, icon: IconName },
    Audio(Rc<dyn Fn() -> AudioPlayerButton>),
}

impl fmt::Debug for ChatMedia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.kind {
            MediaKind::Photo { .. } => "photo",
            MediaKind::Document { .. } => "document",
            MediaKind::Audio(_) => "audio",
        };
        f.debug_struct("ChatMedia").field("kind", &kind).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChatMedia {
    /// A photo. While it is `None` it holds a space of the same size, so that what is above
    /// does not jump when it arrives.
    pub fn photo(image: Option<ImageData>) -> Self {
        Self { kind: MediaKind::Photo { image, side: 240.0, alt: None }, on_open: None }
    }

    /// A file shown as a card, with its name.
    pub fn document(name: impl Into<String>) -> Self {
        Self {
            kind: MediaKind::Document { name: name.into(), size: None, icon: IconName::FileText },
            on_open: None,
        }
    }

    /// A recording. The player is built each time the message is shown, so that the clip
    /// or the transcript that arrives later is there.
    pub fn audio(player: impl Fn() -> AudioPlayerButton + 'static) -> Self {
        Self { kind: MediaKind::Audio(Rc::new(player)), on_open: None }
    }

    /// The side of a photo, which is shown square (240 logical pixels by default).
    #[must_use]
    pub fn side(mut self, side: f32) -> Self {
        if let MediaKind::Photo { side: current, .. } = &mut self.kind {
            *current = side;
        }
        self
    }

    /// What a photo shows, for screen readers.
    #[must_use]
    pub fn alt(mut self, text: impl Into<String>) -> Self {
        if let MediaKind::Photo { alt, .. } = &mut self.kind {
            *alt = Some(text.into());
        }
        self
    }

    /// The size of a document in bytes.
    #[must_use]
    pub fn size(mut self, bytes: u64) -> Self {
        if let MediaKind::Document { size, .. } = &mut self.kind {
            *size = Some(bytes);
        }
        self
    }

    /// The icon of a document ([`IconName::FileText`] by default).
    #[must_use]
    pub fn icon(mut self, icon: IconName) -> Self {
        if let MediaKind::Document { icon: current, .. } = &mut self.kind {
            *current = icon;
        }
        self
    }

    /// Makes a photo or a document open with a click, Enter or Space: for instance in an
    /// [`ImageLightbox`](crate::ImageLightbox), or through the system.
    #[must_use]
    pub fn on_open(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl View for ChatMedia {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (node, name) = match &self.kind {
            MediaKind::Audio(player) => {
                let player = cx.child(&player());
                // A bubble fits what is inside, and a progress bar has no width of its own.
                return cx.container(
                    LayoutStyle {
                        min_width: Some(MEDIA_AUDIO_WIDTH),
                        align: Align::Stretch,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &[player],
                    Paint::None,
                );
            }
            MediaKind::Photo { image, side, alt } => {
                let name = alt.clone().unwrap_or_else(|| labels.chat_photo.clone());
                let radius = theme.radii.md;
                let content = match image {
                    Some(image) => {
                        cx.child(&Image::new(image.clone()).size(*side, *side).corner_radius(radius))
                    }
                    None => cx.child(
                        &Text::new(labels.chat_photo_loading.clone())
                            .style(TextStyle::Caption)
                            .foreground(Semantic::InkMuted),
                    ),
                };
                let node = cx.container(
                    LayoutStyle {
                        width: Some(*side),
                        height: Some(*side),
                        align: Align::Center,
                        justify: Justify::Center,
                        align_self: Some(Align::Start),
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &[content],
                    if image.is_some() {
                        Paint::None
                    } else {
                        Paint::Quad(QuadPaint {
                            corner_radius: radius,
                            ..QuadPaint::new(theme.color(Semantic::Surface))
                        })
                    },
                );
                if self.on_open.is_none() {
                    cx.semantics(node, Semantics::new(Role::Image).name(name.clone()));
                }
                (node, name)
            }
            MediaKind::Document { name, size, icon } => {
                let icon = cx.child(&Icon::new(*icon).size(20.0).color(theme.color(Semantic::InkMuted)));
                let title = cx.child(&Text::new(name.clone()).style(TextStyle::Callout).truncate());
                let mut lines = vec![title];
                if let Some(size) = size {
                    lines.push(cx.child(
                        &Text::new(size_text(*size)).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                    ));
                }
                let lines = cx.container(
                    LayoutStyle {
                        grow: 1.0,
                        min_width: Some(0.0),
                        align: Align::Stretch,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &lines,
                    Paint::None,
                );
                let surface = if cx.is_hovered() && self.on_open.is_some() {
                    Semantic::SurfaceElevated
                } else {
                    Semantic::Surface
                };
                let node = cx.container(
                    LayoutStyle {
                        spacing: theme.spacing.s2,
                        align: Align::Center,
                        align_self: Some(Align::Stretch),
                        min_width: Some(0.0),
                        padding: Insets::symmetric(theme.spacing.s2, theme.spacing.s3),
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[icon, lines],
                    Paint::Quad(QuadPaint {
                        corner_radius: theme.radii.md,
                        ..QuadPaint::new(theme.color(surface))
                    }),
                );
                let named = file_name_and_size(name, *size);
                if self.on_open.is_none() {
                    cx.semantics(node, Semantics::new(Role::Group).name(named.clone()));
                }
                (node, named)
            }
        };
        if let Some(on_open) = &self.on_open {
            cx.on_click(node, on_open.clone());
            cx.focusable(node, theme.radii.md);
            cx.cursor(node, CursorIcon::Pointer);
            cx.semantics(node, Semantics::new(Role::Button).name(labels.chat_open.replace("{what}", &name)));
        }
        node
    }
}

/// Who wrote a [`ChatMessage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ChatAuthor {
    /// The person at the screen.
    Person,
    /// Whatever answers them.
    Assistant,
}

/// One message of a [`Chat`].
#[derive(Clone)]
pub struct ChatMessage {
    author: ChatAuthor,
    text: Arc<Mutex<String>>,
    card: Option<Rc<dyn Fn() -> AnyView>>,
    media: Option<ChatMedia>,
    formatted: bool,
}

impl fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatMessage")
            .field("author", &self.author)
            .field("text", &self.text())
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChatMessage {
    /// What the person wrote.
    pub fn person(text: impl Into<String>) -> Self {
        Self::of(ChatAuthor::Person, Arc::new(Mutex::new(text.into())))
    }

    /// What the assistant answered.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::of(ChatAuthor::Assistant, Arc::new(Mutex::new(text.into())))
    }

    /// What the assistant is still answering. Whoever answers writes to `text`, from
    /// any thread, and asks for a rebuild with a [`UiWaker`](crate::UiWaker). While it
    /// is empty, the message shows that it is on its way.
    pub fn assistant_writing(text: Arc<Mutex<String>>) -> Self {
        Self::of(ChatAuthor::Assistant, text)
    }

    fn of(author: ChatAuthor, text: Arc<Mutex<String>>) -> Self {
        Self { author, text, card: None, media: None, formatted: false }
    }

    /// A photo, a document or a recording in the bubble, above the text, which may be empty.
    #[must_use]
    pub fn media(mut self, media: ChatMedia) -> Self {
        self.media = Some(media);
        self
    }

    /// Reads its text as a [`RichText`], with marks and links.
    #[must_use]
    pub fn formatted(mut self) -> Self {
        self.formatted = true;
        self
    }

    /// Something to act on under the text, such as buttons. It is built each time
    /// the message is shown.
    #[must_use]
    pub fn card(mut self, card: impl Fn() -> AnyView + 'static) -> Self {
        self.card = Some(Rc::new(card));
        self
    }

    /// Who wrote it.
    pub fn author(&self) -> ChatAuthor {
        self.author
    }

    /// What it says.
    pub fn text(&self) -> String {
        self.text.lock().map(|text| text.clone()).unwrap_or_default()
    }
}

/// A conversation over a field to write in. It fills what its parent gives it and
/// follows the newest message.
///
/// It does not know who answers: `on_send` gets what the person wrote, already in
/// the list, and whoever answers adds to `messages`, now or later.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Chat, ChatMessage};
///
/// let messages = State::new(Vec::<ChatMessage>::new());
/// let answers = messages.clone();
/// let chat = Chat::new(messages.binding())
///     .placeholder("Ask something")
///     .examples(["How do I start?"])
///     .on_send(move |_text| answers.update(|m| m.push(ChatMessage::assistant("Hello!"))));
/// ```
pub struct Chat {
    messages: Binding<Vec<ChatMessage>>,
    placeholder: String,
    intro: Option<String>,
    examples: Vec<String>,
    on_send: Option<Rc<dyn Fn(String)>>,
    on_listen: Option<Rc<dyn Fn(String)>>,
    on_link: Option<Rc<dyn Fn(String)>>,
    busy: bool,
}

impl fmt::Debug for Chat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chat").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Chat {
    /// A conversation over `messages`.
    pub fn new(messages: Binding<Vec<ChatMessage>>) -> Self {
        Self {
            messages,
            placeholder: String::new(),
            intro: None,
            examples: Vec::new(),
            on_send: None,
            on_listen: None,
            on_link: None,
            busy: false,
        }
    }

    /// Muted text inside the empty field.
    #[must_use]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// What it says before the first message.
    #[must_use]
    pub fn intro(mut self, text: impl Into<String>) -> Self {
        self.intro = Some(text.into());
        self
    }

    /// Things to ask, offered as buttons before the first message.
    #[must_use]
    pub fn examples(mut self, examples: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.examples = examples.into_iter().map(Into::into).collect();
        self
    }

    /// Whether an answer is on its way: nothing is sent until it arrives, and what is
    /// being written stays in the field.
    #[must_use]
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Puts a button to hear each answer, which `handler` gets as text.
    #[must_use]
    pub fn on_listen(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_listen = Some(Rc::new(handler));
        self
    }

    /// Called with the address of a link in a [`ChatMessage::formatted`] message.
    #[must_use]
    pub fn on_link(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    /// Called with what the person sent, after it joined the messages.
    #[must_use]
    pub fn on_send(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_send = Some(Rc::new(handler));
        self
    }
}

/// Adds what was written to the messages and passes it on.
#[derive(Clone)]
struct Send {
    messages: Binding<Vec<ChatMessage>>,
    draft: State<String>,
    on_send: Option<Rc<dyn Fn(String)>>,
    busy: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Send {
    fn text(&self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.busy {
            return;
        }
        self.draft.set(String::new());
        self.messages.update(|messages| messages.push(ChatMessage::person(text)));
        if let Some(on_send) = &self.on_send {
            on_send(text.to_owned());
        }
    }
}

/// The messages, scrolled to the newest when one arrives.
struct Conversation {
    messages: Vec<ChatMessage>,
    empty: Option<AnyView>,
    on_listen: Option<Rc<dyn Fn(String)>>,
    on_link: Option<Rc<dyn Fn(String)>>,
}

impl View for Conversation {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let seen = cx.use_state(|| Rc::new(Cell::new(0_usize))).get();
        let mut children = Vec::new();
        if let Some(empty) = &self.empty {
            children.push(cx.child(empty.as_ref()));
        }
        for (i, message) in self.messages.iter().enumerate() {
            children.push(cx.keyed_child(
                i as u64,
                &Bubble(message.clone(), self.on_listen.clone(), self.on_link.clone()),
            ));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s3,
                grow: 1.0,
                align: Align::Stretch,
                overflow_hidden: true,
                min_height: Some(0.0),
                // The right side leaves room for the scrollbar.
                padding: Insets {
                    top: theme.spacing.s2,
                    bottom: theme.spacing.s2,
                    left: 0.0,
                    right: theme.spacing.s3,
                },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        cx.scrollable(node);
        if seen.replace(self.messages.len()) != self.messages.len() {
            // Clamped to the content, which is not laid out yet.
            cx.set_scroll_offset(Point::new(0.0, f32::MAX));
        }
        node
    }
}

/// One message.
struct Bubble(ChatMessage, Option<Rc<dyn Fn(String)>>, Option<Rc<dyn Fn(String)>>);

impl View for Bubble {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let person = self.0.author == ChatAuthor::Person;
        let words = self.0.text();
        let media = self.0.media.as_ref().map(|media| cx.child(media));
        let text = if !words.is_empty() {
            Some(if self.0.formatted {
                let rich = RichText::new(words.clone()).foreground(Semantic::Ink);
                cx.child(&match self.2.clone() {
                    Some(on_link) => rich.on_link(move |address| on_link(address)),
                    None => rich,
                })
            } else {
                cx.child(&Text::new(words.clone()).foreground(Semantic::Ink))
            })
        } else if media.is_none() {
            Some(cx.child(&Spinner::new().size(16.0)))
        } else {
            None
        };
        let inside: Vec<NodeId> = media.into_iter().chain(text).collect();
        let bubble = cx.container(
            LayoutStyle {
                padding: if person {
                    Insets::symmetric(theme.spacing.s2, theme.spacing.s3)
                } else {
                    Insets::symmetric(theme.spacing.s1, 0.0)
                },
                max_width: Some(MESSAGE_WIDTH),
                spacing: if inside.len() > 1 { theme.spacing.s2 } else { 0.0 },
                align: Align::Stretch,
                align_self: Some(if person { Align::End } else { Align::Start }),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &inside,
            // What someone asked is set apart; the answer is the page itself.
            if person {
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.lg,
                    ..QuadPaint::new(theme.color(Semantic::Control))
                })
            } else {
                Paint::None
            },
        );
        // A card takes the whole width, outside the bubble: it has its own surface.
        let mut children = vec![bubble];
        if let Some(on_listen) = self.1.clone().filter(|_| !person && !words.is_empty()) {
            let listen = IconButton::new(IconName::Volume2)
                .label(cx.labels().listen.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || on_listen(words.clone()));
            let listen = HStack::new((listen,));
            children.push(cx.child(&listen));
        }
        if let Some(card) = &self.0.card {
            let card = card();
            children.push(cx.child(card.as_ref()));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        if !person {
            // Announced as it appears.
            cx.semantics(node, Semantics::new(Role::Status));
        }
        node
    }
}

impl View for Chat {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let draft: State<String> = cx.use_state(String::new);
        let send = Send {
            messages: self.messages.clone(),
            draft: draft.clone(),
            on_send: self.on_send.clone(),
            busy: self.busy,
        };
        let messages = self.messages.get();
        let empty = messages.is_empty().then(|| -> AnyView {
            let examples: Vec<Button> = self
                .examples
                .iter()
                .map(|example| {
                    let (send, example) = (send.clone(), example.clone());
                    Button::new(example.clone())
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .on_click(move || send.text(&example))
                })
                .collect();
            Box::new(
                VStack::new((
                    self.intro.clone().map(|intro| {
                        Text::new(intro).style(TextStyle::Callout).foreground(Semantic::InkMuted)
                    }),
                    HStack::new(examples).spacing(theme.spacing.s2).wrap(),
                ))
                .spacing(theme.spacing.s3)
                .align(Align::Stretch),
            )
        });
        let conversation = cx.child(&Conversation {
            messages,
            empty,
            on_listen: self.on_listen.clone(),
            on_link: self.on_link.clone(),
        });

        let (on_field, on_button, written) = (send.clone(), send, draft.clone());
        let writing = HStack::new((
            TextField::new(draft.binding())
                .placeholder(self.placeholder.clone())
                .on_submit(move |text| on_field.text(&text))
                .grow(1.0),
            IconButton::new(IconName::Send)
                .label(cx.labels().send.clone())
                .style(ButtonStyle::Primary)
                .disabled(self.busy)
                .on_click(move || on_button.text(&written.get())),
        ))
        .spacing(theme.spacing.s2)
        .align(Align::Center);
        let writing = cx.child(&writing);
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s3,
                grow: 1.0,
                align: Align::Stretch,
                min_height: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[conversation, writing],
            Paint::None,
        )
    }
}

/// A corner of the window, where a [`FloatingPanel`] rests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FloatingCorner {
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    #[default]
    BottomRight,
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl FloatingCorner {
    fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }

    fn at(right: bool, bottom: bool) -> Self {
        match (right, bottom) {
            (false, false) => Self::TopLeft,
            (true, false) => Self::TopRight,
            (false, true) => Self::BottomLeft,
            (true, true) => Self::BottomRight,
        }
    }

    /// The corner nearest to a point of a window of this size.
    fn nearest(point: Point, window: Size) -> Self {
        Self::at(point.x > window.width / 2.0, point.y > window.height / 2.0)
    }

    /// The corner an arrow key leads to, if it leads anywhere.
    fn moved(self, key: &Key) -> Option<Self> {
        let (right, bottom) = (self.is_right(), self.is_bottom());
        let next = match key {
            Key::ArrowLeft => Self::at(false, bottom),
            Key::ArrowRight => Self::at(true, bottom),
            Key::ArrowUp => Self::at(right, false),
            Key::ArrowDown => Self::at(right, true),
            _ => return None,
        };
        Some(next)
    }
}

/// A round button in the corner of the window that opens a panel over the app.
/// The app stays usable behind it, and Escape or the sphere closes it. Dragging it
/// or pressing an arrow key sends it to another corner, and it stays there while
/// the app runs.
///
/// It moves while the pointer or the focus is on it. The rest of the time it holds
/// still and asks for no frames, like the rest of the UI.
///
/// Whatever must outlive closing it, such as a [`Chat`]'s messages, belongs outside.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FloatingPanel, Text};
///
/// let open = State::new(false);
/// let panel = FloatingPanel::new(open.binding(), "Assistant", || {
///     Box::new(Text::new("Hello"))
/// });
/// ```
pub struct FloatingPanel {
    open: Binding<bool>,
    title: String,
    content: Rc<dyn Fn() -> AnyView>,
    width: f32,
    height: f32,
    bottom_inset: f32,
    place: Option<Binding<Option<Point>>>,
    minimized: Option<Binding<bool>>,
    summary: Option<Rc<dyn Fn() -> AnyView>>,
}

impl fmt::Debug for FloatingPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FloatingPanel").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FloatingPanel {
    /// A closed panel called `title`, the name screen readers give its sphere.
    pub fn new(
        open: Binding<bool>,
        title: impl Into<String>,
        content: impl Fn() -> AnyView + 'static,
    ) -> Self {
        Self {
            open,
            title: title.into(),
            content: Rc::new(content),
            width: 400.0,
            height: 520.0,
            bottom_inset: 0.0,
            place: None,
            minimized: None,
            summary: None,
        }
    }

    /// Its size in logical pixels (400 by 520 by default). A smaller window gets
    /// less height.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Keeps it this far, in logical pixels, from the bottom of the window, to clear
    /// something fixed there, such as a [`StatusBar`](crate::StatusBar).
    #[must_use]
    pub fn bottom_inset(mut self, inset: f32) -> Self {
        self.bottom_inset = inset;
        self
    }

    /// Lets it go anywhere in the window, dragged by its header or moved with the arrow
    /// keys on it, without the sphere: `open` is what opens it. `place` holds its top-left
    /// corner, so the app can remember it, and is written when a drag ends; `None` puts it
    /// in the corner at the bottom right. It stays inside a window that gets smaller.
    #[must_use]
    pub fn movable(mut self, place: Binding<Option<Point>>) -> Self {
        self.place = Some(place);
        self
    }

    /// Adds a button that reduces it to its header and brings it back, kept in `minimized`.
    #[must_use]
    pub fn minimized(mut self, minimized: Binding<bool>) -> Self {
        self.minimized = Some(minimized);
        self
    }

    /// What the header shows while it is reduced, such as how things stand inside.
    #[must_use]
    pub fn summary(mut self, summary: impl Fn() -> AnyView + 'static) -> Self {
        self.summary = Some(Rc::new(summary));
        self
    }

    fn is_minimized(&self) -> bool {
        self.minimized.as_ref().is_some_and(Binding::get)
    }

    /// The height of the header alone, the height of a reduced panel.
    fn header_height(cx: &BuildCx<'_>) -> f32 {
        ButtonSize::Small.height() + cx.theme().spacing.s4 * 2.0
    }

    /// The panel's surface, with its header and, unless reduced, its content. The header
    /// comes back too, for whoever makes it the place to drag from.
    fn surface(&self, cx: &mut BuildCx<'_>, height: f32) -> (NodeId, NodeId) {
        let theme = *cx.theme();
        let close = self.open.clone();
        let minimized = self.is_minimized();
        let title = util::label(
            cx,
            &self.title,
            TextStyle::Headline,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        );
        let x = cx.child(
            &IconButton::new(IconName::X)
                .label(cx.labels().close.clone())
                .size(ButtonSize::Small)
                .on_click(move || close.set(false)),
        );
        let mut header = vec![title];
        let mut buttons = vec![x];
        if let Some(reduce) = &self.minimized {
            let (labels, reduce) = (cx.labels(), reduce.clone());
            let (icon, label) = if minimized {
                (IconName::Maximize2, labels.panel_restore.clone())
            } else {
                (IconName::Minus, labels.panel_minimize.clone())
            };
            buttons.insert(
                0,
                cx.keyed_child(
                    PANEL_REDUCE_KEY,
                    &IconButton::new(icon)
                        .label(label)
                        .size(ButtonSize::Small)
                        .on_click(move || reduce.set(!reduce.get())),
                ),
            );
            if let Some(summary) = self.summary.as_ref().filter(|_| minimized) {
                let summary = summary();
                let summary = cx.keyed_child(PANEL_SUMMARY_KEY, summary.as_ref());
                header.push(summary);
            }
            let title_side = cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &header,
                Paint::None,
            );
            header = vec![title_side];
            let buttons_side = cx.container(
                LayoutStyle { spacing: theme.spacing.s1, ..LayoutStyle::stack(Axis::Horizontal) },
                &buttons,
                Paint::None,
            );
            buttons = vec![buttons_side];
        }
        header.append(&mut buttons);
        let header = cx.container(
            LayoutStyle {
                justify: Justify::SpaceBetween,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &header,
            Paint::None,
        );
        let mut children = vec![header];
        if !minimized {
            let content = (self.content)();
            children.push(cx.child(content.as_ref()));
        }
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(if minimized { Self::header_height(cx) } else { height }),
                spacing: theme.spacing.s2,
                padding: Insets::uniform(theme.spacing.s4),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.semantics(node, Semantics::new(Role::Group).name(self.title.clone()));
        cx.motion(
            node,
            Motion::new()
                .enter(Effect::moved(0.0, 12.0), theme.motion.gentle)
                .exit(Effect::moved(0.0, 12.0), theme.motion.fast),
        );
        (node, header)
    }
}

/// Keeps the reduce button's state while the header changes around it.
const PANEL_REDUCE_KEY: u64 = 2;
/// Keeps the state of what a reduced panel shows.
const PANEL_SUMMARY_KEY: u64 = 3;
/// How far from the window's edges a layer stays, as every overlay does.
pub(crate) const PANEL_MARGIN: f32 = 4.0;

/// `p` kept inside the window, given the largest place a panel can have in it.
pub(crate) fn panel_inside(p: Point, largest: Point) -> Point {
    Point::new(p.x.clamp(PANEL_MARGIN, largest.x), p.y.clamp(PANEL_MARGIN, largest.y))
}

/// How far an arrow key moves a panel, in logical pixels.
const PANEL_STEP: f32 = 16.0;

/// Makes `handle` move something placed anywhere, by dragging and with the arrow keys. While
/// it is dragged the place stays in `dragging`, and goes to `place` only at the end, since the
/// app may write it to a file.
pub(crate) fn move_by_handle(
    cx: &mut BuildCx<'_>,
    handle: NodeId,
    at: Point,
    largest: Point,
    place: &Binding<Option<Point>>,
    dragging: &State<Option<Point>>,
) {
    let keep_in = move |p: Point| panel_inside(p, largest);
    let grab = cx.use_state(|| Rc::new(Cell::new(Point::ZERO))).get();
    let (dropped, dragging) = (place.clone(), dragging.clone());
    cx.on_drag(
        handle,
        Rc::new(move |drag: Drag| match drag.phase {
            DragPhase::Start => grab.set(Point::new(drag.position.x - at.x, drag.position.y - at.y)),
            DragPhase::Move => {
                let from = grab.get();
                dragging.set(Some(keep_in(Point::new(drag.position.x - from.x, drag.position.y - from.y))));
            }
            DragPhase::End => {
                if let Some(last) = *dragging.peek() {
                    dropped.set(Some(last));
                }
                dragging.set(None);
            }
        }),
    );
    let place = place.clone();
    cx.on_key(
        handle,
        Rc::new(move |key, modifiers| {
            let far = if modifiers.shift { PANEL_STEP * 4.0 } else { PANEL_STEP };
            let (dx, dy) = match key {
                Key::ArrowLeft => (-far, 0.0),
                Key::ArrowRight => (far, 0.0),
                Key::ArrowUp => (0.0, -far),
                Key::ArrowDown => (0.0, far),
                _ => return false,
            };
            place.set(Some(keep_in(Point::new(at.x + dx, at.y + dy))));
            true
        }),
    );
}

/// A panel placed anywhere, dragged by its header.
struct FreeLayer<'a> {
    panel: &'a FloatingPanel,
    height: f32,
    /// Where it is now, and the limits of where it can go.
    at: Point,
    largest: Point,
    place: Binding<Option<Point>>,
    dragging: State<Option<Point>>,
}

impl View for FreeLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (node, header) = self.panel.surface(cx, self.height);
        move_by_handle(cx, header, self.at, self.largest, &self.place, &self.dragging);
        let labels = cx.labels();
        cx.semantics(
            header,
            Semantics::new(Role::Button).name(format!("{}: {}", labels.panel_move, self.panel.title)),
        );
        cx.focusable(header, cx.theme().radii.sm);
        cx.cursor(header, CursorIcon::Pointer);
        node
    }
}

/// The room the sphere and its glow take, in logical pixels.
const ORB_BOX: f32 = 60.0;
/// The sphere itself.
const ORB_SPHERE: f32 = 42.0;
/// What surrounds the sphere inside its box: the panel lines up with the sphere, not the box.
const ORB_MARGIN: f32 = (ORB_BOX - ORB_SPHERE) / 2.0;
/// How far the sphere's box keeps from the edges of the window.
const ORB_EDGE: f32 = 15.0;
/// How far the pointer has to go for a press to become a drag, in logical pixels.
const ORB_DRAG_FROM: f32 = 5.0;
/// The moment of its motion it holds when still.
const ORB_REST: f64 = 0.8;

/// The theme's colors, for painting the sphere.
#[derive(Clone, Copy)]
struct OrbColors {
    surface: Color,
    border: Color,
    accent: Color,
}

/// Paints the sphere: a calm disc over the page, leaving the accent to the icon and
/// to the screen's one main action. All of it is circles, so it needs nothing from the
/// renderer and looks the same in the browser.
fn paint_orb(scene: &mut Scene, frame: Rect, time: f64, colors: OrbColors, moving: bool) {
    let center = frame.center();
    let radius = ORB_SPHERE / 2.0;
    let circle = |x: f32, y: f32, r: f32| Rect::new(x - r, y - r, r * 2.0, r * 2.0);

    // A soft shadow under it, to lift it from whatever is on the page.
    for ring in 0..4 {
        let ring = ring as f32;
        let r = radius + 1.0 + ring * 1.4;
        let shadow = Color::BLACK.with_alpha(0.07 - ring * 0.015);
        scene.push_quad(Quad::new(circle(center.x, center.y + 2.0, r), shadow).radius(r));
    }
    // A ring going out, only while the pointer or the focus is on it.
    if moving {
        let progress = (time as f32 / 1.8).fract();
        let r = radius + 1.0 + progress * 7.0;
        scene.push_quad(Quad {
            border: Some(Border { width: 1.5, color: colors.accent.with_alpha((1.0 - progress) * 0.5) }),
            ..Quad::new(circle(center.x, center.y, r), Color::TRANSPARENT).radius(r)
        });
    }
    scene.push_quad(Quad {
        border: Some(Border { width: 1.0, color: colors.border }),
        ..Quad::new(circle(center.x, center.y, radius), colors.surface).radius(radius)
    });
}

/// The sphere: a button that can also be dragged to another corner.
struct Orb {
    title: String,
    open: Binding<bool>,
    corner: State<FloatingCorner>,
    dragging: State<Option<Point>>,
}

impl View for Orb {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let colors = OrbColors {
            surface: theme.color(Semantic::SurfaceElevated),
            border: theme.color(Semantic::Border),
            accent: theme.color(Semantic::Accent),
        };
        let moving = cx.is_hovered() || cx.is_focus_visible() || self.dragging.peek().is_some();
        let paint = if moving {
            Paint::Live(Rc::new(move |frame: Rect, time, scene: &mut Scene| {
                paint_orb(scene, frame, time, colors, true);
            }))
        } else {
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                paint_orb(scene, frame, ORB_REST, colors, false);
            }))
        };
        let icon = cx.child(&crate::Icon::new(IconName::Sparkles).color(Semantic::Accent));
        let node = cx.container(
            LayoutStyle {
                width: Some(ORB_BOX),
                height: Some(ORB_BOX),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[icon],
            paint,
        );
        cx.semantics(node, Semantics::new(Role::Button).name(self.title.clone()).expanded(self.open.get()));
        // A drag starts with the press, and the sphere is still under the pointer at
        // the release: only moving away from where it was pressed makes it a drag.
        let pressed = cx.use_state(|| Rc::new(Cell::new(None::<Point>))).get();
        let dragged = cx.use_state(|| Rc::new(Cell::new(false))).get();
        let (toggle, was_dragged) = (self.open.clone(), dragged.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                if !was_dragged.replace(false) {
                    toggle.update(|open| *open = !*open);
                }
            }),
        );
        cx.focusable(node, ORB_BOX / 2.0);
        cx.cursor(node, CursorIcon::Pointer);
        let (corner, dragging, window) = (self.corner.clone(), self.dragging.clone(), cx.viewport());
        cx.on_drag(
            node,
            Rc::new(move |drag: Drag| match drag.phase {
                DragPhase::Start => {
                    pressed.set(Some(drag.position));
                    dragged.set(false);
                }
                DragPhase::Move => {
                    let far = pressed.get().is_some_and(|from| {
                        (drag.position.x - from.x).hypot(drag.position.y - from.y) > ORB_DRAG_FROM
                    });
                    if far || dragged.get() {
                        dragged.set(true);
                        dragging.set(Some(drag.position));
                    }
                }
                DragPhase::End => {
                    if dragged.get() {
                        corner.set(FloatingCorner::nearest(drag.position, window));
                        dragging.set(None);
                    }
                }
            }),
        );
        let corner = self.corner.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let next = corner.peek().moved(key);
                next.map(|next| corner.set(next)).is_some()
            }),
        );
        node
    }
}

/// The panel and its sphere.
struct Layer<'a> {
    panel: &'a FloatingPanel,
    corner: FloatingCorner,
    height: f32,
    orb: Orb,
}

impl View for Layer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let panel = self.panel;
        let orb = cx.keyed_child(ORB_KEY, &self.orb);
        let showing = panel.open.get() && self.orb.dragging.peek().is_none();
        let mut children = vec![orb];
        if showing {
            let (node, _) = panel.surface(cx, self.height);
            let (left, right) = if self.corner.is_right() { (0.0, ORB_MARGIN) } else { (ORB_MARGIN, 0.0) };
            let node = cx.container(
                LayoutStyle {
                    padding: Insets { top: 0.0, bottom: 0.0, left, right },
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[node],
                Paint::None,
            );
            // The panel opens toward the middle of the window.
            if self.corner.is_bottom() {
                children.insert(0, node);
            } else {
                children.push(node);
            }
        }
        cx.container(
            LayoutStyle {
                spacing: PANEL_GAP,
                align: if self.corner.is_right() { Align::End } else { Align::Start },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// Keeps the sphere's state while the panel comes and goes around it.
const ORB_KEY: u64 = 1;
const PANEL_GAP: f32 = 4.0;

impl View for FloatingPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        let corner = cx.use_state(FloatingCorner::default);
        let dragging: State<Option<Point>> = cx.use_state(|| None);
        let open = self.open.get();
        if open {
            let open = self.open.clone();
            cx.shortcut(Key::Escape, false, Rc::new(move || open.set(false)));
        }
        let window = cx.viewport();
        if let Some(place) = &self.place {
            if open {
                let free = (window.height - self.bottom_inset - ORB_EDGE * 2.0).max(0.0);
                let height = self.height.min(free);
                let tall = if self.is_minimized() { Self::header_height(cx) } else { height };
                let largest = Point::new(
                    (window.width - self.width - PANEL_MARGIN).max(PANEL_MARGIN),
                    (window.height - self.bottom_inset - tall - PANEL_MARGIN).max(PANEL_MARGIN),
                );
                let corner = Point::new(
                    (largest.x - ORB_EDGE).max(PANEL_MARGIN),
                    (largest.y - ORB_EDGE).max(PANEL_MARGIN),
                );
                let wanted = dragging.get().or_else(|| place.get()).unwrap_or(corner);
                let at = panel_inside(wanted, largest);
                let layer = FreeLayer { panel: self, height, at, largest, place: place.clone(), dragging };
                cx.floating_overlay(anchor, Placement::At(at), &layer);
            }
            return anchor;
        }
        let here = corner.get();
        let held = dragging.get();
        let free = window.height - self.bottom_inset - ORB_BOX - PANEL_GAP - ORB_EDGE * 2.0;
        let height = self.height.min(free.max(0.0));
        let body = if self.is_minimized() { Self::header_height(cx) } else { height };
        let tall = if open && held.is_none() { body + PANEL_GAP + ORB_BOX } else { ORB_BOX };
        let wide = if open && held.is_none() { (self.width + ORB_MARGIN).max(ORB_BOX) } else { ORB_BOX };
        let place = held.map_or_else(
            || {
                Point::new(
                    if here.is_right() { window.width - wide - ORB_EDGE } else { ORB_EDGE },
                    if here.is_bottom() {
                        window.height - self.bottom_inset - tall - ORB_EDGE
                    } else {
                        ORB_EDGE
                    },
                )
            },
            |pointer| Point::new(pointer.x - ORB_BOX / 2.0, pointer.y - ORB_BOX / 2.0),
        );
        let layer = Layer {
            panel: self,
            corner: here,
            height,
            orb: Orb { title: self.title.clone(), open: self.open.clone(), corner, dragging },
        };
        cx.floating_overlay(anchor, Placement::At(place), &layer);
        anchor
    }
}
