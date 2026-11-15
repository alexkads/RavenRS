//! A code editor: a rail of views at the left switches the side bar (files, search, source
//! control), the code is in the middle split in two, the terminal, the problems and the output are
//! below, and the outline is hidden at the right edge, a tab on its rail.

use std::collections::HashMap;

use ravenrs::prelude::*;

use crate::console;

/// The views of the side bar, in the order of the rail.
const SIDE: [&str; 3] = ["files", "search", "git"];

const MAIN: &str = r#"mod sales;

use sales::Order;

fn main() {
    let order = Order::new("ACME", 3);
    println!("{}", order.total());
}
"#;

const SALES: &str = r#"/// An order of a customer.
pub struct Order {
    customer: String,
    items: u32,
}

impl Order {
    pub fn new(customer: &str, items: u32) -> Self {
        Self { customer: customer.to_owned(), items }
    }

    /// What it costs, at 19.90 an item.
    pub fn total(&self) -> f64 {
        f64::from(self.items) * 19.90
    }
}
"#;

const LIB: &str = r#"//! The shop, as a library.

pub mod sales;
pub mod stock;
"#;

const TERMINAL: &str = "$ cargo run\n   Compiling shop v0.1.0\n    Finished dev profile in 1.84s\n     Running `target/debug/shop`\n59.7\n$ ";

const OUTPUT: &str = "[rust-analyzer] indexing 128 files\n[rust-analyzer] ready\n[git] fetched origin/main";

/// Where everything starts: the side bar at the left, the code split in two, the panels below, and
/// the outline hidden at the right edge.
fn start() -> DockLayout {
    let mut layout = DockLayout::new()
        .center(DockNode::horizontal([DockNode::tabs(["main.rs", "lib.rs"]), DockNode::tabs(["sales.rs"])]))
        .left(DockNode::tabs(SIDE), 260.0)
        .bottom(DockNode::tabs(["terminal", "problems", "output"]), 210.0)
        .right(DockNode::tabs(["outline"]), 240.0);
    layout.auto_hide("outline");
    layout
}

fn panels() -> Vec<DockPanel> {
    let document = |id: &str| DockPanel::new(id, id).kind(DockPanelKind::Document).icon(IconName::FileText);
    vec![
        DockPanel::new("files", "Explorer").icon(IconName::Folder).closable(false),
        DockPanel::new("search", "Search").icon(IconName::Search).closable(false),
        DockPanel::new("git", "Source control").icon(IconName::GitBranch).closable(false),
        document("main.rs"),
        document("lib.rs"),
        document("sales.rs").unsaved(true),
        DockPanel::new("terminal", "Terminal").icon(IconName::Terminal),
        DockPanel::new("problems", "Problems").icon(IconName::TriangleAlert),
        DockPanel::new("output", "Output"),
        DockPanel::new("outline", "Outline").icon(IconName::Layers),
    ]
}

/// The files of the project.
fn files() -> TreeView {
    TreeView::new(vec![
        TreeNode::new("src", "src")
            .icon(IconName::Folder)
            .child(TreeNode::new("main.rs", "main.rs").icon(IconName::FileText))
            .child(TreeNode::new("lib.rs", "lib.rs").icon(IconName::FileText))
            .child(TreeNode::new("sales.rs", "sales.rs").icon(IconName::FileText)),
        TreeNode::new("Cargo.toml", "Cargo.toml").icon(IconName::File),
        TreeNode::new("README.md", "README.md").icon(IconName::File),
    ])
}

/// The code editor, as a whole window.
pub fn editor() -> impl View {
    let layout = State::new(start());
    let sources: HashMap<&'static str, State<String>> =
        [("main.rs", MAIN), ("sales.rs", SALES), ("lib.rs", LIB)]
            .map(|(id, text)| (id, State::new(text.to_owned())))
            .into();
    let query = State::new("total".to_owned());
    let content = move |id: &str| -> AnyView {
        match id {
            "files" => Box::new(files().padding(8.0)),
            "search" => Box::new(
                VStack::new((
                    TextField::new(query.binding()).placeholder("Search"),
                    Text::new("2 results in 1 file").style(TextStyle::Caption),
                    Text::new("sales.rs · pub fn total(&self) -> f64"),
                    Text::new("main.rs · println!(\"{}\", order.total());"),
                ))
                .spacing(8.0)
                .padding(12.0),
            ),
            "git" => Box::new(
                VStack::new((
                    Text::new("Changes").style(TextStyle::Caption),
                    Text::new("M  sales.rs"),
                    Button::new("Commit").style(ButtonStyle::Primary),
                ))
                .spacing(8.0)
                .padding(12.0),
            ),
            "terminal" => Box::new(console(TERMINAL)),
            "output" => Box::new(console(OUTPUT)),
            "problems" => Box::new(
                VStack::new((
                    Text::new("sales.rs:14 · unused variable `customer`"),
                    Text::new("main.rs:6 · this value is never read"),
                ))
                .spacing(6.0)
                .padding(12.0),
            ),
            "outline" => Box::new(
                TreeView::new(vec![
                    TreeNode::new("order", "struct Order")
                        .child(TreeNode::new("new", "fn new"))
                        .child(TreeNode::new("total", "fn total")),
                ])
                .padding(8.0),
            ),
            document => match sources.get(document) {
                Some(source) => {
                    Box::new(CodeEditor::new(source.binding()).language(CodeLanguage::Rust).fill())
                }
                None => Box::new(Text::new(document.to_owned())),
            },
        }
    };
    // The rail shows which view of the side bar is in front, and brings another.
    let (reads, writes) = (layout.clone(), layout.clone());
    let side = Binding::new(
        move || {
            let now = reads.get();
            SIDE.iter().position(|id| now.is_active(id)).unwrap_or(0)
        },
        move |index: usize| {
            if let Some(id) = SIDE.get(index) {
                writes.update(|now| {
                    now.activate(id);
                });
            }
        },
    );
    let rail = ToolRail::new(
        side,
        [
            (IconName::Folder, "Explorer"),
            (IconName::Search, "Search"),
            (IconName::GitBranch, "Source control"),
        ],
    )
    .icons_only(true);
    let area = DockArea::new(layout.binding(), panels(), content).label("Workbench");
    let status = StatusBar::new()
        .left((StatusBar::action("main"), StatusBar::text("0 errors, 2 warnings")))
        .right((StatusBar::text("Ln 14, Col 9"), StatusBar::text("UTF-8"), StatusBar::text("Rust")));
    VStack::new((
        TitleBar::new("sales.rs — shop"),
        HStack::new((rail, area)).spacing(0.0).grow(1.0).align(Align::Stretch),
        status,
    ))
    .spacing(0.0)
    .grow(1.0)
    .align(Align::Stretch)
}
