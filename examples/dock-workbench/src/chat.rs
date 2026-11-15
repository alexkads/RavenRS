//! A chat app: the conversations in a side bar that collapses to its strip, the conversation in the
//! middle, and what the assistant made in a panel at the right.

use ravenrs::prelude::*;

const DRAFT: &str = r#"# Monthly sales report

| Region | Sales | vs. last month |
|--------|------:|---------------:|
| North  | 128   | +12%           |
| South  | 94    | -3%            |
| East   | 151   | +21%           |
"#;

fn start() -> DockLayout {
    DockLayout::new()
        .center(DockNode::tabs(["conversation"]))
        .left(DockNode::tabs(["chats"]), 260.0)
        .right(DockNode::tabs(["artifact"]), 420.0)
}

fn panels() -> Vec<DockPanel> {
    vec![
        DockPanel::new("chats", "Chats").icon(IconName::MessageSquare).closable(false),
        DockPanel::new("conversation", "Sales report").closable(false),
        DockPanel::new("artifact", "Report draft").icon(IconName::FileText),
    ]
}

/// The chat app, as a whole window.
pub fn chat() -> impl View {
    let layout = State::new(start());
    let messages = State::new(vec![
        ChatMessage::person("Can you turn last month's sales into a short report?"),
        ChatMessage::assistant(
            "Here is a draft with the three regions and how each did against the month before. \
             It is open beside the conversation, so you can edit it there.",
        ),
        ChatMessage::person("Great. Can you say which region grew the most?"),
        ChatMessage::assistant("East grew the most, 21%, followed by North with 12%. South fell 3%."),
    ]);
    let draft = State::new(DRAFT.to_owned());
    let content = move |id: &str| -> AnyView {
        match id {
            "chats" => Box::new(
                VStack::new((
                    Button::new("New chat").icon(IconName::Plus).style(ButtonStyle::Primary),
                    Text::new("Today").style(TextStyle::Caption),
                    Text::new("Sales report"),
                    Text::new("Trip to the coast"),
                    Text::new("Yesterday").style(TextStyle::Caption),
                    Text::new("Rust lifetimes"),
                    Text::new("Birthday party ideas"),
                ))
                .spacing(10.0)
                .padding(12.0),
            ),
            "conversation" => Box::new(Chat::new(messages.binding()).placeholder("Reply…")),
            "artifact" => Box::new(
                CodeEditor::new(draft.binding()).language(CodeLanguage::Plain).line_numbers(false).fill(),
            ),
            other => Box::new(Text::new(other.to_owned())),
        }
    };
    let area = DockArea::new(layout.binding(), panels(), content)
        .tab_bar(DockTabBar::WhenSeveral)
        .label("Workbench");
    VStack::new((TitleBar::new("Raven Chat"), area)).spacing(0.0).grow(1.0).align(Align::Stretch)
}
