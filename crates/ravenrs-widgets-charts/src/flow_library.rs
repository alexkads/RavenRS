//! `FlowLibrary`: the diagrams an app keeps, to make, save, open, rename and delete, beside a
//! `FlowCanvas`, and to write to or read from a JSON file.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Insets, MemoryStorage, Role, Storage};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::flow::{FlowDocument, FlowGraph, FlowHistory, FlowSelection, FlowShelf, FlowViewport};
use crate::{
    ButtonSize, ButtonStyle, ConfirmDialog, DropdownMenu, FileRequest, IconButton, IconName, ListItem,
    MenuItem, TextField, View, util,
};

/// Makes a diagram of a file's text.
type Reader = Rc<dyn Fn(&str) -> Result<FlowGraph, String>>;
/// Makes the text of a diagram, given its name.
type Writer = Rc<dyn Fn(&FlowGraph, &str) -> String>;

/// A file format a [`FlowLibrary`] opens and saves diagrams in besides its own JSON, such as
/// a notation's XML: what it is called, its extension, and how a diagram is read from it and
/// written to it.
#[derive(Clone)]
pub struct FlowFileFormat {
    name: String,
    extension: String,
    read: Reader,
    write: Writer,
}

impl fmt::Debug for FlowFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowFileFormat")
            .field("name", &self.name)
            .field("extension", &self.extension)
            .finish()
    }
}

impl FlowFileFormat {
    /// A format called `name`, in files ending in `.extension`: `read` makes a diagram of a
    /// file's text, or says why it cannot; `write` makes the text of a diagram, given its name.
    pub fn new(
        name: impl Into<String>,
        extension: impl Into<String>,
        read: impl Fn(&str) -> Result<FlowGraph, String> + 'static,
        write: impl Fn(&FlowGraph, &str) -> String + 'static,
    ) -> Self {
        Self { name: name.into(), extension: extension.into(), read: Rc::new(read), write: Rc::new(write) }
    }
}

/// The diagrams an app keeps, listed beside a [`FlowCanvas`](crate::FlowCanvas) that edits the
/// open one through the same `graph`.
///
/// New starts an empty diagram; Save keeps the open one under its name, which is typed above
/// the list; a diagram with changes not kept says so. Choosing another keeps the open one
/// first, so nothing typed is lost, and starts its history over. Delete asks first. A diagram
/// also goes to a JSON file and comes from one, through the platform's way of saving and
/// choosing files.
///
/// Diagrams are kept in a [`Storage`] under one name: a [`FileStorage`](ravenrs_core::FileStorage)
/// folder on the desktop, the browser's storage on the web; without one, they last as long as
/// the app runs.
///
/// ```
/// use std::rc::Rc;
/// use ravenrs_core::MemoryStorage;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FlowCanvas, FlowGraph, FlowHistory, FlowLibrary, HStack};
///
/// let (graph, history) = (State::new(FlowGraph::new()), State::new(FlowHistory::new()));
/// let editor = HStack::new((
///     FlowLibrary::new(graph.binding()).history(history.binding()).storage(Rc::new(MemoryStorage::default())),
///     FlowCanvas::new(graph.binding()).history(history.binding()),
/// ));
/// ```
pub struct FlowLibrary {
    graph: Binding<FlowGraph>,
    history: Option<Binding<FlowHistory>>,
    selection: Option<Binding<FlowSelection>>,
    view: Option<Binding<FlowViewport>>,
    storage: Option<Rc<dyn Storage>>,
    key: String,
    title: Option<String>,
    width: f32,
    formats: Vec<FlowFileFormat>,
}

impl fmt::Debug for FlowLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowLibrary").field("key", &self.key).finish_non_exhaustive()
    }
}

impl FlowLibrary {
    /// The diagrams kept for the canvas editing `graph`, 240 pixels wide, under the name
    /// `flows` of a storage that lasts while the app runs.
    pub fn new(graph: Binding<FlowGraph>) -> Self {
        Self {
            graph,
            history: None,
            selection: None,
            view: None,
            storage: None,
            key: "flows".into(),
            title: None,
            width: 240.0,
            formats: Vec::new(),
        }
    }

    /// The canvas's history, started over when another diagram opens.
    pub fn history(mut self, history: Binding<FlowHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// The canvas's selection, emptied when another diagram opens.
    pub fn selection(mut self, selection: Binding<FlowSelection>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// The canvas's view, which fits the whole diagram when another one opens.
    pub fn view(mut self, view: Binding<FlowViewport>) -> Self {
        self.view = Some(view);
        self
    }

    /// Keeps the diagrams here, so they are there the next time the app opens.
    pub fn storage(mut self, storage: Rc<dyn Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// The name the diagrams are kept under in the storage, for an app with more than one
    /// library.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// Another heading than "Diagrams".
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Another width, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The file formats of `notation`.
    pub fn notation(mut self, notation: &crate::FlowNotation) -> Self {
        self.formats.extend(notation.formats.iter().cloned());
        self
    }

    /// Also opens and saves files in `format`; Save as file then asks which.
    pub fn format(mut self, format: FlowFileFormat) -> Self {
        self.formats.push(format);
        self
    }
}

/// What the library knows between builds.
#[derive(Clone)]
struct Kept {
    shelf: State<FlowShelf>,
    /// The open document, as it was last kept (or made, before it is kept).
    open: State<FlowDocument>,
    /// Its name as typed.
    name: State<String>,
    /// Why the last reading or writing failed.
    trouble: State<Option<String>>,
}

/// How the library reaches the canvas and the storage.
#[derive(Clone)]
struct Shelf {
    graph: Binding<FlowGraph>,
    history: Option<Binding<FlowHistory>>,
    selection: Option<Binding<FlowSelection>>,
    view: Option<Binding<FlowViewport>>,
    storage: Rc<dyn Storage>,
    key: String,
    kept: Kept,
    formats: Rc<Vec<FlowFileFormat>>,
}

impl Shelf {
    fn write(&self, shelf: FlowShelf) {
        let result = self.storage.write(&self.key, shelf.to_json().as_bytes());
        self.kept.trouble.set_if_changed(result.err());
        self.kept.shelf.set(shelf);
    }

    /// The open document as it is now on the canvas, with its name as typed.
    fn current(&self) -> FlowDocument {
        let open = self.kept.open.get();
        FlowDocument { id: open.id, name: self.kept.name.get(), graph: self.graph.get() }
    }

    fn unsaved(&self) -> bool {
        let open = self.kept.open.get();
        let kept = self.kept.shelf.get().find(&open.id).is_some();
        !kept && !self.graph.get().nodes.is_empty() || kept && self.current() != open
    }

    /// Keeps the open document.
    fn save(&self) {
        let current = self.current();
        let mut shelf = self.kept.shelf.get();
        shelf.put(current.clone());
        self.write(shelf);
        self.kept.open.set(current);
    }

    /// Shows `document` on the canvas, from a fresh history.
    fn show(&self, document: FlowDocument) {
        self.graph.set(document.graph.clone());
        self.kept.name.set(document.name.clone());
        self.kept.open.set(document);
        if let Some(history) = &self.history {
            history.set(FlowHistory::new());
        }
        if let Some(selection) = &self.selection {
            selection.set(FlowSelection::default());
        }
        if let Some(view) = &self.view {
            view.set(FlowViewport::fit());
        }
    }

    /// Opens another document, keeping the open one first if it changed.
    fn open(&self, id: &str) {
        if self.kept.open.get().id == id {
            return;
        }
        if self.unsaved() {
            self.save();
        }
        if let Some(document) = self.kept.shelf.get().find(id).cloned() {
            self.show(document);
        }
    }

    /// Starts an empty document, keeping the open one first if it changed.
    fn make(&self, name: String) {
        if self.unsaved() {
            self.save();
        }
        let shelf = self.kept.shelf.get();
        let id = shelf.fresh_id();
        // "Untitled", then "Untitled 2", so the list tells them apart.
        let taken = |n: &str| shelf.documents.iter().any(|d| d.name == n) || self.kept.name.get() == n;
        let name = if taken(&name) {
            (2..).map(|i| format!("{name} {i}")).find(|n| !taken(n)).unwrap_or(name)
        } else {
            name
        };
        self.show(FlowDocument { id, name, graph: FlowGraph::new() });
    }

    /// Forgets a document; when it is the open one, the first left opens, or an empty one.
    fn delete(&self, id: &str, untitled: String) {
        let mut shelf = self.kept.shelf.get();
        shelf.remove(id);
        let first = shelf.documents.first().cloned();
        self.write(shelf);
        if self.kept.open.get().id == id {
            match first {
                Some(document) => self.show(document),
                None => {
                    let id = self.kept.shelf.get().fresh_id();
                    self.show(FlowDocument { id, name: untitled, graph: FlowGraph::new() });
                }
            }
        }
    }

    /// Opens a diagram read from a file, in the format its extension names (JSON otherwise),
    /// as a new document named after the file.
    fn import(&self, file_name: &str, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let format = self
            .formats
            .iter()
            .find(|f| file_name.to_lowercase().ends_with(&format!(".{}", f.extension.to_lowercase())));
        let read = match format {
            Some(format) => (format.read)(&text),
            None => FlowGraph::from_json(&text).map_err(|e| e.to_string()),
        };
        let stem = file_name.rsplit_once('.').map_or(file_name, |(stem, _)| stem).to_owned();
        match read {
            Ok(graph) => {
                if self.unsaved() {
                    self.save();
                }
                let name = stem;
                let id = self.kept.shelf.get().fresh_id();
                let document = FlowDocument { id, name, graph };
                let mut shelf = self.kept.shelf.get();
                shelf.put(document.clone());
                self.write(shelf);
                self.show(document);
            }
            Err(e) => self.kept.trouble.set(Some(e)),
        }
    }
}

/// A file name from a diagram's name, without what file systems refuse.
fn file_name(name: &str) -> String {
    file_named(name, "json")
}

/// A file name from a diagram's name and an extension.
fn file_named(name: &str, extension: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control())
        .collect();
    let kept = kept.trim();
    format!("{}.{extension}", if kept.is_empty() { "diagram" } else { kept })
}

impl View for FlowLibrary {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let storage =
            cx.use_state(|| self.storage.clone().unwrap_or_else(|| Rc::new(MemoryStorage::default())));
        let storage = storage.get();
        let key = self.key.clone();
        let graph_now = self.graph.get();
        let untitled = labels.flow_library_untitled.clone();
        let kept = {
            let read = storage.clone();
            let shelf = cx.use_state(move || {
                read.read(&key)
                    .ok()
                    .flatten()
                    .and_then(|bytes| FlowShelf::from_json(&String::from_utf8_lossy(&bytes)).ok())
                    .unwrap_or_default()
            });
            // The diagram on the canvas when the library first shows is a new document.
            let first = {
                let shelf = shelf.get();
                let id = shelf.fresh_id();
                FlowDocument { id, name: untitled.clone(), graph: graph_now.clone() }
            };
            let first_name = first.name.clone();
            Kept {
                shelf,
                open: cx.use_state(move || first),
                name: cx.use_state(move || first_name),
                trouble: cx.use_state(|| None::<String>),
            }
        };
        let confirm = cx.use_state(ConfirmDialog::new).get();
        let shelf = Shelf {
            graph: self.graph.clone(),
            history: self.history.clone(),
            selection: self.selection.clone(),
            view: self.view.clone(),
            storage,
            key: self.key.clone(),
            kept: kept.clone(),
            formats: Rc::new(self.formats.clone()),
        };
        let muted = theme.color(Semantic::InkMuted);
        let title = self.title.clone().unwrap_or_else(|| labels.flow_library.clone());
        let mut rows: Vec<NodeId> = Vec::new();

        let button = |cx: &mut BuildCx<'_>, icon: IconName, name: &str, run: Rc<dyn Fn()>| {
            cx.child(
                &IconButton::new(icon)
                    .label(name.to_owned())
                    .size(ButtonSize::Small)
                    .style(ButtonStyle::Ghost)
                    .on_click(move || run()),
            )
        };
        let heading = util::label(cx, &title, TextStyle::Caption, Some(Weight::SEMIBOLD), muted);
        let (s1, s2, s3, s4) = (shelf.clone(), shelf.clone(), shelf.clone(), shelf.clone());
        let new_name = untitled.clone();
        let make = button(
            cx,
            IconName::FilePlus,
            &labels.flow_library_new,
            Rc::new(move || s1.make(new_name.clone())),
        );
        let save = button(cx, IconName::Save, &labels.flow_library_save, Rc::new(move || s2.save()));
        let picker = cx.file_picker();
        let opener = picker.clone();
        let open_file = button(
            cx,
            IconName::FolderOpen,
            &labels.flow_library_open_file,
            Rc::new(move || {
                let s = s3.clone();
                let mut request = FileRequest::default();
                request.extensions = std::iter::once("json".to_owned())
                    .chain(s.formats.iter().map(|f| f.extension.clone()))
                    .collect();
                request.most_bytes = Some(20 << 20);
                opener.pick(&request, move |files| {
                    if let Some(file) = files.first() {
                        s.import(&file.name, &file.bytes);
                    }
                });
            }),
        );
        let save_file = if self.formats.is_empty() {
            button(
                cx,
                IconName::Upload,
                &labels.flow_library_save_file,
                Rc::new(move || {
                    let current = s4.current();
                    picker.save(&file_name(&current.name), current.graph.to_json().as_bytes());
                }),
            )
        } else {
            // Which format, in a menu: the library's own JSON first.
            let (s5, p5) = (s4.clone(), picker.clone());
            let mut menu = DropdownMenu::new(labels.flow_library_save_file.clone())
                .icon(IconName::Upload)
                .icon_only()
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .item(MenuItem::new("JSON").shortcut(".json").on_select(move || {
                    let current = s5.current();
                    p5.save(&file_name(&current.name), current.graph.to_json().as_bytes());
                }));
            for format in self.formats.iter().cloned() {
                let (s6, p6) = (s4.clone(), picker.clone());
                let aside = format!(".{}", format.extension);
                menu = menu.item(MenuItem::new(format.name.clone()).shortcut(aside).on_select(move || {
                    let current = s6.current();
                    let text = (format.write)(&current.graph, &current.name);
                    p6.save(&file_named(&current.name, &format.extension), text.as_bytes());
                }));
            }
            cx.child(&menu)
        };
        let spacer = cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None);
        rows.push(cx.container(
            LayoutStyle { align: Align::Center, spacing: 2.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[heading, spacer, make, save, open_file, save_file],
            Paint::None,
        ));
        rows.push(
            cx.child(&TextField::new(kept.name.binding()).placeholder(labels.flow_library_name.clone())),
        );
        let unsaved = shelf.unsaved();
        let status = if unsaved { &labels.flow_library_unsaved } else { &labels.flow_library_saved };
        let tone = if unsaved { theme.color(Semantic::Warning) } else { muted };
        rows.push(util::label(cx, status, TextStyle::Caption, None, tone));
        if let Some(trouble) = kept.trouble.get() {
            rows.push(util::label(cx, &trouble, TextStyle::Caption, None, theme.color(Semantic::Danger)));
        }

        // The documents kept, and the open one if it is not kept yet.
        let open_id = kept.open.get().id;
        let mut listed = kept.shelf.get().documents;
        if !listed.iter().any(|d| d.id == open_id) {
            listed.push(kept.open.get());
        }
        for document in listed {
            let is_open = document.id == open_id;
            let name = if is_open { kept.name.get() } else { document.name.clone() };
            let document_name = if name.trim().is_empty() { untitled.clone() } else { name.clone() };
            let count = labels.flow_library_count.replace("{0}", &document.graph.nodes.len().to_string());
            let (opened, id) = (shelf.clone(), document.id.clone());
            let item = ListItem::new(if name.trim().is_empty() { untitled.clone() } else { name.clone() })
                .icon(IconName::File)
                .subtitle(count)
                .selected(is_open)
                .on_click(move || opened.open(&id));
            let row = cx.keyed_child(util::hash_text(&format!("doc-{}", document.id)), &item);
            let (gone, id, ask, words) =
                (shelf.clone(), document.id.clone(), confirm.clone(), labels.clone());
            let fallback = untitled.clone();
            let delete = button(
                cx,
                IconName::Trash2,
                &format!("{} {}", labels.flow_library_delete, document_name),
                Rc::new(move || {
                    let (gone, id, fallback) = (gone.clone(), id.clone(), fallback.clone());
                    ask.ask(
                        words.flow_library_delete_title.replace("{0}", &name),
                        words.flow_library_delete_message.clone(),
                        words.flow_library_delete.clone(),
                        move || gone.delete(&id, fallback.clone()),
                    );
                }),
            );
            let row = cx.container(
                LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
                &[row],
                Paint::None,
            );
            rows.push(cx.container(
                LayoutStyle { align: Align::Center, spacing: 2.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &[row, delete],
                Paint::None,
            ));
        }
        rows.push(cx.child(&confirm));
        let root = cx.container(
            LayoutStyle {
                width: Some(self.width),
                padding: Insets::uniform(theme.spacing.s4),
                spacing: theme.spacing.s2,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::Quad(crate::context::QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.semantics(root, Semantics::new(Role::Group).name(title));
        root
    }
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn a_name_becomes_a_safe_file_name() {
        assert_eq!(file_name("Pedidos: maio/junho"), "Pedidos maiojunho.json");
        assert_eq!(file_name("  "), "diagram.json");
    }
}
