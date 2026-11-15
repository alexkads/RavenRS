//! The window: the files at the left, the open files in the middle, the terminal and the problems
//! below, a toolbar to save, build and run, and a status bar. It holds the state and wires the
//! [`project`](crate::project) and [`cargo`](crate::cargo) parts to the components; what they do is
//! in them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use ravenrs::core::Key;
use ravenrs::prelude::*;
use ravenrs::widgets::Cursor;

use crate::cargo::{self, Diagnostic, Level, Report, Task};
use crate::project::{Entry, Project};

/// The panels that are always there.
const EXPLORER: &str = "explorer";
const TERMINAL: &str = "terminal";
const PROBLEMS: &str = "problems";

/// An open file: its text, the text on disk, and where its caret is.
#[derive(Clone)]
struct Document {
    text: State<String>,
    saved: State<String>,
    caret: State<Cursor>,
}

impl Document {
    fn unsaved(&self) -> bool {
        self.text.get() != self.saved.get()
    }
}

/// What the window keeps.
struct Inner {
    project: Project,
    tree: Vec<Entry>,
    layout: State<DockLayout>,
    documents: RefCell<BTreeMap<String, Document>>,
    /// The paths open, which the panels are built from.
    open: State<Vec<String>>,
    selected: State<Option<String>>,
    /// What cargo said, written by its thread.
    report: Arc<Mutex<Report>>,
    /// A copy of it, read by the build, so that a change shows.
    shown: State<Report>,
    terminal: State<String>,
    waker: RefCell<Option<UiWaker>>,
    /// A line for the status bar, when something did not work.
    trouble: State<Option<String>>,
}

/// Raven Code on a project.
#[derive(Clone)]
pub struct RavenCode(Rc<Inner>);

impl std::fmt::Debug for RavenCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RavenCode").field("project", &self.0.project).finish_non_exhaustive()
    }
}

impl RavenCode {
    /// Raven Code on `project`, with `src/main.rs` open when there is one.
    pub fn new(project: Project) -> Self {
        let tree = project.tree().unwrap_or_default();
        let layout = DockLayout::new()
            .left(DockNode::tabs([EXPLORER]), 260.0)
            .bottom(DockNode::tabs([TERMINAL, PROBLEMS]), 220.0);
        let app = Self(Rc::new(Inner {
            project,
            tree,
            layout: State::new(layout),
            documents: RefCell::new(BTreeMap::new()),
            open: State::new(Vec::new()),
            selected: State::new(None),
            report: Arc::new(Mutex::new(Report::default())),
            shown: State::new(Report::default()),
            terminal: State::new(String::new()),
            waker: RefCell::new(None),
            trouble: State::new(None),
        }));
        app.open_file("src/main.rs");
        app
    }

    /// The project.
    pub fn project(&self) -> &Project {
        &self.0.project
    }

    /// The layout of the window, to save or to check.
    pub fn layout(&self) -> DockLayout {
        self.0.layout.get()
    }

    /// The text of an open file.
    pub fn text_of(&self, path: &str) -> Option<String> {
        self.0.documents.borrow().get(path).map(|document| document.text.get())
    }

    /// Changes the text of an open file, as typing would.
    pub fn edit(&self, path: &str, text: &str) {
        if let Some(document) = self.0.documents.borrow().get(path) {
            document.text.set(text.to_owned());
        }
    }

    /// Whether an open file has changes that are not on disk.
    pub fn is_unsaved(&self, path: &str) -> bool {
        self.0.documents.borrow().get(path).is_some_and(Document::unsaved)
    }

    /// Where the caret of an open file is, as a character index.
    pub fn caret_of(&self, path: &str) -> Option<usize> {
        self.0.documents.borrow().get(path).map(|document| document.caret.get().caret)
    }

    /// Opens a file of the project, or brings it to the front when it is open.
    pub fn open_file(&self, path: &str) {
        let inner = &self.0;
        if !inner.documents.borrow().contains_key(path) {
            let text = match inner.project.read(path) {
                Ok(text) => text,
                Err(error) => {
                    inner.trouble.set(Some(format!("{path}: {error}")));
                    return;
                }
            };
            let document = Document {
                text: State::new(text.clone()),
                saved: State::new(text),
                caret: State::new(Cursor::default()),
            };
            inner.documents.borrow_mut().insert(path.to_owned(), document);
            inner.open.update(|open| open.push(path.to_owned()));
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

    /// Writes an open file to disk.
    pub fn save(&self, path: &str) {
        let documents = self.0.documents.borrow();
        let Some(document) = documents.get(path) else { return };
        let text = document.text.get();
        match self.0.project.write(path, &text) {
            Ok(()) => document.saved.set(text),
            Err(error) => self.0.trouble.set(Some(format!("{path}: {error}"))),
        }
    }

    /// Writes every open file with changes.
    pub fn save_all(&self) {
        let unsaved: Vec<String> = self
            .0
            .documents
            .borrow()
            .iter()
            .filter(|(_, d)| d.unsaved())
            .map(|(path, _)| path.clone())
            .collect();
        for path in unsaved {
            self.save(&path);
        }
    }

    /// The open file at the front, if one is.
    pub fn active(&self) -> Option<String> {
        let layout = self.0.layout.get();
        let documents = self.0.documents.borrow();
        layout
            .panels()
            .into_iter()
            .find(|id| documents.contains_key(*id) && layout.is_active(id))
            .map(str::to_owned)
    }

    /// Saves what changed and starts cargo. What it says shows as it comes.
    pub fn run(&self, task: Task) {
        self.save_all();
        let waker = self.0.waker.borrow().clone();
        let root = self.0.project.root().to_path_buf();
        cargo::start(root, task, self.0.report.clone(), move || {
            if let Some(waker) = &waker {
                waker.wake();
            }
        });
        self.0.layout.update(|layout| {
            layout.activate(TERMINAL);
        });
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
        if let Some(waker) = self.0.waker.borrow().as_ref() {
            waker.wake();
        }
    }

    /// Opens the file of a diagnostic with the caret on it.
    pub fn go_to(&self, diagnostic: &Diagnostic) {
        self.open_file(&diagnostic.file);
        if let Some(document) = self.0.documents.borrow().get(&diagnostic.file) {
            let at = offset_of(&document.text.get(), diagnostic.line, diagnostic.column);
            document.caret.set(Cursor::at(at));
        }
    }

    fn panels(&self) -> Vec<DockPanel> {
        let mut panels = vec![
            DockPanel::new(EXPLORER, "Explorer").icon(IconName::Folder).closable(false),
            DockPanel::new(TERMINAL, "Terminal").icon(IconName::Terminal).closable(false),
            DockPanel::new(PROBLEMS, "Problems").icon(IconName::TriangleAlert).closable(false),
        ];
        let documents = self.0.documents.borrow();
        for path in self.0.open.get() {
            let Some(document) = documents.get(&path) else { continue };
            let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
            panels.push(
                DockPanel::new(path.clone(), name)
                    .kind(DockPanelKind::Document)
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
        let nodes: Vec<TreeNode> = self.0.tree.iter().map(node).collect();
        let files: Vec<String> = flatten(&self.0.tree);
        let (app, reads) = (self.clone(), self.0.selected.clone());
        // Choosing a file opens it.
        let selected = Binding::new(
            move || reads.get(),
            move |path: Option<String>| {
                if let Some(path) = path.filter(|path| files.contains(path)) {
                    app.open_file(&path);
                }
            },
        );
        Box::new(TreeView::new(nodes).selected(selected).padding(8.0))
    }

    fn problems(&self) -> AnyView {
        let report = self.0.shown.get();
        if report.diagnostics.is_empty() {
            let words = if report.success == Some(true) { "No problems" } else { "Build to see problems" };
            return Box::new(Text::new(words).padding(12.0));
        }
        let rows: Vec<AnyView> = report
            .diagnostics
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

    fn document(&self, path: &str) -> AnyView {
        let documents = self.0.documents.borrow();
        let Some(document) = documents.get(path) else { return Box::new(Text::new(path.to_owned())) };
        let language = if path.ends_with(".rs") { CodeLanguage::Rust } else { CodeLanguage::Plain };
        let diagnostics: Vec<CodeDiagnostic> = self
            .0
            .shown
            .get()
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.file == path)
            .map(|d| match d.level {
                Level::Error => CodeDiagnostic::error(d.line, d.column, d.length, d.message.clone()),
                Level::Warning => CodeDiagnostic::warning(d.line, d.column, d.length, d.message.clone()),
            })
            .collect();
        Box::new(
            CodeEditor::new(document.text.binding())
                .language(language)
                .diagnostics(diagnostics)
                .caret(document.caret.clone())
                .fill(),
        )
    }

    fn status(&self) -> StatusBar {
        let report = self.0.shown.get();
        let state = match (&self.0.trouble.get(), report.running, report.task, report.success) {
            (Some(trouble), ..) => trouble.clone(),
            (None, true, Some(Task::Run), _) => "Running…".to_owned(),
            (None, true, _, _) => "Building…".to_owned(),
            (None, false, _, Some(true)) => format!("Build succeeded · {}", report.summary()),
            (None, false, _, Some(false)) => format!("Build failed · {}", report.summary()),
            (None, false, _, None) => "Ready".to_owned(),
        };
        let file = self.active().unwrap_or_default();
        StatusBar::new()
            .left((StatusBar::action(self.0.project.name()), StatusBar::text(state)))
            .right((StatusBar::text(file), StatusBar::text("Rust")))
    }
}

/// Every file of a tree, by path.
fn flatten(entries: &[Entry]) -> Vec<String> {
    let mut files = Vec::new();
    for entry in entries {
        if entry.folder {
            files.extend(flatten(&entry.children));
        } else {
            files.push(entry.path.clone());
        }
    }
    files
}

impl View for RavenCode {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = &self.0;
        *inner.waker.borrow_mut() = Some(cx.ui_waker());
        // What cargo's thread wrote since the last build.
        let report = self.report();
        if inner.shown.peek().clone() != report {
            inner.terminal.set(report.output.clone());
            // A build that ended with something found brings the problems to the front.
            let found = !report.running
                && !report.diagnostics.is_empty()
                && inner.shown.peek().running != report.running
                || (!report.running
                    && report.diagnostics.len() != inner.shown.peek().diagnostics.len()
                    && !report.diagnostics.is_empty());
            inner.shown.set(report);
            if found {
                inner.layout.update(|layout| {
                    layout.activate(PROBLEMS);
                });
            }
        }
        let (save, build, run) = (self.clone(), self.clone(), self.clone());
        cx.shortcut(
            Key::Character("s".into()),
            true,
            Rc::new(move || {
                if let Some(path) = save.active() {
                    save.save(&path);
                }
            }),
        );
        cx.shortcut(Key::Character("b".into()), true, Rc::new(move || build.run(Task::Build)));
        cx.shortcut(Key::Function(5), false, Rc::new(move || run.run(Task::Run)));
        let running = inner.shown.get().running;
        let (save_all, build, run) = (self.clone(), self.clone(), self.clone());
        let tools = Toolbar::new()
            .action(ToolbarAction::new("Save all").icon(IconName::Save).on_click(move || save_all.save_all()))
            .separator()
            .action(
                ToolbarAction::new(if running { "Building…" } else { "Build" })
                    .icon(IconName::Package)
                    .disabled(running)
                    .on_click(move || build.run(Task::Build)),
            )
            .action(
                ToolbarAction::new("Run")
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move || run.run(Task::Run)),
            );
        let app = self.clone();
        let terminal = inner.terminal.clone();
        let content = move |id: &str| -> AnyView {
            match id {
                EXPLORER => app.explorer(),
                TERMINAL => Box::new(
                    CodeEditor::new(terminal.binding())
                        .language(CodeLanguage::Plain)
                        .line_numbers(false)
                        .read_only(true)
                        .fill(),
                ),
                PROBLEMS => app.problems(),
                path => app.document(path),
            }
        };
        let area = DockArea::new(inner.layout.binding(), self.panels(), content)
            .on_documents_empty(|| -> AnyView { Box::new(Text::new("Open a file from the Explorer")) })
            .label("Workbench");
        let title = TitleBar::new(format!("{} — Raven Code", inner.project.name()));
        let window =
            VStack::new((title, tools, area, self.status())).spacing(0.0).grow(1.0).align(Align::Stretch);
        cx.child(&window)
    }
}
