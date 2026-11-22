//! The stories of `Chat`. Written by hand: the gallery's example keeps its messages inside a view
//! of its own, which the scaffold does not take apart.

use ravenrs_reactive::State;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, VStack};

use crate::{Chat, ChatMessage};

fn render(args: &Args) -> AnyView {
    let messages = if args.bool("conversation") {
        vec![
            ChatMessage::person("Where are my files?"),
            ChatMessage::assistant("In *Documents*, sorted by the day you added them.").formatted(),
        ]
    } else {
        Vec::new()
    };
    let messages = State::new(messages);
    let chat = Chat::new(messages.binding())
        .placeholder(args.text("placeholder"))
        .intro(args.text("intro"))
        .examples(["How do I start?", "Where are my files?"])
        .busy(args.bool("busy"));
    Box::new(VStack::new((chat,)).height(380.0).width(420.0))
}

impl ComponentBook for Chat {
    fn book() -> Meta {
        Meta::new("Forms/Chat", render)
            .summary(
                "A conversation: messages, a field to write, examples to start and an answer on its way.",
            )
            .arg(ArgType::text("placeholder", "Ask something"))
            .arg(ArgType::text(
                "intro",
                "It does not know who answers: yours adds to the messages, now or later.",
            ))
            .arg(
                ArgType::bool("conversation", false)
                    .description("Messages already there; off, the intro and the examples."),
            )
            .arg(ArgType::bool("busy", false).description("An answer on its way."))
            .story("Empty", Args::new())
            .story("Conversation", Args::new().with("conversation", true))
            .story("Answering", Args::new().with("conversation", true).with("busy", true))
    }
}
