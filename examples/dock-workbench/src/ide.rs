//! An IDE: a menu bar and a toolbar over the work, the toolbox and the servers hidden at the left
//! edge, the project tree over the properties at the right, the list of errors, the output and the
//! call stack below, and a search window floating over the code.

use std::collections::HashMap;

use ravenrs::prelude::*;

use crate::console;

const ORDER: &str = r#"use crate::customer::Customer;

/// An order, with its customer and its lines.
pub struct Order {
    pub customer: Customer,
    pub lines: Vec<Line>,
}

impl Order {
    pub fn total(&self) -> f64 {
        self.lines.iter().map(Line::amount).sum()
    }
}
"#;

const CUSTOMER: &str = r#"/// Who buys.
pub struct Customer {
    pub name: String,
    pub tax_id: String,
}
"#;

const STARTUP: &str = r#"fn main() {
    let app = shop::App::new();
    app.run();
}
"#;

const OUTPUT: &str = "Build started...\n1>------ Build started: Project: Shop, Configuration: Debug ------\n1>   Compiling shop v0.1.0\n1>error[E0425]: cannot find type `Line` in this scope\n========== Build: 0 succeeded, 1 failed ==========";

fn start() -> DockLayout {
    let mut layout = DockLayout::new()
        .center(DockNode::tabs(["order.rs", "customer.rs", "main.rs"]))
        .left(DockNode::tabs(["toolbox", "servers"]), 240.0)
        .right(DockNode::vertical([DockNode::tabs(["solution"]), DockNode::tabs(["properties"])]), 300.0)
        .bottom(DockNode::tabs(["errors", "output", "stack"]), 200.0)
        .top(DockNode::tabs(["find"]), 120.0);
    layout.auto_hide("toolbox");
    layout.auto_hide("servers");
    layout.float("find", Rect::new(560.0, 70.0, 360.0, 190.0));
    layout
}

fn panels() -> Vec<DockPanel> {
    let document = |id: &str| DockPanel::new(id, id).kind(DockPanelKind::Document).icon(IconName::Code);
    vec![
        DockPanel::new("toolbox", "Toolbox").icon(IconName::Package),
        DockPanel::new("servers", "Server Explorer"),
        document("order.rs").unsaved(true),
        document("customer.rs"),
        document("main.rs"),
        DockPanel::new("solution", "Solution Explorer").icon(IconName::Layers),
        DockPanel::new("properties", "Properties").icon(IconName::Settings),
        DockPanel::new("errors", "Error List").icon(IconName::CircleAlert),
        DockPanel::new("output", "Output"),
        DockPanel::new("stack", "Call Stack"),
        DockPanel::new("find", "Find and Replace").icon(IconName::Search),
    ]
}

/// The IDE, as a whole window.
pub fn ide() -> impl View {
    let layout = State::new(start());
    let sources: HashMap<&'static str, State<String>> =
        [("order.rs", ORDER), ("customer.rs", CUSTOMER), ("main.rs", STARTUP)]
            .map(|(id, text)| (id, State::new(text.to_owned())))
            .into();
    let (find, replace) = (State::new("total".to_owned()), State::new(String::new()));
    let content = move |id: &str| -> AnyView {
        match id {
            "toolbox" => Box::new(
                VStack::new((Text::new("Button"), Text::new("Text field"), Text::new("Data grid")))
                    .spacing(8.0)
                    .padding(12.0),
            ),
            "servers" => Box::new(Text::new("Data connections").padding(12.0)),
            "solution" => Box::new(
                TreeView::new(vec![
                    TreeNode::new("shop", "Solution 'Shop' (1 project)").child(
                        TreeNode::new("project", "Shop")
                            .icon(IconName::Package)
                            .child(TreeNode::new("order.rs", "order.rs").icon(IconName::Code))
                            .child(TreeNode::new("customer.rs", "customer.rs").icon(IconName::Code))
                            .child(TreeNode::new("main.rs", "main.rs").icon(IconName::Code)),
                    ),
                ])
                .padding(8.0),
            ),
            "properties" => Box::new(
                VStack::new((
                    PropertyRow::new("File name", "order.rs"),
                    PropertyRow::new("Build action", "Compile"),
                    PropertyRow::new("Encoding", "UTF-8"),
                    PropertyRow::new("Full path", "src/order.rs"),
                ))
                .spacing(4.0)
                .padding(12.0),
            ),
            "errors" => Box::new(
                VStack::new((
                    Text::new("E0425 · cannot find type `Line` in this scope · order.rs · line 7"),
                    Text::new("W0001 · unused import · customer.rs · line 1"),
                ))
                .spacing(6.0)
                .padding(12.0),
            ),
            "output" => Box::new(console(OUTPUT)),
            "stack" => Box::new(Text::new("Not debugging").padding(12.0)),
            "find" => Box::new(
                VStack::new((
                    TextField::new(find.binding()).placeholder("Find"),
                    TextField::new(replace.binding()).placeholder("Replace"),
                    HStack::new((Button::new("Find next"), Button::new("Replace all"))).spacing(8.0),
                ))
                .spacing(8.0)
                .padding(12.0),
            ),
            document => match sources.get(document) {
                Some(source) => {
                    Box::new(CodeEditor::new(source.binding()).language(CodeLanguage::Rust).fill())
                }
                None => Box::new(Text::new(document.to_owned())),
            },
        }
    };
    let menus = Menubar::new()
        .menu("File")
        .item(MenuItem::new("New project"))
        .item(MenuItem::new("Save all").shortcut("Ctrl+Shift+S"))
        .menu("Edit")
        .item(MenuItem::new("Undo").shortcut("Ctrl+Z"))
        .menu("View")
        .menu("Project")
        .menu("Build")
        .item(MenuItem::new("Build solution").shortcut("Ctrl+Shift+B"))
        .menu("Debug")
        .menu("Tools")
        .menu("Window")
        .menu("Help");
    let tools = Toolbar::new()
        .action(ToolbarAction::new("New").icon(IconName::Plus))
        .action(ToolbarAction::new("Save").icon(IconName::Save))
        .separator()
        .action(ToolbarAction::new("Undo").icon(IconName::Undo2))
        .action(ToolbarAction::new("Redo").icon(IconName::Redo2))
        .separator()
        .action(ToolbarAction::new("Start").icon(IconName::Play));
    let area = DockArea::new(layout.binding(), panels(), content).label("Workbench");
    let status = StatusBar::new()
        .left((StatusBar::text("Ready"),))
        .right((StatusBar::text("Ln 10, Col 5"), StatusBar::text("INS")));
    VStack::new((menus, tools, area, status)).spacing(0.0).grow(1.0).align(Align::Stretch)
}
