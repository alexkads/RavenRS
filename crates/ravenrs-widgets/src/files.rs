//! Files from outside the app: the service that brings them, a place to drop them, and an
//! image field.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, PathFill, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Button, ButtonSize, ButtonStyle, IconButton, IconName, View, util};

/// The largest file a [`FileUpload`] takes unless told otherwise: 100 MiB.
const DEFAULT_MOST_BYTES: u64 = 100 * 1024 * 1024;
/// The largest picture an [`ImageField`] takes unless told otherwise: 20 MiB.
#[cfg(feature = "decode")]
const DEFAULT_MOST_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// A file brought into the app, whole, in memory.
#[derive(Clone, PartialEq, Eq)]
pub struct PickedFile {
    /// Its name, with no folder. Never trust it as a path.
    pub name: String,
    /// Its size on disk. A file bigger than the request allowed comes with no bytes.
    pub size: u64,
    /// Its content.
    pub bytes: Rc<[u8]>,
}

impl PickedFile {
    /// A file with this name and content.
    pub fn new(name: impl Into<String>, bytes: impl Into<Rc<[u8]>>) -> Self {
        let bytes = bytes.into();
        Self { name: name.into(), size: bytes.len() as u64, bytes }
    }

    /// The part of the name after the last dot, in lower case.
    pub fn extension(&self) -> Option<String> {
        self.name.rsplit_once('.').map(|(_, extension)| extension.to_lowercase())
    }
}

impl fmt::Debug for PickedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PickedFile").field("name", &self.name).field("size", &self.size).finish()
    }
}

/// What to ask of a [`FilePicker`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileRequest {
    /// The extensions offered, such as `pdf`; empty offers every file.
    pub extensions: Vec<String>,
    /// Whether more than one can be chosen.
    pub multiple: bool,
    /// Files above this size are not read, and come with no bytes.
    pub most_bytes: Option<u64>,
}

/// Asks the person for files. Platform layers install one with [`crate::Ui::set_file_picker`].
pub trait FilePickerProvider {
    /// Shows the system's chooser, and calls `done` with what was chosen, now or later
    /// on the UI thread. Canceling gives an empty list.
    fn pick(&self, request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>);

    /// Hands a file over to the person: the system's dialog on a desktop, a download in a
    /// browser. Nothing happens where there is no system to ask.
    fn save(&self, _name: &str, _bytes: &[u8]) {}
}

/// A picker that chooses nothing, for where there is no system to ask (tests, headless use).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFilePicker;

impl FilePickerProvider for NoFilePicker {
    fn pick(&self, _request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>) {
        done(Vec::new());
    }
}

/// A shared handle to the file chooser, from [`crate::BuildCx::file_picker`].
#[derive(Clone)]
pub struct FilePicker(Rc<dyn FilePickerProvider>);

impl FilePicker {
    /// Wraps a provider.
    pub fn new(provider: impl FilePickerProvider + 'static) -> Self {
        Self(Rc::new(provider))
    }

    /// Asks for files; `done` runs with what was chosen.
    pub fn pick(&self, request: &FileRequest, done: impl FnOnce(Vec<PickedFile>) + 'static) {
        self.0.pick(request, Box::new(done));
    }

    /// Hands a file over to the person.
    pub fn save(&self, name: &str, bytes: &[u8]) {
        self.0.save(name, bytes);
    }
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new(NoFilePicker)
    }
}

impl fmt::Debug for FilePicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilePicker")
    }
}

/// A size as people read it.
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "";
    for name in UNITS {
        value /= 1024.0;
        unit = name;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

/// What a file field accepts, and why it would refuse a file.
#[derive(Clone, Debug)]
struct Rules {
    extensions: Vec<String>,
    most_bytes: u64,
    most_files: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Kind,
    Size,
    Count,
}

impl Rules {
    fn refusal(&self, file: &PickedFile) -> Option<Refusal> {
        if !self.extensions.is_empty() && !file.extension().is_some_and(|e| self.extensions.contains(&e)) {
            return Some(Refusal::Kind);
        }
        (file.size > self.most_bytes).then_some(Refusal::Size)
    }

    /// Splits what arrived: what stays with `kept`, and what was refused and why.
    fn take(&self, kept: &mut Vec<PickedFile>, arrived: Vec<PickedFile>) -> Vec<(String, Refusal)> {
        let mut refused = Vec::new();
        for file in arrived {
            if let Some(reason) = self.refusal(&file) {
                refused.push((file.name, reason));
            } else if let Some(same) = kept.iter_mut().find(|k| k.name == file.name) {
                // The same name again is a newer version of that file.
                *same = file;
            } else if self.most_files.is_some_and(|most| kept.len() >= most) {
                refused.push((file.name, Refusal::Count));
            } else {
                kept.push(file);
            }
        }
        refused
    }
}

fn refusal_text(labels: &crate::Labels, name: &str, reason: Refusal) -> String {
    let why = match reason {
        Refusal::Kind => &labels.file_wrong_kind,
        Refusal::Size => &labels.file_too_large,
        Refusal::Count => &labels.file_too_many,
    };
    format!("{name}: {why}")
}

fn normalized(extensions: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    extensions.into_iter().map(|e| e.into().trim_start_matches('.').to_lowercase()).collect()
}

/// What opens the system's chooser for these rules, and hands over what comes back.
fn chooser(cx: &BuildCx<'_>, rules: &Rules, on_files: Rc<dyn Fn(Vec<PickedFile>)>) -> Rc<dyn Fn()> {
    let (picker, waker) = (cx.file_picker(), cx.ui_waker());
    let request = FileRequest {
        extensions: rules.extensions.clone(),
        multiple: rules.most_files != Some(1),
        most_bytes: Some(rules.most_bytes),
    };
    Rc::new(move || {
        let (on_files, waker) = (on_files.clone(), waker.clone());
        picker.pick(&request, move |files| {
            on_files(files);
            waker.wake();
        });
    })
}

/// One line for each refused file, saying why.
fn push_refusals(cx: &mut BuildCx<'_>, refused: &[(String, Refusal)], rows: &mut Vec<NodeId>) {
    let (theme, labels) = (*cx.theme(), cx.labels());
    for (name, reason) in refused {
        let line = refusal_text(&labels, name, *reason);
        let text = util::label(cx, &line, TextStyle::Callout, None, theme.color(Semantic::Danger));
        cx.semantics(text, Semantics::new(Role::Alert).name(line));
        rows.push(text);
    }
}

/// How thick the dashes of a [`DropZone`] are, in logical pixels, at rest and with files over it.
const ZONE_STROKE: (f32, f32) = (1.0, 2.0);
const ZONE_DASH: f32 = 6.0;
const ZONE_GAP: f32 = 4.0;
/// How much of what is under the hint of a [`DropZone`] still shows.
const ZONE_HINT_OPACITY: f32 = 0.88;

/// A dashed area around any content, which takes files dropped on it and shows when they are
/// over the window. Unlike [`FileUpload`] it keeps no list: the files go to
/// [`DropZone::on_files`], and what it holds and how it looks are up to the content.
///
/// A dropped file lands in the zone under the pointer. While it is being dragged, every zone
/// on the screen lights up, because the platforms say only that files are over the window, and
/// not where. A keyboard cannot drop anything, so [`DropZone::choosable`] is there for that.
///
/// ```
/// use ravenrs_widgets::{DropZone, Text, Ui};
///
/// let zone = DropZone::new(Text::new("Photos of the session"))
///     .accept(["jpg", "png"])
///     .on_files(|files| println!("{} files", files.len()));
/// let mut ui = Ui::new(zone, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct DropZone<C> {
    content: C,
    rules: Rules,
    on_files: Option<Rc<dyn Fn(Vec<PickedFile>)>>,
    on_refused: Option<Rc<dyn Fn(Vec<String>)>>,
    label: Option<String>,
    hint: Option<String>,
    show_hint: bool,
    choosable: bool,
    quiet: bool,
    padding: Option<f32>,
    min_height: Option<f32>,
    fill: bool,
    disabled: bool,
}

impl<C> fmt::Debug for DropZone<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropZone").field("rules", &self.rules).finish_non_exhaustive()
    }
}

impl<C: View> DropZone<C> {
    /// A zone around `content`, taking any file.
    pub fn new(content: C) -> Self {
        Self {
            content,
            rules: Rules { extensions: Vec::new(), most_bytes: DEFAULT_MOST_BYTES, most_files: None },
            on_files: None,
            on_refused: None,
            label: None,
            hint: None,
            show_hint: true,
            choosable: false,
            quiet: false,
            padding: None,
            min_height: None,
            fill: false,
            disabled: false,
        }
    }

    /// Called with the files it took. It is not called when all of them were refused.
    pub fn on_files(mut self, handler: impl Fn(Vec<PickedFile>) + 'static) -> Self {
        self.on_files = Some(Rc::new(handler));
        self
    }

    /// Called with one sentence for each refused file, in the words of [`crate::Labels`], for
    /// showing them elsewhere. Without it they show inside the zone, under the content.
    pub fn on_refused(mut self, handler: impl Fn(Vec<String>) + 'static) -> Self {
        self.on_refused = Some(Rc::new(handler));
        self
    }

    /// Only these extensions, such as `["pdf", "png"]`. Any file by default.
    pub fn accept(mut self, extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.rules.extensions = normalized(extensions);
        self
    }

    /// The largest file taken, in bytes. 100 MiB by default.
    pub fn most_bytes(mut self, bytes: u64) -> Self {
        self.rules.most_bytes = bytes;
        self
    }

    /// The most files taken from one drop. No limit by default.
    pub fn most_files(mut self, files: usize) -> Self {
        self.rules.most_files = Some(files.max(1));
        self
    }

    /// The name screen readers announce. By default, the words of the hint.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What shows over the content while files are over the window. By default,
    /// [`file_drop`](crate::Labels::file_drop) of the labels.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Shows nothing over the content while files are over the window, only the stronger edge.
    pub fn no_hint(mut self) -> Self {
        self.show_hint = false;
        self
    }

    /// Makes the whole zone a button that opens the system's chooser, with a click, Enter or
    /// Space, so that the keyboard can bring files too. Leave it off when the content has
    /// its own way of choosing.
    pub fn choosable(mut self) -> Self {
        self.choosable = true;
        self
    }

    /// Draws the dashes only while files are over the window, for a zone that is a whole
    /// screen and should not look like one until then.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    /// The space around the content, in logical pixels (`space-4` by default).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }

    /// The shortest it gets, in logical pixels.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Takes the free space of the parent, for a zone that is a whole screen.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Turns it off: it takes no files and does not react to them.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// What a [`DropZone`] shows over its content while files are over the window.
fn zone_hint(cx: &mut BuildCx<'_>, words: &str, size: ravenrs_core::Size) -> NodeId {
    let theme = *cx.theme();
    let accent = theme.color(Semantic::Accent);
    let icon = util::icon(cx, IconName::CloudUpload, 22.0, accent);
    let text = util::label(cx, words, TextStyle::Callout, None, theme.color(Semantic::Ink));
    cx.semantics(text, Semantics::hidden());
    let fill = theme.color(Semantic::Surface);
    let node = cx.container(
        LayoutStyle {
            absolute: Some((0.0, 0.0)),
            width: Some(size.width),
            height: Some(size.height),
            spacing: theme.spacing.s2,
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &[icon, text],
        Paint::Quad(QuadPaint {
            corner_radius: theme.radii.md,
            ..QuadPaint::new(fill.with_alpha(fill.a * ZONE_HINT_OPACITY))
        }),
    );
    cx.semantics(node, Semantics::new(Role::Status).name(words));
    node
}

impl<C: View> View for DropZone<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let refused: State<Vec<(String, Refusal)>> = cx.use_state(Vec::new);
        let over = cx.files_over() && !self.disabled;
        let bring: Rc<dyn Fn(Vec<PickedFile>)> = {
            let (rules, shown) = (self.rules.clone(), refused.clone());
            let (on_files, on_refused) = (self.on_files.clone(), self.on_refused.clone());
            let labels = labels.clone();
            Rc::new(move |arrived| {
                if arrived.is_empty() {
                    return;
                }
                let mut kept = Vec::new();
                let out = rules.take(&mut kept, arrived);
                match &on_refused {
                    Some(tell) if !out.is_empty() => {
                        tell(out.iter().map(|(name, why)| refusal_text(&labels, name, *why)).collect());
                    }
                    Some(_) => {}
                    None => shown.set(out),
                }
                if let (Some(on_files), false) = (&on_files, kept.is_empty()) {
                    on_files(kept);
                }
            })
        };

        let mut rows = vec![cx.child(&self.content)];
        push_refusals(cx, &refused.get(), &mut rows);
        let words = self.hint.as_deref().unwrap_or(&labels.file_drop);
        if over && self.show_hint {
            let size = cx.scroll_viewport().unwrap_or(ravenrs_core::Size::ZERO);
            rows.push(zone_hint(cx, words, size));
        }

        let accent = theme.color(Semantic::Accent);
        let base = theme.color(Semantic::Border);
        let edge = if over {
            accent
        } else if self.disabled {
            util::faded(base)
        } else {
            base
        };
        let stroke = if over { ZONE_STROKE.1 } else { ZONE_STROKE.0 };
        let tint = over.then(|| accent.with_alpha(0.08));
        let dashed = over || !self.quiet;
        let radius = theme.radii.md;
        let root = cx.container(
            LayoutStyle {
                padding: Insets::uniform(self.padding.unwrap_or(theme.spacing.s4)),
                spacing: theme.spacing.s2,
                min_height: self.min_height.or(self.fill.then_some(0.0)),
                min_width: self.fill.then_some(0.0),
                grow: if self.fill { 1.0 } else { 0.0 },
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                if let Some(tint) = tint {
                    scene.push_quad(Quad::new(frame, tint).radius(radius));
                }
                if dashed {
                    let outline = util::dashed_outline(frame, radius, stroke, ZONE_DASH, ZONE_GAP);
                    scene.push_path(PathFill::solid(outline, edge));
                }
            })),
        );
        // The hint needs the size; and a drop looks for its place among what can be pointed at.
        cx.report_frame(root);
        cx.cursor(
            root,
            if self.choosable && !self.disabled { CursorIcon::Pointer } else { CursorIcon::Default },
        );
        let name = self.label.as_deref().unwrap_or(words);
        if self.disabled {
            cx.semantics(root, Semantics::new(Role::Group).name(name).disabled(true));
            return root;
        }
        if self.choosable {
            let choose = chooser(cx, &self.rules, bring.clone());
            cx.on_click(root, choose);
            cx.focusable(root, radius);
            cx.semantics(
                root,
                Semantics::new(Role::Button).name(name).description(labels.file_choose.clone()),
            );
        } else {
            cx.semantics(root, Semantics::new(Role::Group).name(name));
        }
        cx.on_file_drop(root, bring);
        root
    }
}

/// A place to bring files, with the system's chooser or by dropping them, and the list
/// of what came in. Refused files say why. The app reads [`PickedFile::bytes`], and
/// sending them somewhere is up to it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FileUpload, PickedFile, Ui};
///
/// let files = State::new(Vec::<PickedFile>::new());
/// let upload = FileUpload::new(files.binding()).accept(["pdf", "png"]).most_files(3);
/// let mut ui = Ui::new(upload, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct FileUpload {
    files: Binding<Vec<PickedFile>>,
    rules: Rules,
    width: Option<f32>,
    label: Option<String>,
    state: util::EditorState,
}

impl fmt::Debug for FileUpload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileUpload").field("rules", &self.rules).finish_non_exhaustive()
    }
}

impl FileUpload {
    /// A field bound to the files that came in.
    pub fn new(files: Binding<Vec<PickedFile>>) -> Self {
        Self {
            files,
            rules: Rules { extensions: Vec::new(), most_bytes: DEFAULT_MOST_BYTES, most_files: None },
            width: None,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// Only these extensions, such as `["pdf", "png"]`. Any file by default.
    pub fn accept(mut self, extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.rules.extensions = normalized(extensions);
        self
    }

    /// The largest file taken, in bytes. 100 MiB by default.
    pub fn most_bytes(mut self, bytes: u64) -> Self {
        self.rules.most_bytes = bytes;
        self
    }

    /// The most files kept at once. No limit by default.
    pub fn most_files(mut self, files: usize) -> Self {
        self.rules.most_files = Some(files.max(1));
        self
    }

    /// Width in logical pixels. It takes the width of its parent by default.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the files without letting them change.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

struct FileRow<'a> {
    file: &'a PickedFile,
    files: Option<Binding<Vec<PickedFile>>>,
}

impl View for FileRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let icon = util::icon(cx, IconName::File, 16.0, theme.color(Semantic::InkMuted));
        let name = cx.text_truncated(
            &self.file.name,
            &util::attrs(&theme, TextStyle::Body, None),
            theme.color(Semantic::Ink),
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        let size = util::label(
            cx,
            &size_text(self.file.size),
            TextStyle::Callout,
            None,
            theme.color(Semantic::InkMuted),
        );
        let mut parts = vec![icon, name, size];
        if let Some(files) = &self.files {
            let (files, gone) = (files.clone(), self.file.name.clone());
            let label = format!("{} {}", cx.labels().file_remove, self.file.name);
            parts.push(
                cx.child(
                    &IconButton::new(IconName::X)
                        .label(label)
                        .size(ButtonSize::Small)
                        .on_click(move || files.update(|all| all.retain(|f| f.name != gone))),
                ),
            );
        }
        let node = cx.container(
            LayoutStyle {
                min_height: Some(32.0),
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::ListItem).name(self.file.name.clone()));
        node
    }
}

impl View for FileUpload {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let state = self.state;
        let refused: State<Vec<(String, Refusal)>> = cx.use_state(Vec::new);
        let bring: Rc<dyn Fn(Vec<PickedFile>)> = {
            let (files, rules, refused) = (self.files.clone(), self.rules.clone(), refused.clone());
            Rc::new(move |arrived| {
                if arrived.is_empty() {
                    return;
                }
                let mut kept = files.get();
                refused.set(rules.take(&mut kept, arrived));
                files.set(kept);
            })
        };
        let mut rows = Vec::new();
        if !state.read_only {
            let over = cx.files_over() && state.accepts_input();
            let muted = state.ink(theme.color(Semantic::InkMuted));
            let icon = util::icon(cx, IconName::CloudUpload, 22.0, muted);
            let hint = util::label(cx, &labels.file_drop, TextStyle::Callout, None, muted);
            let choose = chooser(cx, &self.rules, bring.clone());
            let mut button = Button::new(labels.file_choose.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .disabled(state.disabled);
            if state.accepts_input() {
                button = button.on_click(move || choose());
            }
            let button = cx.child(&button);
            let mut hints = vec![hint];
            if !self.rules.extensions.is_empty() {
                let kinds = self.rules.extensions.join(", ");
                hints.push(util::label(cx, &kinds, TextStyle::Caption, None, muted));
            }
            let words = cx.container(
                LayoutStyle { spacing: 2.0, align: Align::Center, ..LayoutStyle::stack(Axis::Vertical) },
                &hints,
                Paint::None,
            );
            let accent = theme.color(Semantic::Accent);
            let edge = if over { accent } else { state.ink(theme.color(Semantic::Border)) };
            let zone = cx.container(
                LayoutStyle {
                    padding: Insets::uniform(16.0),
                    spacing: 8.0,
                    align: Align::Center,
                    justify: Justify::Center,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[icon, words, button],
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.md,
                    border: Some(Border { width: if over { 1.5 } else { 1.0 }, color: edge }),
                    ..QuadPaint::new(if over { accent.with_alpha(0.08) } else { Color::TRANSPARENT })
                }),
            );
            if state.accepts_input() {
                cx.on_file_drop(zone, bring.clone());
            }
            rows.push(zone);
        }
        push_refusals(cx, &refused.get(), &mut rows);
        let files = self.files.get();
        let can_remove = state.accepts_input();
        for file in &files {
            let row = FileRow { file, files: can_remove.then(|| self.files.clone()) };
            rows.push(cx.keyed_child(util::hash_text(&file.name), &row));
        }
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                spacing: 8.0,
                align: Align::Stretch,
                align_self: Some(if self.width.is_some() { Align::Fit } else { Align::Stretch }),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::None,
        );
        let mut semantics = Semantics::new(Role::List);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        node
    }
}

/// A picture chosen by the person: what it looks like now, and buttons to change it or
/// take it off. Takes PNG and JPEG, and refuses what it cannot show.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ImageField, PickedFile, Ui};
///
/// let photo = State::new(None::<PickedFile>);
/// let field = ImageField::new(photo.binding()).size(160.0, 120.0);
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg(feature = "decode")]
pub struct ImageField {
    value: Binding<Option<PickedFile>>,
    most_bytes: u64,
    width: f32,
    height: f32,
    label: Option<String>,
    state: util::EditorState,
}

#[cfg(feature = "decode")]
impl fmt::Debug for ImageField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageField").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg(feature = "decode")]
impl ImageField {
    /// A field bound to the picture's file.
    pub fn new(value: Binding<Option<PickedFile>>) -> Self {
        Self {
            value,
            most_bytes: DEFAULT_MOST_IMAGE_BYTES,
            width: 160.0,
            height: 120.0,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// The largest file taken, in bytes. 20 MiB by default.
    pub fn most_bytes(mut self, bytes: u64) -> Self {
        self.most_bytes = bytes;
        self
    }

    /// The size of the picture in logical pixels. 160 by 120 by default.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// The name screen readers announce, and the picture's alternative text.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the picture without letting it change.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

/// A decoded picture, kept so a rebuild does not decode it again.
#[cfg(feature = "decode")]
#[derive(Clone)]
struct Decoded {
    source: Rc<[u8]>,
    image: Result<ravenrs_core::ImageData, String>,
}

#[cfg(feature = "decode")]
impl View for ImageField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let state = self.state;
        let refused: State<Option<String>> = cx.use_state(|| None);
        let decoded: State<Option<Decoded>> = cx.use_state(|| None);
        let current = self.value.get();
        let image = current.as_ref().and_then(|file| {
            let cached = decoded.peek().clone().filter(|d| Rc::ptr_eq(&d.source, &file.bytes));
            let entry = cached.unwrap_or_else(|| {
                let image = crate::image::decode_image(&file.bytes).map_err(|e| e.to_string());
                let entry = Decoded { source: file.bytes.clone(), image };
                decoded.set(Some(entry.clone()));
                entry
            });
            entry.image.ok()
        });
        let picture = match (&image, &current) {
            (Some(data), _) => {
                let mut picture = crate::Image::new(data.clone())
                    .content_mode(crate::ContentMode::Fit)
                    .size(self.width, self.height)
                    .corner_radius(theme.radii.md);
                if let Some(label) = &self.label {
                    picture = picture.alt(label.clone());
                }
                cx.child(&picture)
            }
            (None, broken) => {
                let muted = state.ink(theme.color(Semantic::InkMuted));
                let icon = util::icon(cx, IconName::Image, 28.0, muted);
                let mut parts = vec![icon];
                if broken.is_some() {
                    parts.push(
                        cx.child(&crate::Text::new(labels.image_invalid.clone()).style(TextStyle::Caption)),
                    );
                }
                let node = cx.container(
                    LayoutStyle {
                        width: Some(self.width),
                        height: Some(self.height),
                        spacing: 4.0,
                        align: Align::Center,
                        justify: Justify::Center,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &parts,
                    Paint::Quad(QuadPaint {
                        corner_radius: theme.radii.md,
                        border: Some(Border { width: 1.0, color: state.ink(theme.color(Semantic::Border)) }),
                        ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
                    }),
                );
                let name = self.label.clone().unwrap_or_else(|| labels.image_none.clone());
                cx.semantics(node, Semantics::new(Role::Image).name(name));
                node
            }
        };
        let mut rows = vec![picture];
        let rules = Rules {
            extensions: normalized(["png", "jpg", "jpeg"]),
            most_bytes: self.most_bytes,
            most_files: Some(1),
        };
        let bring: Rc<dyn Fn(Vec<PickedFile>)> = {
            let (value, rules, refused, words) =
                (self.value.clone(), rules.clone(), refused.clone(), labels.clone());
            Rc::new(move |arrived| {
                let Some(file) = arrived.into_iter().next() else { return };
                let bad = rules.refusal(&file).map(|why| refusal_text(&words, &file.name, why));
                refused.set(bad.clone());
                if bad.is_none() {
                    value.set(Some(file));
                }
            })
        };
        if state.accepts_input() {
            cx.on_file_drop(picture, bring.clone());
        }
        if !state.read_only {
            let (picker, on_files, waker) = (cx.file_picker(), bring, cx.ui_waker());
            let request = FileRequest {
                extensions: rules.extensions,
                multiple: false,
                most_bytes: Some(self.most_bytes),
            };
            let text = if current.is_some() { &labels.image_replace } else { &labels.image_choose };
            let mut button = Button::new(text.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .disabled(state.disabled);
            if state.accepts_input() {
                button = button.on_click(move || {
                    let (on_files, waker) = (on_files.clone(), waker.clone());
                    picker.pick(&request, move |files| {
                        on_files(files);
                        waker.wake();
                    });
                });
            }
            let mut buttons = vec![cx.child(&button)];
            if current.is_some() && state.accepts_input() {
                let (value, refused) = (self.value.clone(), refused.clone());
                buttons.push(
                    cx.child(
                        &Button::new(labels.file_remove.clone())
                            .style(ButtonStyle::Ghost)
                            .size(ButtonSize::Small)
                            .on_click(move || {
                                refused.set(None);
                                value.set(None);
                            }),
                    ),
                );
            }
            rows.push(cx.container(
                LayoutStyle { spacing: 4.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &buttons,
                Paint::None,
            ));
        }
        if let Some(line) = refused.get() {
            let text = util::label(cx, &line, TextStyle::Callout, None, theme.color(Semantic::Danger));
            cx.semantics(text, Semantics::new(Role::Alert).name(line));
            rows.push(text);
        }
        cx.container(
            LayoutStyle { spacing: 8.0, align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: usize) -> PickedFile {
        PickedFile::new(name, vec![0u8; size])
    }

    #[test]
    fn sizes_read_like_people_say_them() {
        assert_eq!(size_text(0), "0 B");
        assert_eq!(size_text(1023), "1023 B");
        assert_eq!(size_text(1536), "1.5 KB");
        assert_eq!(size_text(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn extensions_ignore_case_and_need_a_dot() {
        assert_eq!(file("Foto.PNG", 0).extension().as_deref(), Some("png"));
        assert_eq!(file("sem-extensao", 0).extension(), None);
    }

    #[test]
    fn what_arrives_is_kept_or_refused_with_a_reason() {
        let rules = Rules { extensions: normalized([".PDF", "png"]), most_bytes: 10, most_files: Some(2) };
        let mut kept = vec![];
        let refused = rules.take(
            &mut kept,
            vec![file("a.pdf", 1), file("b.exe", 1), file("c.png", 11), file("d.png", 1), file("e.png", 1)],
        );
        assert_eq!(kept.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["a.pdf", "d.png"]);
        assert_eq!(
            refused,
            [
                ("b.exe".to_owned(), Refusal::Kind),
                ("c.png".to_owned(), Refusal::Size),
                ("e.png".to_owned(), Refusal::Count),
            ]
        );
    }

    #[test]
    fn the_same_name_again_replaces_the_file() {
        let rules = Rules { extensions: vec![], most_bytes: 100, most_files: Some(1) };
        let mut kept = vec![file("a.pdf", 1)];
        assert!(rules.take(&mut kept, vec![file("a.pdf", 2)]).is_empty(), "takes no more room");
        assert_eq!(kept[0].size, 2);
    }
}
