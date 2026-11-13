//! Folders and files, from whatever data the app has.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Modifiers, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use super::selection;
use super::virtual_list::{ScrollCommand, VirtualList};
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::files::size_text;
use crate::selection_mode::SelectionMode;
use crate::view::View;
use crate::{Breadcrumb, Divider, IconName, util};

/// What runs when an entry is opened.
type OpenHandler = Rc<dyn Fn(&FileEntry)>;

const ROW_HEIGHT: f32 = 32.0;
const SIZE_WIDTH: f32 = 96.0;
const MODIFIED_WIDTH: f32 = 148.0;

/// A folder or file of a [`FileBrowser`]. Nothing here touches the file system: it says what the
/// app knows, wherever that came from.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FileEntry {
    /// What the selection keeps. The name by default.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// It opens into more entries.
    pub is_folder: bool,
    /// How many bytes it has, if that is known.
    pub size: Option<u64>,
    /// When it changed, as it should read.
    pub modified: Option<String>,
    /// What sorts the dates, as a number that grows with time.
    pub modified_order: i64,
    /// An icon of its own, instead of the one for a folder or a file.
    pub icon: Option<IconName>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FileEntry {
    fn new(name: String, is_folder: bool, size: Option<u64>) -> Self {
        Self { id: name.clone(), name, is_folder, size, modified: None, modified_order: 0, icon: None }
    }

    /// A folder called `name`.
    pub fn folder(name: impl Into<String>) -> Self {
        Self::new(name.into(), true, None)
    }

    /// A file called `name`, of `size` bytes.
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Self::new(name.into(), false, Some(size))
    }

    /// What identifies it in the selection, when names can repeat.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// When it changed: how it reads, and a number that grows with time to sort it by.
    pub fn modified(mut self, text: impl Into<String>, order: i64) -> Self {
        self.modified = Some(text.into());
        self.modified_order = order;
        self
    }

    /// An icon of its own.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// What a [`FileBrowser`] sorts by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FileSortColumn {
    /// By name, ignoring case.
    #[default]
    Name,
    /// By size.
    Size,
    /// By date.
    Modified,
}

/// The order of a [`FileBrowser`]. Folders always come first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FileSort {
    /// What it sorts by.
    pub column: FileSortColumn,
    /// From the least to the most.
    pub ascending: bool,
}

impl Default for FileSort {
    fn default() -> Self {
        Self { column: FileSortColumn::Name, ascending: true }
    }
}

/// The positions of `entries` in this order, folders first.
fn sorted(entries: &[FileEntry], sort: FileSort) -> Vec<usize> {
    let names: Vec<String> = entries.iter().map(|entry| entry.name.to_lowercase()).collect();
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        let by = match sort.column {
            FileSortColumn::Name => names[a].cmp(&names[b]),
            FileSortColumn::Size => x.size.cmp(&y.size),
            FileSortColumn::Modified => x.modified_order.cmp(&y.modified_order),
        };
        let by = if sort.ascending { by } else { by.reverse() };
        y.is_folder.cmp(&x.is_folder).then(by).then_with(|| names[a].cmp(&names[b]))
    });
    order
}

/// An explorer of folders and files: a breadcrumb that goes back, a header that sorts, and a
/// list of any length with icon, name, size and date. The entries are data from the app, which
/// answers [`on_navigate`](Self::on_navigate) and [`on_open`](Self::on_open) with another
/// folder; it never touches the file system, so it serves a backup on a server as well as a disk.
/// A click selects alone, Ctrl or Cmd adds, and Shift takes a range. The arrows, Home and End
/// move, Space adds, Enter opens, and Backspace goes up.
///
/// ```
/// use ravenrs_widgets::{FileBrowser, FileEntry, Ui};
///
/// let browser = FileBrowser::new(
///     ["Backups", "2026"],
///     vec![FileEntry::folder("October"), FileEntry::file("notes.txt", 2048)],
/// )
/// .on_open(|entry| println!("{}", entry.name))
/// .on_navigate(|depth| println!("{depth}"));
/// let mut ui = Ui::new(browser, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FileBrowser {
    path: Vec<String>,
    entries: Rc<[FileEntry]>,
    selection: Option<Binding<BTreeSet<String>>>,
    sort: Option<Binding<FileSort>>,
    mode: SelectionMode,
    on_navigate: Option<Rc<dyn Fn(usize)>>,
    on_open: Option<OpenHandler>,
    height: Option<f32>,
    label: Option<String>,
}

impl fmt::Debug for FileBrowser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileBrowser").field("path", &self.path).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FileBrowser {
    /// The folder at `path`, from the root, with `entries` inside it. Keep the entries in an
    /// `Rc` between builds when they are many: they are sorted again only when they are not the
    /// same ones.
    pub fn new(
        path: impl IntoIterator<Item = impl Into<String>>,
        entries: impl Into<Rc<[FileEntry]>>,
    ) -> Self {
        Self {
            path: path.into_iter().map(Into::into).collect(),
            entries: entries.into(),
            selection: None,
            sort: None,
            mode: SelectionMode::Multiple,
            on_navigate: None,
            on_open: None,
            height: None,
            label: None,
        }
    }

    /// The ids of the selected entries, to read and to change from outside.
    pub fn selection(mut self, selection: Binding<BTreeSet<String>>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// The order, to keep or to set from outside.
    pub fn sort(mut self, sort: Binding<FileSort>) -> Self {
        self.sort = Some(sort);
        self
    }

    /// How entries can be selected ([`SelectionMode::Multiple`] by default).
    pub fn mode(mut self, mode: SelectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Called with the depth of the place to go back to, 0 being the root.
    pub fn on_navigate(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_navigate = Some(Rc::new(handler));
        self
    }

    /// Called with an entry on a double click or on Enter.
    pub fn on_open(mut self, handler: impl Fn(&FileEntry) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// A fixed height. It takes the height its parent gives it by default.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// What screen readers call it, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A column title that sorts.
struct SortHeader {
    text: String,
    column: FileSortColumn,
    sort: Binding<FileSort>,
    width: Option<f32>,
    end: bool,
}

impl View for SortHeader {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let now = self.sort.get();
        let on = now.column == self.column;
        let lit = cx.is_hovered() || cx.is_focus_visible();
        let color = theme.color(if on || lit { Semantic::Ink } else { Semantic::InkMuted });
        let mut parts = vec![util::label(cx, &self.text, TextStyle::Caption, Some(Weight::SEMIBOLD), color)];
        if on {
            let icon = if now.ascending { IconName::ArrowUp } else { IconName::ArrowDown };
            parts.push(util::icon(cx, icon, 12.0, color));
        }
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                grow: if self.width.is_none() { 1.0 } else { 0.0 },
                min_width: Some(0.0),
                spacing: theme.spacing.s1,
                align: Align::Center,
                justify: if self.end { Justify::End } else { Justify::Start },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::None,
        );
        let (sort, column) = (self.sort.clone(), self.column);
        cx.on_click(
            node,
            Rc::new(move || {
                sort.update(|s| {
                    s.ascending = if s.column == column { !s.ascending } else { true };
                    s.column = column;
                });
            }),
        );
        cx.semantics(node, Semantics::new(Role::ColumnHeader).name(self.text.clone()).selected(on));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// A line of the list.
struct FileRow {
    entry_index: usize,
    position: usize,
    entries: Rc<[FileEntry]>,
    selected: bool,
    active: bool,
    keyboard: bool,
    dates: bool,
    pick: Rc<dyn Fn(usize)>,
    open: Option<OpenHandler>,
}

impl View for FileRow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let Some(entry) = self.entries.get(self.entry_index) else { return cx.empty() };
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let icon = entry.icon.unwrap_or(if entry.is_folder { IconName::Folder } else { IconName::File });
        let tint = if entry.is_folder { theme.color(Semantic::Accent) } else { muted };
        let mut parts = vec![util::icon(cx, icon, 16.0, tint)];
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        parts.push(cx.text_truncated(
            &entry.name,
            &attrs,
            ink,
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        ));
        let size = entry.size.map(size_text).unwrap_or_default();
        let column = |width: f32, end: bool| LayoutStyle {
            width: Some(width),
            align: Align::Center,
            justify: if end { Justify::End } else { Justify::Start },
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let number = util::label(cx, &size, TextStyle::Caption, None, muted);
        parts.push(cx.container(column(SIZE_WIDTH, true), &[number], Paint::None));
        if self.dates {
            let date = entry.modified.as_deref().unwrap_or("");
            let date = util::label(cx, date, TextStyle::Caption, None, muted);
            parts.push(cx.container(column(MODIFIED_WIDTH, true), &[date], Paint::None));
        }
        let accent = theme.color(Semantic::Accent);
        let fill = if self.selected {
            accent.with_alpha(0.16)
        } else if cx.is_hovered() {
            ink.with_alpha(0.05)
        } else {
            Color::TRANSPARENT
        };
        let border = (self.active && self.keyboard).then_some(Border { width: 1.5, color: accent });
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                spacing: theme.spacing.s2,
                padding: Insets::symmetric(0.0, theme.spacing.s3),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, border, ..QuadPaint::new(fill) }),
        );
        let (pick, position) = (self.pick.clone(), self.position);
        cx.on_click(node, Rc::new(move || pick(position)));
        if let Some(open) = self.open.clone() {
            let (entries, index) = (self.entries.clone(), self.entry_index);
            cx.on_double_click(
                node,
                Rc::new(move || {
                    if let Some(entry) = entries.get(index) {
                        open(entry);
                    }
                }),
            );
        }
        cx.cursor(node, CursorIcon::Pointer);
        let what = if entry.is_folder { labels.folder_word.clone() } else { size };
        cx.semantics(
            node,
            Semantics::new(Role::ListBoxOption)
                .name(entry.name.clone())
                .description(what)
                .selected(self.selected)
                .active(self.active),
        );
        node
    }
}

impl View for FileBrowser {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let local_selection: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let selection = self.selection.clone().unwrap_or_else(|| local_selection.binding());
        let local_sort: State<FileSort> = cx.use_state(FileSort::default);
        let sort = self.sort.clone().unwrap_or_else(|| local_sort.binding());
        let commands: State<Option<ScrollCommand>> = cx.use_state(|| None);
        let cursor: State<usize> = cx.use_state(|| 0);
        let anchor: State<usize> = cx.use_state(|| 0);
        type Sorted = (Rc<[FileEntry]>, FileSort, Rc<Vec<usize>>);
        let cache: Rc<RefCell<Option<Sorted>>> = cx.use_state(|| Rc::new(RefCell::new(None))).peek().clone();
        let order = sort.get();
        let sorted_now = {
            let mut cached = cache.borrow_mut();
            match cached.as_ref() {
                Some((entries, by, found)) if Rc::ptr_eq(entries, &self.entries) && *by == order => {
                    found.clone()
                }
                _ => {
                    let found = Rc::new(sorted(&self.entries, order));
                    *cached = Some((self.entries.clone(), order, found.clone()));
                    found
                }
            }
        };

        let count = sorted_now.len();
        let active = cursor.get().min(count.saturating_sub(1));
        let selected = Rc::new(selection.get());
        let keyboard = cx.is_focus_visible();
        let mode = self.mode;
        let dates = self.entries.iter().any(|entry| entry.modified.is_some());
        let id_of = {
            let (entries, order) = (self.entries.clone(), sorted_now.clone());
            move |position: usize| {
                order.get(position).and_then(|i| entries.get(*i)).map_or_else(String::new, |e| e.id.clone())
            }
        };
        let modifiers = cx.modifiers();
        let apply: Rc<dyn Fn(usize, Modifiers)> = {
            let (selection, cursor, anchor, id_of) =
                (selection.clone(), cursor.clone(), anchor.clone(), id_of.clone());
            Rc::new(move |position, held| {
                let mut next = anchor.get();
                selection.update(|set| {
                    next = selection::pick(set, mode, next, position, held, &id_of);
                });
                anchor.set(next);
                cursor.set(position);
            })
        };
        let pick: Rc<dyn Fn(usize)> = {
            let apply = apply.clone();
            let modifiers = modifiers.clone();
            Rc::new(move |position| apply(position, modifiers.get()))
        };

        let mut crumbs = Breadcrumb::new(self.path.iter().cloned()).root_icon(IconName::House);
        if let Some(go) = self.on_navigate.clone() {
            crumbs = crumbs.on_navigate(move |depth| go(depth));
        }
        let crumbs = cx.child(&crumbs);
        let crumbs = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(theme.spacing.s2, theme.spacing.s3),
                align: Align::Start,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[crumbs],
            Paint::None,
        );
        let first_line = cx.child(&Divider::horizontal());
        let mut titles = vec![cx.child(&SortHeader {
            text: labels.file_name.clone(),
            column: FileSortColumn::Name,
            sort: sort.clone(),
            width: None,
            end: false,
        })];
        titles.push(cx.child(&SortHeader {
            text: labels.file_size.clone(),
            column: FileSortColumn::Size,
            sort: sort.clone(),
            width: Some(SIZE_WIDTH),
            end: true,
        }));
        if dates {
            titles.push(cx.child(&SortHeader {
                text: labels.file_modified.clone(),
                column: FileSortColumn::Modified,
                sort: sort.clone(),
                width: Some(MODIFIED_WIDTH),
                end: true,
            }));
        }
        let header = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                // The icon column of the rows, and their padding, line up the titles.
                padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s3),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &titles,
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        );
        let second_line = cx.child(&Divider::horizontal());

        let body = if count == 0 {
            let words = util::label(
                cx,
                &labels.folder_empty,
                TextStyle::Callout,
                None,
                theme.color(Semantic::InkMuted),
            );
            cx.container(
                LayoutStyle {
                    grow: 1.0,
                    min_height: Some(0.0),
                    padding: Insets::uniform(theme.spacing.s6),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[words],
                Paint::None,
            )
        } else {
            let (entries, order_now, open, shown, picker) = (
                self.entries.clone(),
                sorted_now.clone(),
                self.on_open.clone(),
                selected.clone(),
                pick.clone(),
            );
            let id = id_of.clone();
            let mut list = VirtualList::new(count, move |position| FileRow {
                entry_index: order_now.get(position).copied().unwrap_or(0),
                position,
                entries: entries.clone(),
                selected: shown.contains(&id(position)),
                active: position == active,
                keyboard,
                dates,
                pick: picker.clone(),
                open: open.clone(),
            })
            .row_height(ROW_HEIGHT)
            .bare()
            .commands(commands.clone());
            let (keyed_entries, keyed_order) = (self.entries.clone(), sorted_now.clone());
            list = list.key(move |position| {
                keyed_order
                    .get(position)
                    .and_then(|i| keyed_entries.get(*i))
                    .map_or(position as u64, |entry| util::hash_text(&entry.id))
            });
            cx.keyed_child(1, &list)
        };
        let root = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_some() { 0.0 } else { 1.0 },
                min_height: Some(0.0),
                min_width: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[crumbs, first_line, header, second_line, body],
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.file_browser_name.clone());
        cx.semantics(root, Semantics::new(Role::ListBox).name(name).description(count.to_string()));
        cx.focusable(root, theme.radii.sm);
        let (entries, order_now, open, up, depth) = (
            self.entries.clone(),
            sorted_now,
            self.on_open.clone(),
            self.on_navigate.clone(),
            self.path.len(),
        );
        let select_all = selection;
        cx.on_key(
            root,
            Rc::new(move |key, held| {
                let to = match key {
                    Key::ArrowUp => active.saturating_sub(1),
                    Key::ArrowDown => (active + 1).min(count.saturating_sub(1)),
                    Key::Home => 0,
                    Key::End => count.saturating_sub(1),
                    Key::Enter => {
                        if let (Some(open), Some(entry)) =
                            (&open, order_now.get(active).and_then(|i| entries.get(*i)))
                        {
                            open(entry);
                        }
                        return true;
                    }
                    Key::Space => {
                        if count > 0 {
                            apply(active, Modifiers { ctrl: true, ..Modifiers::default() });
                        }
                        return true;
                    }
                    Key::Backspace => {
                        if let (Some(up), true) = (&up, depth >= 2) {
                            up(depth - 2);
                        }
                        return true;
                    }
                    Key::Character(c) if selection::adds(held) && c.eq_ignore_ascii_case("a") => {
                        if mode != SelectionMode::Multiple {
                            return false;
                        }
                        select_all.set((0..count).map(&id_of).collect());
                        return true;
                    }
                    _ => return false,
                };
                if count == 0 {
                    return false;
                }
                if selection::adds(held) {
                    cursor.set(to);
                } else {
                    apply(to, held);
                }
                commands.set(Some(ScrollCommand::Reveal(to)));
                true
            }),
        );
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_stay_first_in_either_direction() {
        let entries = [
            FileEntry::file("b.txt", 10),
            FileEntry::folder("Z"),
            FileEntry::file("A.txt", 30),
            FileEntry::folder("c"),
        ];
        let by = |column, ascending| sorted(&entries, FileSort { column, ascending });
        assert_eq!(by(FileSortColumn::Name, true), vec![3, 1, 2, 0]);
        assert_eq!(by(FileSortColumn::Name, false), vec![1, 3, 0, 2]);
        assert_eq!(by(FileSortColumn::Size, false), vec![3, 1, 2, 0]);
    }
}
