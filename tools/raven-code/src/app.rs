//! The window: the files and the search at the left, the open files in the middle, the
//! terminal and the problems below, a toolbar and a status bar. It holds the state and wires
//! the [`project`](crate::project), [`cargo`](crate::cargo), [`lsp`](crate::lsp) and
//! [`config`](crate::config) parts to the components; what they do is in them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ravenrs::core::Key;
use ravenrs::prelude::*;
use ravenrs::widgets::{CodeCompletion, Cursor, TextHistory};

use crate::cargo::{self, Control, Diagnostic, Level, Report, Task};
use crate::config::{Config, Session};
use crate::lsp::Lsp;
use crate::project::{self, Entry, Hit, Project};

/// The panels that are always there.
const EXPLORER: &str = "explorer";
const SEARCH: &str = "search";
const TERMINAL: &str = "terminal";
const PROBLEMS: &str = "problems";

/// How often the watcher looks at the disk, in milliseconds.
const WATCH_EVERY: u64 = 1500;

/// An open file: its text, the text on disk, where its caret is, and what can be undone.
#[derive(Clone)]
struct Document {
    text: State<String>,
    saved: State<String>,
    caret: State<Cursor>,
    history: State<TextHistory>,
    /// Turned on to give the editor the keyboard, as going to a place does.
    focus: State<bool>,
    /// The text the language server was last told.
    told: Rc<RefCell<String>>,
    /// Changed on disk while it had changes here: the bar asks which to keep.
    on_disk: State<Option<String>>,
}

impl Document {
    fn new(text: String) -> Self {
        Self {
            text: State::new(text.clone()),
            saved: State::new(text.clone()),
            caret: State::new(Cursor::default()),
            history: State::new(TextHistory::new()),
            focus: State::new(false),
            told: Rc::new(RefCell::new(text)),
            on_disk: State::new(None),
        }
    }

    fn unsaved(&self) -> bool {
        self.text.get() != self.saved.get()
    }
}

/// What to do once a question about unsaved changes is answered.
#[derive(Clone, Debug, PartialEq)]
enum Then {
    /// Close the window.
    Quit,
    /// Open another project.
    Open(PathBuf),
    /// Close one document.
    CloseDocument(String),
}

/// What a name typed in the dialog is for.
#[derive(Clone, Debug, PartialEq)]
enum Naming {
    /// A new file in this folder ("" for the top).
    File(String),
    /// A new folder in this folder.
    Folder(String),
    /// A new name for this file or folder.
    Rename(String),
}

/// The dialog open over the window, if one is.
#[derive(Clone)]
enum Asking {
    /// What to do with the unsaved files, before `then`.
    Unsaved(Then),
    /// A name, with the field it is typed in and why it was refused.
    Name { naming: Naming, typed: State<String>, refused: State<Option<String>> },
    /// Whether to delete a file or a folder for good.
    Delete(String),
}

/// What changes with the project: everything but the window and what is remembered.
struct Workspace {
    project: Project,
    tree: Vec<Entry>,
    /// The paths open, which the panels are built from, in the order of their tabs.
    documents: BTreeMap<String, Document>,
    open: Vec<String>,
    lsp: Option<Lsp>,
    /// A number the watcher writes when the disk changed, and the one the screen last read.
    stamp: Arc<AtomicU64>,
    seen: u64,
    /// Turned on to stop the watcher when the project changes.
    stop_watching: Arc<AtomicBool>,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.stop_watching.store(true, Ordering::Relaxed);
    }
}

/// What the window keeps.
struct Inner {
    config: RefCell<Option<Config>>,
    work: RefCell<Workspace>,
    /// Bumped when the workspace changes, so the screen builds again.
    version: State<u64>,
    layout: State<DockLayout>,
    selected: State<Option<String>>,
    /// What cargo said, written by its thread.
    report: Arc<Mutex<Report>>,
    control: Control,
    /// A copy of it, read by the build, so that a change shows.
    shown: State<Report>,
    terminal: State<String>,
    /// The line typed in the terminal, and the lines sent before.
    typed: State<String>,
    sent: State<Vec<String>>,
    /// The binaries of the project, once cargo was asked, and the one Run runs.
    binaries: State<Option<Vec<String>>>,
    binary: State<usize>,
    waker: RefCell<Option<UiWaker>>,
    /// Whether rust-analyzer is there, or why not.
    lsp_note: State<Option<String>>,
    /// A line for the status bar, when something did not work.
    trouble: State<Option<String>>,
    asking: State<Option<Asking>>,
    closer: RefCell<Option<WindowCloser>>,
    search: State<String>,
    search_case: State<bool>,
    hits: State<Vec<Hit>>,
    /// What the editor in front last asked the language server for.
    tip: State<Option<String>>,
    offered: State<Vec<CodeCompletion>>,
}

/// Raven Code on a project.
#[derive(Clone)]
pub struct RavenCode(Rc<Inner>);

impl std::fmt::Debug for RavenCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RavenCode").field("project", &self.0.work.borrow().project).finish_non_exhaustive()
    }
}

fn default_layout() -> DockLayout {
    DockLayout::new()
        .left(DockNode::tabs([EXPLORER, SEARCH]), 260.0)
        .bottom(DockNode::tabs([TERMINAL, PROBLEMS]), 220.0)
}

fn workspace(project: Project) -> Workspace {
    let tree = project.tree().unwrap_or_default();
    let stamp = project.stamp();
    Workspace {
        project,
        tree,
        documents: BTreeMap::new(),
        open: Vec::new(),
        lsp: None,
        stamp: Arc::new(AtomicU64::new(stamp)),
        seen: stamp,
        stop_watching: Arc::new(AtomicBool::new(false)),
    }
}

impl RavenCode {
    /// Raven Code on `project`, with `src/main.rs` open when there is one. It remembers nothing
    /// and starts no language server; [`remembering`](Self::remembering) and
    /// [`with_language_server`](Self::with_language_server) add them.
    pub fn new(project: Project) -> Self {
        let app = Self(Rc::new(Inner {
            config: RefCell::new(None),
            work: RefCell::new(workspace(project)),
            version: State::new(0),
            layout: State::new(default_layout()),
            selected: State::new(None),
            report: Arc::new(Mutex::new(Report::default())),
            control: Control::default(),
            shown: State::new(Report::default()),
            terminal: State::new(String::new()),
            typed: State::new(String::new()),
            sent: State::new(Vec::new()),
            binaries: State::new(None),
            binary: State::new(0),
            waker: RefCell::new(None),
            lsp_note: State::new(None),
            trouble: State::new(None),
            asking: State::new(None),
            closer: RefCell::new(None),
            search: State::new(String::new()),
            search_case: State::new(false),
            hits: State::new(Vec::new()),
            tip: State::new(None),
            offered: State::new(Vec::new()),
        }));
        app.open_file("src/main.rs");
        app
    }

    /// Remembers the project among the recent ones, and how it is left, in `config`; a session
    /// kept there before is put back now.
    pub fn remembering(self, config: Config) -> Self {
        let root = self.root();
        let _ = config.remember(&root);
        if let Some(session) = config.session(&root) {
            self.restore(&session);
        }
        *self.0.config.borrow_mut() = Some(config);
        self
    }

    /// Starts rust-analyzer for the project, for what is wrong while typing, suggestions, tips
    /// and going to definitions. Without it, the status bar says so and the rest works.
    pub fn with_language_server(self) -> Self {
        self.start_language_server();
        self
    }

    /// Talks to a language server through these streams instead, for tests.
    pub fn with_language_server_on(
        self,
        reader: impl std::io::Read + Send + 'static,
        writer: impl std::io::Write + Send + 'static,
    ) -> Self {
        let lsp = Lsp::connect(reader, writer, &self.root(), self.wake());
        self.attach(lsp);
        self
    }

    fn start_language_server(&self) {
        match Lsp::start(&self.root(), self.wake()) {
            Ok(lsp) => self.attach(lsp),
            Err(_) => self.0.lsp_note.set(Some("rust-analyzer not found".to_owned())),
        }
    }

    fn attach(&self, lsp: Lsp) {
        let mut work = self.0.work.borrow_mut();
        for (path, document) in &work.documents {
            lsp.open(path, &document.text.get());
        }
        work.lsp = Some(lsp);
        self.0.lsp_note.set(None);
    }

    /// A call that asks for a rebuild, from any thread.
    fn wake(&self) -> impl Fn() + Send + Sync + 'static {
        let waker = self.0.waker.borrow().clone();
        move || {
            if let Some(waker) = &waker {
                waker.wake();
            }
        }
    }

    /// The project's folder.
    pub fn root(&self) -> PathBuf {
        self.0.work.borrow().project.root().to_path_buf()
    }

    /// The project's name.
    pub fn name(&self) -> String {
        self.0.work.borrow().project.name()
    }

    /// The layout of the window, to save or to check.
    pub fn layout(&self) -> DockLayout {
        self.0.layout.get()
    }

    /// The files open, in the order of their tabs.
    pub fn open_files(&self) -> Vec<String> {
        self.0.work.borrow().open.clone()
    }

    /// The text of an open file.
    pub fn text_of(&self, path: &str) -> Option<String> {
        self.document(path).map(|document| document.text.get())
    }

    fn document(&self, path: &str) -> Option<Document> {
        self.0.work.borrow().documents.get(path).cloned()
    }

    /// Changes the text of an open file, as typing would.
    pub fn edit(&self, path: &str, text: &str) {
        if let Some(document) = self.document(path) {
            document.text.set(text.to_owned());
        }
    }

    /// Whether an open file has changes that are not on disk.
    pub fn is_unsaved(&self, path: &str) -> bool {
        self.document(path).is_some_and(|document| document.unsaved())
    }

    /// The open files with changes that are not on disk.
    pub fn unsaved(&self) -> Vec<String> {
        let work = self.0.work.borrow();
        work.open.iter().filter(|p| work.documents.get(*p).is_some_and(Document::unsaved)).cloned().collect()
    }

    /// Where the caret of an open file is, as a character index.
    pub fn caret_of(&self, path: &str) -> Option<usize> {
        self.document(path).map(|document| document.caret.get().caret)
    }

    /// Opens a file of the project, or brings it to the front when it is open.
    pub fn open_file(&self, path: &str) {
        let inner = &self.0;
        if self.document(path).is_none() {
            let text = match inner.work.borrow().project.read(path) {
                Ok(text) => text,
                Err(error) => {
                    inner.trouble.set(Some(format!("{path}: {error}")));
                    return;
                }
            };
            let mut work = inner.work.borrow_mut();
            if let Some(lsp) = &work.lsp {
                lsp.open(path, &text);
            }
            work.documents.insert(path.to_owned(), Document::new(text));
            work.open.push(path.to_owned());
            drop(work);
            inner.version.update(|v| *v += 1);
        }
        inner.layout.update(|layout| {
            if layout.contains(path) {
                layout.activate(path);
            } else {
                // A panel the arrangement does not know yet goes in as closed, then back in the center.
                *layout = layout.clone().closed([path], DockSide::Center);
                layout.reopen(path);
            }
        });
        inner.selected.set(Some(path.to_owned()));
    }

    /// Closes an open file, without asking: what is not saved is lost.
    pub fn close_file(&self, path: &str) {
        let inner = &self.0;
        let mut work = inner.work.borrow_mut();
        if work.documents.remove(path).is_none() {
            return;
        }
        work.open.retain(|p| p != path);
        if let Some(lsp) = &work.lsp {
            lsp.close(path);
        }
        drop(work);
        inner.layout.update(|layout| {
            layout.close(path);
        });
        inner.version.update(|v| *v += 1);
        self.keep_session();
    }

    /// Writes an open file to disk. False when it could not.
    pub fn save(&self, path: &str) -> bool {
        let Some(document) = self.document(path) else { return true };
        let text = document.text.get();
        let written = self.0.work.borrow().project.write(path, &text);
        match written {
            Ok(()) => {
                document.saved.set(text);
                document.on_disk.set(None);
                true
            }
            Err(error) => {
                self.0.trouble.set(Some(format!("{path}: {error}")));
                false
            }
        }
    }

    /// Writes every open file with changes. False when one could not be written.
    pub fn save_all(&self) -> bool {
        // Every file is tried, even after one fails.
        let mut all = true;
        for path in self.unsaved() {
            all &= self.save(&path);
        }
        all
    }

    /// The open file at the front, if one is.
    pub fn active(&self) -> Option<String> {
        let layout = self.0.layout.get();
        let work = self.0.work.borrow();
        layout
            .panels()
            .into_iter()
            .find(|id| work.documents.contains_key(*id) && layout.is_active(id))
            .map(str::to_owned)
    }

    /// Saves what changed and starts cargo. What it says shows as it comes.
    pub fn run(&self, task: Task) {
        let inner = &self.0;
        let mut bin = None;
        if task == Task::Run {
            let binaries = self.binaries();
            if binaries.is_empty() {
                inner.trouble.set(Some("No binary to run".to_owned()));
                return;
            }
            bin = binaries.get(inner.binary.get()).cloned().filter(|_| binaries.len() > 1);
        }
        inner.trouble.set(None);
        self.save_all();
        cargo::start(self.root(), task, bin, inner.report.clone(), inner.control.clone(), self.wake());
        inner.layout.update(|layout| {
            layout.activate(TERMINAL);
        });
    }

    /// The binaries of the project, asking cargo the first time.
    pub fn binaries(&self) -> Vec<String> {
        if self.0.binaries.peek().is_none() {
            self.0.binaries.set(Some(cargo::binaries(&self.root()).unwrap_or_default()));
        }
        self.0.binaries.get().unwrap_or_default()
    }

    /// Sets the binaries of the project, as cargo would say them, for tests.
    pub fn set_binaries(&self, names: &[&str]) {
        self.0.binaries.set(Some(names.iter().map(|n| (*n).to_owned()).collect()));
    }

    /// The files and folders of the project, as the Explorer shows them.
    pub fn tree(&self) -> Vec<Entry> {
        self.0.work.borrow().tree.clone()
    }

    /// Stops what cargo runs. False when nothing runs.
    pub fn stop(&self) -> bool {
        self.0.control.stop()
    }

    /// Sends a line to the program that runs, as typed in the terminal.
    pub fn send_line(&self, line: &str) -> bool {
        let sent = self.0.control.send(line);
        if sent && let Ok(mut report) = self.0.report.lock() {
            report.output.push_str(line);
            report.output.push('\n');
        }
        self.wake()();
        sent
    }

    /// What cargo has said so far.
    pub fn report(&self) -> Report {
        self.0.report.lock().map(|report| report.clone()).unwrap_or_default()
    }

    /// Takes in what cargo said, as its thread would, for tests.
    pub fn take_output(&self, lines: &[&str], success: bool) {
        if let Ok(mut report) = self.0.report.lock() {
            for line in lines {
                report.take(line);
            }
            report.running = false;
            report.success = Some(success);
        }
        self.wake()();
    }

    /// Opens the file of a diagnostic with the caret on it.
    pub fn go_to(&self, diagnostic: &Diagnostic) {
        self.go_to_place(&diagnostic.file, diagnostic.line, diagnostic.column);
    }

    /// Opens a file with the caret at a line and a column, both from 1.
    pub fn go_to_place(&self, file: &str, line: usize, column: usize) {
        self.open_file(file);
        if let Some(document) = self.document(file) {
            let at = offset_of(&document.text.get(), line, column);
            document.caret.set(Cursor::at(at));
            document.focus.set(true);
        }
    }

    /// Looks for `query` in every file of the project; the hits show in the Search panel.
    pub fn search(&self, query: &str) {
        self.0.search.set(query.to_owned());
        let hits = self.0.work.borrow().project.search(query, self.0.search_case.get());
        self.0.hits.set(hits);
    }

    /// What the last search found.
    pub fn hits(&self) -> Vec<Hit> {
        self.0.hits.get()
    }

    /// Opens another project in this window, as "Open Folder…" does once a folder is chosen.
    /// It asks first when files have changes.
    pub fn open_project(&self, folder: PathBuf) {
        if !self.unsaved().is_empty() {
            self.0.asking.set(Some(Asking::Unsaved(Then::Open(folder))));
            return;
        }
        self.switch_to(folder);
    }

    fn switch_to(&self, folder: PathBuf) {
        let inner = &self.0;
        let project = match Project::open(&folder) {
            Ok(project) => project,
            Err(error) => {
                inner.trouble.set(Some(error.to_string()));
                if let Some(config) = inner.config.borrow().as_ref() {
                    let _ = config.forget(&folder);
                }
                return;
            }
        };
        self.keep_session();
        let had_lsp = inner.work.borrow().lsp.is_some();
        *inner.work.borrow_mut() = workspace(project);
        inner.layout.set(default_layout());
        inner.binaries.set(None);
        inner.binary.set(0);
        inner.hits.set(Vec::new());
        inner.trouble.set(None);
        if let Ok(mut report) = inner.report.lock() {
            *report = Report::default();
        }
        let config = inner.config.borrow().clone();
        match config.as_ref().and_then(|config| {
            let _ = config.remember(&folder);
            config.session(&folder)
        }) {
            Some(session) => self.restore(&session),
            None => self.open_file("src/main.rs"),
        }
        if had_lsp {
            self.start_language_server();
        }
        self.watch();
        inner.version.update(|v| *v += 1);
    }

    /// Puts back how the project was left.
    fn restore(&self, session: &Session) {
        for (path, caret) in &session.open {
            if self.0.work.borrow().project.read(path).is_ok() {
                self.open_file(path);
                if let Some(document) = self.document(path) {
                    document.caret.set(Cursor::at(*caret));
                }
            }
        }
        let open = self.open_files();
        if let Ok(mut layout) = DockLayout::parse(&session.layout) {
            // A file that is gone from disk leaves the arrangement.
            for id in layout.panels().into_iter().map(str::to_owned).collect::<Vec<_>>() {
                let fixed = [EXPLORER, SEARCH, TERMINAL, PROBLEMS].contains(&id.as_str());
                if !fixed && !open.contains(&id) {
                    layout.close(&id);
                }
            }
            for path in &open {
                if !layout.contains(path) {
                    layout = layout.closed([path.as_str()], DockSide::Center);
                    layout.reopen(path);
                }
            }
            self.0.layout.set(layout);
        }
        if let Some(active) = session.active.as_ref().filter(|a| open.contains(a)) {
            self.0.layout.update(|layout| {
                layout.activate(active);
            });
        }
    }

    /// How the project is left now.
    pub fn session(&self) -> Session {
        let work = self.0.work.borrow();
        Session {
            layout: self.0.layout.get().to_text(),
            open: work
                .open
                .iter()
                .filter_map(|path| Some((path.clone(), work.documents.get(path)?.caret.get().caret)))
                .collect(),
            active: drop_then(work, || self.active()),
        }
    }

    /// Writes down how the project is left, when it remembers.
    fn keep_session(&self) {
        let Some(config) = self.0.config.borrow().clone() else { return };
        let _ = config.save_session(&self.root(), &self.session());
    }

    /// Looks at the disk every so often on a thread, and wakes the screen when it changed.
    fn watch(&self) {
        let work = self.0.work.borrow();
        let (stamp, stop, project, wake) =
            (work.stamp.clone(), work.stop_watching.clone(), work.project.clone(), self.wake());
        drop(work);
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(WATCH_EVERY));
                let now = project.stamp();
                if stamp.swap(now, Ordering::Relaxed) != now {
                    wake();
                }
            }
        });
    }

    /// Reads the disk again: the tree, and the open files that changed there. A file with no
    /// changes here takes what is on disk; one with changes asks which to keep.
    pub fn refresh(&self) {
        let mut work = self.0.work.borrow_mut();
        work.tree = work.project.tree().unwrap_or_default();
        let now = work.project.stamp();
        work.stamp.store(now, Ordering::Relaxed);
        work.seen = now;
        let mut gone = Vec::new();
        for (path, document) in &work.documents {
            match work.project.read(path) {
                Ok(disk) if disk != document.saved.get() => {
                    if document.unsaved() {
                        document.on_disk.set(Some(disk));
                    } else {
                        document
                            .history
                            .update(|h| h.record_change(&document.text.get(), &disk, document.caret.get()));
                        document.text.set(disk.clone());
                        document.saved.set(disk);
                    }
                }
                Ok(_) => {}
                Err(_) => gone.push(path.clone()),
            }
        }
        drop(work);
        for path in gone {
            if !self.is_unsaved(&path) {
                self.close_file(&path);
            }
        }
        self.0.version.update(|v| *v += 1);
    }

    /// Asks for a name for a new file or folder, or for a new name, as the Explorer's menu does.
    fn ask_name(&self, naming: Naming) {
        let start = match &naming {
            Naming::Rename(path) => path.rsplit('/').next().unwrap_or(path).to_owned(),
            _ => String::new(),
        };
        self.0.asking.set(Some(Asking::Name { naming, typed: State::new(start), refused: State::new(None) }));
    }

    /// Does what a name typed in the dialog was for. `Err` says why not, and nothing changed.
    pub fn name_it(&self, naming: &NamingFor, name: &str) -> Result<(), String> {
        let naming = match naming {
            NamingFor::File(folder) => Naming::File(folder.clone()),
            NamingFor::Folder(folder) => Naming::Folder(folder.clone()),
            NamingFor::Rename(path) => Naming::Rename(path.clone()),
        };
        self.apply_name(&naming, name)
    }

    fn apply_name(&self, naming: &Naming, name: &str) -> Result<(), String> {
        if let Some(why) = project::bad_name(name) {
            return Err(why.to_owned());
        }
        let join =
            |folder: &str| if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") };
        let result = {
            let work = self.0.work.borrow();
            match naming {
                Naming::File(folder) => work.project.create_file(&join(folder)),
                Naming::Folder(folder) => work.project.create_folder(&join(folder)),
                Naming::Rename(path) => {
                    let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
                    work.project.rename(path, &join(folder))
                }
            }
        };
        result.map_err(|error| error.to_string())?;
        match naming {
            Naming::File(folder) => {
                self.refresh();
                self.open_file(&join(folder));
            }
            Naming::Folder(_) => self.refresh(),
            Naming::Rename(path) => {
                let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
                self.renamed(path, &join(folder));
                self.refresh();
            }
        }
        Ok(())
    }

    /// Moves the open files under `from` to `to`: their tabs, their text and their changes.
    fn renamed(&self, from: &str, to: &str) {
        let moved: Vec<String> = self
            .open_files()
            .into_iter()
            .filter(|path| path == from || path.starts_with(&format!("{from}/")))
            .collect();
        for old in moved {
            let new = format!("{to}{}", &old[from.len()..]);
            let mut work = self.0.work.borrow_mut();
            let Some(document) = work.documents.remove(&old) else { continue };
            if let Some(lsp) = &work.lsp {
                lsp.close(&old);
                lsp.open(&new, &document.text.get());
            }
            work.documents.insert(new.clone(), document);
            for path in work.open.iter_mut() {
                if *path == old {
                    *path = new.clone();
                }
            }
            drop(work);
            let active = self.active().as_deref() == Some(old.as_str());
            self.0.layout.update(|layout| {
                layout.close(&old);
                *layout = layout.clone().closed([new.as_str()], DockSide::Center);
                layout.reopen(&new);
                if active {
                    layout.activate(&new);
                }
            });
        }
        self.0.version.update(|v| *v += 1);
    }

    /// Deletes a file or a folder for good, closing what was open in it.
    pub fn delete(&self, path: &str) -> Result<(), String> {
        let removed = self.0.work.borrow().project.remove(path);
        removed.map_err(|error| error.to_string())?;
        for open in self.open_files() {
            if open == path || open.starts_with(&format!("{path}/")) {
                self.close_file(&open);
            }
        }
        self.refresh();
        Ok(())
    }

    /// What closing the window does now: true when nothing is unsaved; otherwise it asks.
    pub fn may_close(&self) -> bool {
        if self.unsaved().is_empty() {
            self.keep_session();
            return true;
        }
        if self.0.asking.peek().is_none() {
            self.0.asking.set(Some(Asking::Unsaved(Then::Quit)));
        }
        false
    }

    /// Answers the question about unsaved changes: `save` writes them first, and `None`
    /// cancels. What it was asked for happens then, unless a file could not be written.
    pub fn answer_unsaved(&self, save: Option<bool>) {
        let Some(Asking::Unsaved(then)) = self.0.asking.get() else { return };
        self.0.asking.set(None);
        let Some(save) = save else { return };
        match &then {
            Then::CloseDocument(path) => {
                if save && !self.save(path) {
                    return;
                }
                self.close_file(path);
            }
            _ => {
                if save && !self.save_all() {
                    return;
                }
                if !save {
                    // Changes left behind are not saved: the files go back to the disk's text.
                    for path in self.unsaved() {
                        if let Some(document) = self.document(&path) {
                            document.text.set(document.saved.get());
                        }
                    }
                }
                match then {
                    Then::Quit => {
                        self.keep_session();
                        if let Some(closer) = self.0.closer.borrow().as_ref() {
                            closer.close();
                        }
                    }
                    Then::Open(folder) => self.switch_to(folder),
                    Then::CloseDocument(_) => {}
                }
            }
        }
    }

    /// Whether a question is open over the window, and which, in a few words.
    pub fn asking(&self) -> Option<String> {
        self.0.asking.get().map(|asking| match asking {
            Asking::Unsaved(Then::CloseDocument(path)) => format!("unsaved {path}"),
            Asking::Unsaved(_) => "unsaved".to_owned(),
            Asking::Name { naming, .. } => format!("name {naming:?}"),
            Asking::Delete(path) => format!("delete {path}"),
        })
    }

    fn panels(&self) -> Vec<DockPanel> {
        let mut panels = vec![
            DockPanel::new(EXPLORER, "Explorer").icon(IconName::Folder).closable(false),
            DockPanel::new(SEARCH, "Search").icon(IconName::Search).closable(false),
            DockPanel::new(TERMINAL, "Terminal").icon(IconName::Terminal).closable(false),
            DockPanel::new(PROBLEMS, "Problems").icon(IconName::TriangleAlert).closable(false),
        ];
        let work = self.0.work.borrow();
        for path in &work.open {
            let Some(document) = work.documents.get(path) else { continue };
            let name = path.rsplit('/').next().unwrap_or(path).to_owned();
            panels.push(
                DockPanel::new(path.clone(), name)
                    .kind(DockPanelKind::Document)
                    .close_action(DockCloseAction::Remove)
                    .icon(IconName::FileText)
                    .unsaved(document.unsaved()),
            );
        }
        panels
    }

    fn explorer(&self) -> AnyView {
        fn node(entry: &Entry) -> TreeNode {
            let icon = if entry.folder { IconName::Folder } else { IconName::FileText };
            entry
                .children
                .iter()
                .fold(TreeNode::new(entry.path.clone(), entry.name.clone()).icon(icon), |n, c| {
                    n.child(node(c))
                })
        }
        let work = self.0.work.borrow();
        let nodes: Vec<TreeNode> = work.tree.iter().map(node).collect();
        let mut files = Vec::new();
        project::collect_files(&work.tree, &mut files);
        drop(work);
        let folders: Rc<Vec<String>> = Rc::new(Vec::new());
        let (app, reads, all_files) = (self.clone(), self.0.selected.clone(), files.clone());
        // Choosing a file opens it.
        let selected = Binding::new(
            move || reads.get(),
            move |path: Option<String>| {
                if let Some(path) = path.filter(|path| all_files.contains(path)) {
                    app.open_file(&path);
                }
            },
        );
        let app = self.clone();
        let _ = folders;
        let menu = move |key: &str| -> Vec<MenuItem> {
            let is_file = files.iter().any(|f| f == key);
            // A new file or folder goes in the folder clicked, or beside the file clicked.
            let folder = if is_file {
                key.rsplit_once('/').map_or(String::new(), |(f, _)| f.to_owned())
            } else {
                key.to_owned()
            };
            let (new_file, new_folder, rename, delete) = (app.clone(), app.clone(), app.clone(), app.clone());
            let (in_file, in_folder, path, gone) = (folder.clone(), folder, key.to_owned(), key.to_owned());
            vec![
                MenuItem::new("New File…")
                    .on_select(move || new_file.ask_name(Naming::File(in_file.clone()))),
                MenuItem::new("New Folder…")
                    .on_select(move || new_folder.ask_name(Naming::Folder(in_folder.clone()))),
                MenuItem::new("Rename…").on_select(move || rename.ask_name(Naming::Rename(path.clone()))),
                MenuItem::new("Delete")
                    .on_select(move || delete.0.asking.set(Some(Asking::Delete(gone.clone())))),
            ]
        };
        Box::new(TreeView::new(nodes).selected(selected).context_menu(menu).label("Files").padding(8.0))
    }

    fn search_panel(&self) -> AnyView {
        let app = self.clone();
        let field = TextField::new(self.0.search.binding())
            .placeholder("Search the project")
            .icon(IconName::Search)
            .on_submit(move |query| app.search(&query));
        let case = ToggleButton::new(self.0.search_case.binding(), "Aa");
        let hits = self.0.hits.get();
        let mut rows: Vec<AnyView> =
            vec![Box::new(HStack::new((field, case)).spacing(4.0).align(Align::Center))];
        if !self.0.search.get().is_empty() {
            let count = match hits.len() {
                0 => "No results".to_owned(),
                1 => "1 result".to_owned(),
                n => format!("{n} results"),
            };
            rows.push(Box::new(Text::new(count).style(TextStyle::Caption)));
        }
        for hit in hits {
            let label = format!("{}:{} · {}", hit.file, hit.line, hit.text);
            let app = self.clone();
            rows.push(Box::new(
                Button::new(label)
                    .icon(IconName::FileText)
                    .style(ButtonStyle::Ghost)
                    .on_click(move || app.go_to_place(&hit.file, hit.line, hit.column)),
            ));
        }
        Box::new(ScrollView::new((VStack::new(rows).spacing(4.0).padding(8.0).align(Align::Stretch),)))
    }

    fn problems(&self) -> AnyView {
        let report = self.0.shown.get();
        let mut diagnostics = report.diagnostics.clone();
        // What the language server found in the open files, past what the last build said.
        if let Some(lsp) = &self.0.work.borrow().lsp {
            for path in self.open_files() {
                if let Some(found) = lsp.diagnostics(&path) {
                    diagnostics.retain(|d| d.file != path);
                    diagnostics.extend(found);
                }
            }
        }
        if diagnostics.is_empty() {
            let words = if report.success == Some(true) { "No problems" } else { "Build to see problems" };
            return Box::new(Text::new(words).padding(12.0));
        }
        let rows: Vec<AnyView> = diagnostics
            .iter()
            .map(|diagnostic| -> AnyView {
                let mark = match diagnostic.level {
                    Level::Error => IconName::CircleAlert,
                    Level::Warning => IconName::TriangleAlert,
                };
                let label = format!(
                    "{}:{}:{} · {}",
                    diagnostic.file, diagnostic.line, diagnostic.column, diagnostic.message
                );
                let (app, found) = (self.clone(), diagnostic.clone());
                Box::new(
                    Button::new(label)
                        .icon(mark)
                        .style(ButtonStyle::Ghost)
                        .on_click(move || app.go_to(&found)),
                )
            })
            .collect();
        Box::new(ScrollView::new((VStack::new(rows).spacing(2.0).padding(8.0),)))
    }

    fn terminal_panel(&self) -> AnyView {
        let output = CodeEditor::new(self.0.terminal.binding())
            .language(CodeLanguage::Plain)
            .line_numbers(false)
            .read_only(true)
            .follow_end(true)
            .fill();
        let running = self.0.shown.get().running;
        let (send, stop) = (self.clone(), self.clone());
        let typed = self.0.typed.clone();
        let keys: KeyHandler = Rc::new(move |key, modifiers| {
            // Ctrl+C stops the program, as in a terminal; Cmd+C still copies on a Mac.
            if modifiers.ctrl
                && !modifiers.meta
                && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("c"))
            {
                stop.stop();
                return true;
            }
            false
        });
        let input = TextField::new(self.0.typed.binding())
            .placeholder(if running { "Type for the program, Enter sends" } else { "Nothing is running" })
            .icon(IconName::ChevronRight)
            .recall(self.0.sent.clone())
            .intercept_keys(keys)
            .disabled(!running)
            .on_submit(move |line| {
                send.send_line(&line);
                typed.set(String::new());
            });
        Box::new(VStack::new((output, input)).spacing(4.0).grow(1.0).align(Align::Stretch).padding(4.0))
    }

    fn editor(&self, path: &str) -> AnyView {
        let Some(document) = self.document(path) else { return Box::new(Text::new(path.to_owned())) };
        let rust = path.ends_with(".rs");
        let language = if rust { CodeLanguage::Rust } else { CodeLanguage::Plain };
        let work = self.0.work.borrow();
        let from_lsp = work.lsp.as_ref().and_then(|lsp| lsp.diagnostics(path));
        let has_lsp = work.lsp.is_some() && rust;
        drop(work);
        let found = from_lsp.unwrap_or_else(|| {
            self.0.shown.get().diagnostics.into_iter().filter(|d| d.file == path).collect()
        });
        let diagnostics: Vec<CodeDiagnostic> = found
            .iter()
            .map(|d| match d.level {
                Level::Error => CodeDiagnostic::error(d.line, d.column, d.length, d.message.clone()),
                Level::Warning => CodeDiagnostic::warning(d.line, d.column, d.length, d.message.clone()),
            })
            .collect();
        let mut editor = CodeEditor::new(document.text.binding())
            .language(language)
            .diagnostics(diagnostics)
            .caret(document.caret.clone())
            .history(document.history.clone())
            .focus_request(document.focus.clone())
            .fill();
        if has_lsp {
            let (complete, hover, go) = (self.clone(), self.clone(), self.clone());
            let (at_complete, at_hover, at_go) = (path.to_owned(), path.to_owned(), path.to_owned());
            editor = editor
                .on_complete(move |at| complete.with_lsp(|lsp| lsp.complete(&at_complete, at)))
                .completions(self.0.offered.get())
                .on_hover(move |at| hover.with_lsp(|lsp| lsp.ask_hover(&at_hover, at)))
                .hover(self.0.tip.get())
                .on_go_to(move |at| go.with_lsp(|lsp| lsp.ask_definition(&at_go, at)));
        }
        // Found by the file it edits, as a test of Raven Test Studio asks for it.
        let editor = editor.test_id(format!("editor:{path}"));
        let Some(disk) = document.on_disk.get() else { return Box::new(editor) };
        // Changed on disk while it had changes here: which to keep.
        let (reload, keep) = (document.clone(), document.clone());
        let bar = HStack::new((
            Text::new("This file changed on disk.").grow(1.0),
            Button::new("Reload").style(ButtonStyle::Secondary).on_click(move || {
                reload.history.update(|h| h.record_change(&reload.text.get(), &disk, reload.caret.get()));
                reload.text.set(disk.clone());
                reload.saved.set(disk.clone());
                reload.on_disk.set(None);
            }),
            Button::new("Keep mine").style(ButtonStyle::Ghost).on_click(move || keep.on_disk.set(None)),
        ))
        .spacing(8.0)
        .align(Align::Center)
        .padding(6.0);
        Box::new(VStack::new((bar, editor)).spacing(0.0).grow(1.0).align(Align::Stretch))
    }

    fn with_lsp(&self, ask: impl FnOnce(&Lsp)) {
        if let Some(lsp) = &self.0.work.borrow().lsp {
            ask(lsp);
        }
    }

    fn welcome(&self) -> AnyView {
        let mut rows: Vec<AnyView> = vec![Box::new(Text::new("Open a file from the Explorer"))];
        let recent = self.0.config.borrow().as_ref().map(Config::recent).unwrap_or_default();
        let here = self.root();
        let recent: Vec<PathBuf> = recent.into_iter().filter(|p| *p != here).collect();
        if !recent.is_empty() {
            rows.push(Box::new(Text::new("Recent").style(TextStyle::Headline)));
            for folder in recent {
                let app = self.clone();
                let label = folder.display().to_string();
                rows.push(Box::new(
                    Button::new(label)
                        .icon(IconName::Folder)
                        .style(ButtonStyle::Ghost)
                        .on_click(move || app.open_project(folder.clone())),
                ));
            }
        }
        Box::new(VStack::new(rows).spacing(8.0).padding(24.0).align(Align::Start))
    }

    fn status(&self) -> StatusBar {
        let report = self.0.shown.get();
        let task = report.task.map_or("Build", Task::name);
        let state = match (&self.0.trouble.get(), report.running, report.task, report.success) {
            (Some(trouble), ..) => trouble.clone(),
            (None, true, Some(Task::Run), _) => "Running…".to_owned(),
            (None, true, _, _) => format!("{task}…"),
            (None, false, _, _) if report.stopped => "Stopped".to_owned(),
            (None, false, Some(Task::Test), Some(_)) => {
                let (passed, failed) = report.tests.unwrap_or_default();
                format!("Tests: {passed} passed, {failed} failed")
            }
            (None, false, _, Some(true)) => format!("{task} succeeded · {}", report.summary()),
            (None, false, _, Some(false)) => format!("{task} failed · {}", report.summary()),
            (None, false, _, None) => "Ready".to_owned(),
        };
        let file = self.active().unwrap_or_default();
        let note = self.0.lsp_note.get().unwrap_or_default();
        StatusBar::new().left((StatusBar::action(self.name()), StatusBar::text(state))).right((
            StatusBar::text(note),
            StatusBar::text(file),
            StatusBar::text("Rust"),
        ))
    }

    fn dialog(&self) -> Option<AnyView> {
        let asking = self.0.asking.get()?;
        let open = {
            let asking = self.0.asking.clone();
            Binding::new(
                || true,
                move |open: bool| {
                    if !open {
                        asking.set(None);
                    }
                },
            )
        };
        Some(match asking {
            Asking::Unsaved(then) => {
                let files = match &then {
                    Then::CloseDocument(path) => vec![path.clone()],
                    _ => self.unsaved(),
                };
                let (save, discard, cancel) = (self.clone(), self.clone(), self.clone());
                let leave = match then {
                    Then::Quit => "close",
                    Then::Open(_) => "open another folder",
                    Then::CloseDocument(_) => "close it",
                };
                Box::new(
                    Dialog::new(open, "Save your changes?")
                        .description(format!(
                            "These files have changes that are not saved: {}.",
                            files.join(", ")
                        ))
                        .close_on_backdrop(false)
                        .action(
                            Button::new("Cancel")
                                .style(ButtonStyle::Ghost)
                                .on_click(move || cancel.answer_unsaved(None)),
                        )
                        .action(
                            Button::new(format!("Don't save and {leave}"))
                                .style(ButtonStyle::Secondary)
                                .on_click(move || discard.answer_unsaved(Some(false))),
                        )
                        .action(
                            Button::new(format!("Save and {leave}"))
                                .on_click(move || save.answer_unsaved(Some(true))),
                        ),
                )
            }
            Asking::Name { naming, typed, refused } => {
                let title = match &naming {
                    Naming::File(_) => "New file",
                    Naming::Folder(_) => "New folder",
                    Naming::Rename(_) => "Rename",
                };
                let go = {
                    let (app, naming, typed, refused) =
                        (self.clone(), naming.clone(), typed.clone(), refused.clone());
                    move || match app.apply_name(&naming, &typed.get()) {
                        Ok(()) => app.0.asking.set(None),
                        Err(why) => refused.set(Some(why)),
                    }
                };
                let submit = go.clone();
                let field =
                    TextField::new(typed.binding()).autofocus().select_on_open().on_submit(move |_| submit());
                let cancel = self.0.asking.clone();
                Box::new(
                    Dialog::new(open, title)
                        .content(FormField::new("Name", field).error_text(refused.get()))
                        .action(
                            Button::new("Cancel")
                                .style(ButtonStyle::Ghost)
                                .on_click(move || cancel.set(None)),
                        )
                        .action(
                            Button::new(if title == "Rename" { "Rename" } else { "Create" }).on_click(go),
                        ),
                )
            }
            Asking::Delete(path) => {
                let (app, cancel) = (self.clone(), self.0.asking.clone());
                let gone = path.clone();
                Box::new(
                    Dialog::new(open, format!("Delete {path}?"))
                        .description("It is deleted for good, with everything in it. This cannot be undone.")
                        .action(
                            Button::new("Cancel")
                                .style(ButtonStyle::Ghost)
                                .on_click(move || cancel.set(None)),
                        )
                        .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || {
                            app.0.asking.set(None);
                            if let Err(why) = app.delete(&gone) {
                                app.0.trouble.set(Some(why));
                            }
                        })),
                )
            }
        })
    }

    fn open_folder_dialog(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(folder) = ravenrs::native::OpenDialog::new().title("Open Folder").folders().ask() {
            self.open_project(folder);
        }
    }
}

/// Lets go of a borrow before running `then`, which borrows again.
fn drop_then<T, R>(held: T, then: impl FnOnce() -> R) -> R {
    drop(held);
    then()
}

/// What a name typed in the Explorer's dialog is for, for tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamingFor {
    /// A new file in this folder ("" for the top).
    File(String),
    /// A new folder in this folder.
    Folder(String),
    /// A new name for this file or folder.
    Rename(String),
}

impl View for RavenCode {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = &self.0;
        let first = inner.waker.borrow().is_none();
        *inner.waker.borrow_mut() = Some(cx.ui_waker());
        *inner.closer.borrow_mut() = Some(cx.window_closer());
        if first {
            self.watch();
        }
        let _ = inner.version.get();
        // The disk changed since the last look.
        let changed = {
            let work = inner.work.borrow();
            work.stamp.load(Ordering::Relaxed) != work.seen
        };
        if changed {
            self.refresh();
        }
        // What cargo's thread wrote since the last build.
        let report = self.report();
        if inner.shown.peek().clone() != report {
            inner.terminal.set(report.output.clone());
            // A build that ended with something found brings the problems to the front.
            let ended = inner.shown.peek().running && !report.running;
            let found = !report.diagnostics.is_empty()
                && !report.running
                && (ended || report.diagnostics.len() != inner.shown.peek().diagnostics.len());
            inner.shown.set(report);
            if found {
                inner.layout.update(|layout| {
                    layout.activate(PROBLEMS);
                });
            }
        }
        // The language server: what changed goes to it, and what it answered comes back.
        if let Some(lsp) = &inner.work.borrow().lsp {
            for (path, document) in &inner.work.borrow().documents {
                let text = document.text.get();
                if *document.told.borrow() != text {
                    lsp.change(path, &text);
                    *document.told.borrow_mut() = text;
                }
            }
            let offered: Vec<CodeCompletion> = lsp
                .completions()
                .into_iter()
                .map(|c| {
                    let mut suggestion = CodeCompletion::new(c.label).insert(c.insert);
                    if let Some(detail) = c.detail {
                        suggestion = suggestion.detail(detail);
                    }
                    suggestion
                })
                .collect();
            inner.offered.set_if_changed(offered);
            inner.tip.set_if_changed(lsp.hover());
            if lsp.is_gone() && inner.lsp_note.peek().is_none() {
                inner.lsp_note.set(Some("rust-analyzer stopped".to_owned()));
            }
        }
        let definition = inner.work.borrow().lsp.as_ref().and_then(Lsp::take_definition);
        if let Some(found) = definition {
            self.go_to_place(&found.file, found.line, found.column);
        }
        let quit = self.clone();
        cx.on_close_request(move || quit.may_close());

        let (save, build, run, test, open, stop) =
            (self.clone(), self.clone(), self.clone(), self.clone(), self.clone(), self.clone());
        cx.shortcut(
            Key::Character("s".into()),
            true,
            Rc::new(move || {
                if let Some(path) = save.active() {
                    save.save(&path);
                }
            }),
        );
        cx.shortcut(Key::Character("o".into()), true, Rc::new(move || open.open_folder_dialog()));
        cx.shortcut(Key::Character("b".into()), true, Rc::new(move || build.run(Task::Build)));
        cx.shortcut_with_shift(Key::Character("t".into()), true, Rc::new(move || test.run(Task::Test)));
        cx.shortcut_with_shift(
            Key::Function(5),
            false,
            Rc::new(move || {
                stop.stop();
            }),
        );
        cx.shortcut(Key::Function(5), false, Rc::new(move || run.run(Task::Run)));
        let search = inner.layout.clone();
        cx.shortcut_with_shift(
            Key::Character("f".into()),
            true,
            Rc::new(move || {
                search.update(|layout| {
                    layout.activate(SEARCH);
                });
            }),
        );
        let running = inner.shown.get().running;
        let (open_folder, save_all, build, run, stop) =
            (self.clone(), self.clone(), self.clone(), self.clone(), self.clone());
        let tasks = [Task::Test, Task::Check, Task::Clippy].into_iter().fold(
            DropdownMenu::new("Tasks").style(ButtonStyle::Ghost).size(ButtonSize::Small),
            |menu, task| {
                let app = self.clone();
                let mut item = MenuItem::new(task.name()).on_select(move || app.run(task));
                if task == Task::Test {
                    item = item.shortcut("Ctrl+Shift+T");
                }
                if running {
                    item = item.disabled("Cargo is running");
                }
                menu.item(item)
            },
        );
        let mut tools = Toolbar::new()
            .action(
                ToolbarAction::new("Open Folder…")
                    .icon(IconName::Folder)
                    .on_click(move || open_folder.open_folder_dialog()),
            )
            .action(ToolbarAction::new("Save all").icon(IconName::Save).on_click(move || {
                save_all.save_all();
            }))
            .separator()
            .action(
                ToolbarAction::new(if running { "Running…" } else { "Build" })
                    .icon(IconName::Package)
                    .disabled(running)
                    .on_click(move || build.run(Task::Build)),
            )
            .action(
                ToolbarAction::new("Run")
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move || run.run(Task::Run)),
            )
            .action(ToolbarAction::new("Stop").icon(IconName::Square).disabled(!running).on_click(
                move || {
                    stop.stop();
                },
            ))
            .view(96.0, tasks);
        let binaries = inner.binaries.get().unwrap_or_default();
        if binaries.len() > 1 {
            tools = tools.view(160.0, Select::new(inner.binary.binding(), binaries).label("Binary"));
        }
        let app = self.clone();
        let content = move |id: &str| -> AnyView {
            match id {
                EXPLORER => app.explorer(),
                SEARCH => app.search_panel(),
                TERMINAL => app.terminal_panel(),
                PROBLEMS => app.problems(),
                path => app.editor(path),
            }
        };
        let (welcome, guard) = (self.clone(), self.clone());
        let area = DockArea::new(inner.layout.binding(), self.panels(), content)
            .on_documents_empty(move || welcome.welcome())
            .on_change(move |_, _, change| match change {
                DockChange::Close { panel } if guard.is_unsaved(panel) => {
                    guard.0.asking.set(Some(Asking::Unsaved(Then::CloseDocument(panel.clone()))));
                    Err(format!("{panel} has unsaved changes"))
                }
                DockChange::Close { panel } if guard.document(panel).is_some() => {
                    // Closing a document lets go of it.
                    let app = guard.clone();
                    let panel = panel.clone();
                    let mut work = app.0.work.borrow_mut();
                    work.documents.remove(&panel);
                    work.open.retain(|p| *p != panel);
                    if let Some(lsp) = &work.lsp {
                        lsp.close(&panel);
                    }
                    drop(work);
                    app.0.version.update(|v| *v += 1);
                    Ok(())
                }
                _ => Ok(()),
            })
            .label("Workbench");
        let title = TitleBar::new(format!("{} — Raven Code", self.name()));
        let mut parts: Vec<AnyView> =
            vec![Box::new(title), Box::new(tools), Box::new(area), Box::new(self.status())];
        if let Some(dialog) = self.dialog() {
            parts.push(dialog);
        }
        let window = VStack::new(parts).spacing(0.0).grow(1.0).align(Align::Stretch);
        cx.child(&window)
    }
}
