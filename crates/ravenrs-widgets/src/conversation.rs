//! Pieces of a conversation: files on their way, a recording to hear, and what to do with
//! a selection.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, ImageData, Insets, Key, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::files::size_text;
use crate::{
    Button, ButtonSize, ButtonStyle, DropdownMenu, Icon, IconButton, IconName, Image, MenuItem, SoundClip,
    Text, Toolbar, ToolbarAction, View, ViewExt,
};

/// The name of a file with its size: what a row of a file says first.
pub(crate) fn file_name_and_size(name: &str, size: Option<u64>) -> String {
    size.map_or_else(|| name.to_owned(), |size| format!("{name}, {}", size_text(size)))
}

/// How a file can be sent, for an [`Attachment`].
struct SendOptions {
    options: Vec<String>,
    chosen: usize,
    on_choose: Rc<dyn Fn(usize)>,
}

/// A file in an [`AttachmentStrip`].
pub struct Attachment {
    name: String,
    size: Option<u64>,
    icon: IconName,
    thumbnail: Option<ImageData>,
    send_as: Option<SendOptions>,
}

impl fmt::Debug for Attachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attachment")
            .field("name", &self.name)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Attachment {
    /// A file called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), size: None, icon: IconName::File, thumbnail: None, send_as: None }
    }

    /// Its size in bytes, which is shown in the units people read.
    #[must_use]
    pub fn size(mut self, bytes: u64) -> Self {
        self.size = Some(bytes);
        self
    }

    /// The icon before its name ([`IconName::File`] by default).
    #[must_use]
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// A small picture in place of the icon.
    #[must_use]
    pub fn thumbnail(mut self, image: ImageData) -> Self {
        self.thumbnail = Some(image);
        self
    }

    /// The ways this file can be sent, such as a photo or a document, with `chosen` selected.
    /// `on_choose` gets the index of another one. With fewer than two there is nothing to
    /// ask, and the row does not ask.
    #[must_use]
    pub fn send_as(
        mut self,
        options: impl IntoIterator<Item = impl Into<String>>,
        chosen: usize,
        on_choose: impl Fn(usize) + 'static,
    ) -> Self {
        self.send_as = Some(SendOptions {
            options: options.into_iter().map(Into::into).collect(),
            chosen,
            on_choose: Rc::new(on_choose),
        });
        self
    }
}

/// The files about to go with a message: icon or thumbnail, name, size, how to send each one
/// and a button to take it out. It keeps nothing: the app holds the list and changes it when
/// told.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Attachment, AttachmentStrip, Ui};
///
/// let removed = State::new(None::<usize>);
/// let told = removed.clone();
/// let strip = AttachmentStrip::new([
///     Attachment::new("report.pdf").size(120_000),
///     Attachment::new("photo.jpg").size(2_400_000).send_as(["photo", "document"], 0, |_| {}),
/// ])
/// .on_remove(move |index| told.set(Some(index)));
/// let mut ui = Ui::new(strip, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct AttachmentStrip {
    attachments: Vec<Attachment>,
    on_remove: Option<Rc<dyn Fn(usize)>>,
}

impl fmt::Debug for AttachmentStrip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentStrip").field("attachments", &self.attachments).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AttachmentStrip {
    /// A strip with these files.
    pub fn new(attachments: impl IntoIterator<Item = Attachment>) -> Self {
        Self { attachments: attachments.into_iter().collect(), on_remove: None }
    }

    /// Adds a button to each row, which calls `handler` with its index.
    #[must_use]
    pub fn on_remove(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

/// One row of an [`AttachmentStrip`], with an identity of its own for its menu and button.
struct AttachmentRow<'a> {
    attachment: &'a Attachment,
    index: usize,
    on_remove: Option<Rc<dyn Fn(usize)>>,
}

impl View for AttachmentRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let file = self.attachment;
        let side = theme.text(TextStyle::Body).line_height;
        let leading = match &file.thumbnail {
            Some(image) => {
                cx.child(&Image::new(image.clone()).size(side, side).corner_radius(theme.radii.sm))
            }
            None => cx.child(&Icon::new(file.icon).size(16.0).color(theme.color(Semantic::InkMuted))),
        };
        // The same room for either, so that the names line up.
        let leading = cx.container(
            LayoutStyle {
                width: Some(side),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[leading],
            Paint::None,
        );
        let mut children = vec![leading];
        children.push(cx.child(&Text::new(file.name.clone()).style(TextStyle::Callout).truncate().grow(1.0)));
        if let Some(size) = file.size {
            // It is the name that gives way: half a size says nothing.
            let (size, attrs) = (size_text(size), crate::util::attrs(&theme, TextStyle::Caption, None));
            let width = cx.measure_text(&size, &attrs).width;
            let style = LayoutStyle { min_width: Some(width + 1.0), ..LayoutStyle::leaf() };
            children.push(cx.text(&size, &attrs, theme.color(Semantic::InkMuted), style));
        }
        if let Some(send) = file.send_as.as_ref().filter(|send| send.options.len() > 1) {
            let chosen = send.options.get(send.chosen).or(send.options.first());
            let chosen = chosen.map_or("", String::as_str);
            let mut menu = DropdownMenu::new(labels.attachment_send_as.replace("{what}", chosen))
                .size(ButtonSize::Mini)
                .style(ButtonStyle::Secondary);
            for (i, option) in send.options.iter().enumerate() {
                let on_choose = send.on_choose.clone();
                menu = menu.item(
                    MenuItem::new(option.clone()).checked(i == send.chosen).on_select(move || on_choose(i)),
                );
            }
            children.push(cx.child(&menu));
        }
        if let Some(on_remove) = self.on_remove.clone() {
            let index = self.index;
            children.push(
                cx.child(
                    &IconButton::new(IconName::X)
                        .label(format!("{} {}", labels.attachment_remove, file.name))
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Mini)
                        .on_click(move || on_remove(index)),
                ),
            );
        }
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s2),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::ListItem).name(file_name_and_size(&file.name, file.size)));
        node
    }
}

impl View for AttachmentStrip {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let rows: Vec<NodeId> = self
            .attachments
            .iter()
            .enumerate()
            .map(|(index, attachment)| {
                cx.keyed_child(
                    index as u64,
                    &AttachmentRow { attachment, index, on_remove: self.on_remove.clone() },
                )
            })
            .collect();
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::List).name(cx.labels().attachments.clone()));
        node
    }
}

/// A bar for what to do with several things chosen at once: how many, a way out and the
/// actions, which are disabled while nothing is chosen. Escape also leaves. It goes where the
/// field to write in was, or over a list; the app decides when it shows.
///
/// ```
/// use ravenrs_widgets::{IconName, SelectionBar, ToolbarAction, Ui};
///
/// let bar = SelectionBar::new(3)
///     .on_exit(|| {})
///     .action(ToolbarAction::new("Copy").icon(IconName::Copy).icon_only().on_click(|| {}));
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct SelectionBar {
    count: usize,
    hint: Option<String>,
    on_exit: Option<Rc<dyn Fn()>>,
    actions: Toolbar,
}

impl fmt::Debug for SelectionBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectionBar").field("count", &self.count).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SelectionBar {
    /// A bar over `count` things chosen.
    pub fn new(count: usize) -> Self {
        Self { count, hint: None, on_exit: None, actions: Toolbar::new().aligned_end() }
    }

    /// What it says while nothing is chosen, such as how to choose. By default, that nothing
    /// is.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds a button to leave the selection, and makes Escape do the same.
    #[must_use]
    pub fn on_exit(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Rc::new(handler));
        self
    }

    /// Adds an action. Those that do not fit go to a menu, as in any [`Toolbar`].
    #[must_use]
    pub fn action(mut self, action: ToolbarAction) -> Self {
        let action = if self.count == 0 { action.disabled(true) } else { action };
        self.actions = self.actions.action(action);
        self
    }
}

impl View for SelectionBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let mut children = Vec::new();
        if let Some(on_exit) = &self.on_exit {
            cx.shortcut(Key::Escape, false, on_exit.clone());
            let on_exit = on_exit.clone();
            children.push(
                cx.child(
                    &IconButton::new(IconName::X)
                        .label(labels.selection_exit.clone())
                        .style(ButtonStyle::Ghost)
                        .on_click(move || on_exit()),
                ),
            );
        }
        let says = match self.count {
            0 => self.hint.clone().unwrap_or_else(|| labels.selection_none.clone()),
            1 => labels.selection_one.clone(),
            n => labels.selection_many.replace("{count}", &n.to_string()),
        };
        let count = cx.child(&Text::new(says.clone()).style(TextStyle::Callout));
        cx.semantics(count, Semantics::new(Role::Status).name(says));
        children.push(count);
        children.push(cx.child(&self.actions));
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s2),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group).name(labels.selection_bar.clone()));
        node
    }
}

/// `0:07` or `12:34`.
fn clock_text(seconds: f32) -> String {
    let whole = seconds.max(0.0) as u32;
    format!("{}:{:02}", whole / 60, whole % 60)
}

fn draw_progress(frame: Rect, fraction: f32, track: Color, fill: Color, scene: &mut Scene) {
    let radius = frame.height() / 2.0;
    scene.push_quad(Quad::new(frame, track).radius(radius));
    if fraction > 0.0 {
        let done = Rect::new(frame.x(), frame.y(), frame.width() * fraction.min(1.0), frame.height());
        scene.push_quad(Quad::new(done, fill).radius(radius));
    }
}

/// A button that plays a recording and stops it, with its progress and, when there is one, a
/// transcript to open. Space and Enter work on the button, as on any.
///
/// The app delivers the audio as a [`SoundClip`]. It can do it later: with
/// [`Self::on_request`], the first press asks for it and playing starts when it arrives.
///
/// It plays through [`Sound::speak_clip`](crate::Sound::speak_clip), so a new recording or
/// anything else said by the [`Sound`](crate::Sound) takes its place, and this button goes
/// back to rest. Progress is by the clock, as the service does not report where it is. The
/// sound does not stop by itself if the view leaves the screen while playing.
///
/// ```
/// use ravenrs_widgets::{AudioPlayerButton, SoundClip, Ui};
///
/// let audio = AudioPlayerButton::new()
///     .clip(SoundClip::new(vec![0.0; 44_100], 44_100))
///     .transcript("Your order is ready.");
/// let mut ui = Ui::new(audio, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone, Default)]
pub struct AudioPlayerButton {
    clip: Option<SoundClip>,
    seconds: Option<f32>,
    loading: bool,
    on_request: Option<Rc<dyn Fn()>>,
    transcript: Option<String>,
    transcribing: bool,
    on_transcribe: Option<Rc<dyn Fn()>>,
    transcript_open: Option<Binding<bool>>,
}

impl fmt::Debug for AudioPlayerButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioPlayerButton")
            .field("seconds", &self.length())
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AudioPlayerButton {
    /// A player with nothing to play yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The recording.
    #[must_use]
    pub fn clip(mut self, clip: SoundClip) -> Self {
        self.clip = Some(clip);
        self
    }

    /// How long it lasts, to show before the recording arrives.
    #[must_use]
    pub fn seconds(mut self, seconds: f32) -> Self {
        self.seconds = Some(seconds);
        self
    }

    /// Whether the recording is on its way, which holds the button until it arrives.
    #[must_use]
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Called when someone presses play with no recording here: the app brings it, and it
    /// plays when it arrives through [`Self::clip`]. Without this or a clip, the button stays
    /// disabled.
    #[must_use]
    pub fn on_request(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_request = Some(Rc::new(handler));
        self
    }

    /// What was said, under a button that shows and hides it. Blank text says there is no
    /// speech in the recording.
    #[must_use]
    pub fn transcript(mut self, text: impl Into<String>) -> Self {
        self.transcript = Some(text.into());
        self
    }

    /// With no transcript, offers a button that asks for one.
    #[must_use]
    pub fn on_transcribe(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_transcribe = Some(Rc::new(handler));
        self
    }

    /// Whether a transcript is on its way, which holds that button.
    #[must_use]
    pub fn transcribing(mut self, transcribing: bool) -> Self {
        self.transcribing = transcribing;
        self
    }

    /// Keeps whether the transcript is open in the app's hands. By default it starts closed
    /// and is kept here.
    #[must_use]
    pub fn transcript_open(mut self, open: Binding<bool>) -> Self {
        self.transcript_open = Some(open);
        self
    }

    fn length(&self) -> Option<f32> {
        self.clip.as_ref().map(SoundClip::seconds).or(self.seconds)
    }

    /// The button to read what was said, or to ask for it.
    fn transcript_part(&self, cx: &mut BuildCx<'_>) -> Vec<NodeId> {
        let labels = cx.labels();
        let Some(transcript) = &self.transcript else {
            let Some(on_transcribe) = self.on_transcribe.clone() else { return Vec::new() };
            let says = if self.transcribing {
                labels.audio_transcribing.clone()
            } else {
                labels.audio_transcribe.clone()
            };
            return vec![
                cx.child(
                    &Button::new(says)
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Mini)
                        .disabled(self.transcribing)
                        .on_click(move || on_transcribe()),
                ),
            ];
        };
        let local: State<bool> = cx.use_state(|| false);
        let open = self.transcript_open.clone().unwrap_or_else(|| local.binding());
        let is_open = open.get();
        let toggle = cx.child(
            &Button::new(labels.audio_transcript.clone())
                .icon(if is_open { IconName::ChevronDown } else { IconName::ChevronRight })
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .on_click(move || open.set(!open.get())),
        );
        cx.update_semantics(toggle, |s| s.expanded = Some(is_open));
        let mut parts = vec![toggle];
        if is_open {
            let theme = *cx.theme();
            let said = if transcript.trim().is_empty() {
                labels.audio_no_speech.clone()
            } else {
                transcript.clone()
            };
            let bar = cx.container(
                LayoutStyle { width: Some(2.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint {
                    corner_radius: 1.0,
                    ..QuadPaint::new(theme.color(Semantic::Border))
                }),
            );
            let said = cx.child(&Text::new(said).style(TextStyle::Callout).foreground(Semantic::InkMuted));
            parts.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    min_width: Some(0.0),
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[bar, said],
                Paint::None,
            ));
        }
        parts
    }
}

impl View for AudioPlayerButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let sound = cx.sound();
        let now = cx.now();
        let wanted: State<bool> = cx.use_state(|| false);
        let playing: State<Option<(u64, f64)>> = cx.use_state(|| None);
        let length = self.length().unwrap_or(0.0);

        // A press knows no clock, so it only asks, and playing starts on the build it causes.
        if wanted.get()
            && let Some(clip) = &self.clip
        {
            wanted.set(false);
            sound.speak_clip(clip);
            playing.set(Some((sound.turn(), now)));
        }
        let start = playing
            .get()
            .filter(|(turn, start)| *turn == sound.turn() && now < start + f64::from(length))
            .map(|(_, start)| start);
        if start.is_none() && playing.get().is_some() {
            playing.set(None);
        }
        let waiting = wanted.get() || self.loading;

        let (icon, label) = if start.is_some() {
            (IconName::Square, labels.audio_stop.clone())
        } else {
            (IconName::Play, labels.audio_play.clone())
        };
        let request = self.on_request.clone();
        let has_clip = self.clip.is_some();
        let (stopper, wish, stopped) = (sound.clone(), wanted.clone(), playing.clone());
        let is_playing = start.is_some();
        let button = cx.child(
            &IconButton::new(icon)
                .label(label)
                .style(if is_playing { ButtonStyle::Primary } else { ButtonStyle::Secondary })
                .disabled(waiting || !(has_clip || request.is_some()))
                .on_click(move || {
                    if is_playing {
                        stopper.stop_speaking();
                        stopped.set(None);
                    } else {
                        wish.set(true);
                        if !has_clip && let Some(request) = &request {
                            request();
                        }
                    }
                }),
        );

        let elapsed = start.map_or(0.0, |start| (now - start) as f32);
        let (track, fill) = (theme.color(Semantic::Border), theme.color(Semantic::Accent));
        let paint = match start {
            // Drawn without rebuilding; reduced motion stops the clock this reads.
            Some(start) if !cx.reduced_motion() => Paint::Live(Rc::new(move |frame, time, scene| {
                let fraction = ((time - start) as f32 / length).clamp(0.0, 1.0);
                draw_progress(frame, fraction, track, fill, scene);
            })),
            _ => {
                let fraction = if length > 0.0 { elapsed / length } else { 0.0 };
                Paint::Custom(Rc::new(move |frame, scene| draw_progress(frame, fraction, track, fill, scene)))
            }
        };
        let bar = cx.container(
            LayoutStyle {
                height: Some(4.0),
                grow: 1.0,
                min_width: Some(theme.spacing.s6),
                align_self: Some(Align::Center),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        cx.semantics(
            bar,
            Semantics::new(Role::ProgressIndicator).name(labels.audio_player.clone()).numeric(
                f64::from(elapsed),
                0.0,
                f64::from(length),
                None,
            ),
        );
        if let Some(start) = start {
            // The time on the label moves by the second, and the end stops it.
            let next_second = start + f64::from(elapsed.floor() + 1.0);
            cx.wake_at(next_second.min(start + f64::from(length)));
        }
        let says = if waiting {
            labels.audio_loading.clone()
        } else if start.is_some() {
            format!("{} / {}", clock_text(elapsed), clock_text(length))
        } else {
            self.length().map_or_else(String::new, clock_text)
        };
        let time = cx.child(&Text::new(says).style(TextStyle::Caption).foreground(Semantic::InkMuted));
        let player = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[button, bar, time],
            Paint::None,
        );
        let mut children = vec![player];
        children.extend(self.transcript_part(cx));
        cx.container(
            // Stretching only what asks: a button keeps the size of its words.
            LayoutStyle {
                spacing: theme.spacing.s1,
                align: Align::Start,
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}
