//! The stories of `ConfirmDialog`. Reviewed by hand: it opens only when the app asks, so each
//! story asks as it draws, to show the dialog open, in each of the three ways it asks.

use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Button, ButtonStyle, VStack};

use crate::ConfirmDialog;

fn render(args: &Args) -> AnyView {
    let confirm = ConfirmDialog::new();
    let (title, message, action) = (args.text("title"), args.text("message"), args.text("action"));
    if args.bool("open") {
        match args.choice("kind") {
            "Plainly" => confirm.ask_plainly(title.clone(), message.clone(), action.clone(), || {}),
            "Typing" => {
                confirm.ask_typing(title.clone(), message.clone(), action.clone(), args.text("word"), || {})
            }
            _ => confirm.ask(title.clone(), message.clone(), action.clone(), || {}),
        }
    }
    let asker = confirm.clone();
    Box::new(VStack::new((
        Button::new(action.clone()).style(ButtonStyle::Danger).on_click(move || {
            asker.ask(title.clone(), message.clone(), action.clone(), || {});
        }),
        confirm,
    )))
}

impl ComponentBook for ConfirmDialog {
    fn book() -> Meta {
        Meta::new("Actions and inputs/ConfirmDialog", render)
            .summary("Asks before an action that cannot be undone, or one worth a second look.")
            .arg(
                ArgType::bool("open", true)
                    .description("Asked as the story draws; off, only the button that asks."),
            )
            .arg(ArgType::choice("kind", &["Danger", "Plainly", "Typing"], "Danger").description(
                "Danger for what cannot be undone; Plainly for what can; Typing asks for a word first.",
            ))
            .arg(ArgType::text("title", "Delete this record?"))
            .arg(ArgType::text("message", "This cannot be undone."))
            .arg(ArgType::text("action", "Delete").description("The button that confirms; a verb."))
            .arg(ArgType::text("word", "DELETE").description("What Typing asks to be typed."))
            .story("Danger", Args::new())
            .story(
                "Plainly",
                Args::new()
                    .with("kind", ArgValue::choice("Plainly"))
                    .with("title", "Finish this task?")
                    .with("message", "You can open it again later.")
                    .with("action", "Finish"),
            )
            .story("Typing", Args::new().with("kind", ArgValue::choice("Typing")))
            .story("Closed", Args::new().with("open", false))
    }
}
