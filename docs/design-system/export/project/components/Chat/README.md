# Chat

A conversation: messages, a field to write, examples to start and an answer on its way.

**Family:** Forms · **Rust:** `ravenrs::prelude::Chat`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `placeholder` | text | `Ask something` |  |
| `intro` | text | `It does not know who answers: yours adds to the messages, now or later.` |  |
| `conversation` | on/off | `false` | Messages already there; off, the intro and the examples. |
| `busy` | on/off | `false` | An answer on its way. |

## Stories

- **Empty**: 413 × 380 pt, settings `{"busy":false,"conversation":false,"intro":"It does not know who answers: yours adds to the messages, now or later.","placeholder":"Ask something"}`
- **Conversation**: 332 × 380 pt, settings `{"busy":false,"conversation":true,"intro":"It does not know who answers: yours adds to the messages, now or later.","placeholder":"Ask something"}`
- **Answering**: 332 × 380 pt, settings `{"busy":true,"conversation":true,"intro":"It does not know who answers: yours adds to the messages, now or later.","placeholder":"Ask something"}`

## Sizing

Hugs its content: 413 × 380 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Empty:

- **Hover** on "How do I start?": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "How do I start?": adds a rect (border 2.0 focus-ring).
- **Pressed** on "How do I start?": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,56.0,119.0,32.0] → [1.78,56.48,115.43,31.04].

## Behavior

- **Events**: `on_link`, `on_listen`, `on_send`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Group`.
- **Focus order** (Tab): Button "How do I start?" → Button "Where are my files?" → TextInput "Ask something" → Button "Send".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Empty | Label "It does not know who answers: yours adds to the messages, now or later." | 401.0 × 36.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Empty | Button "How do I start?" | 119.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Empty | TextInput "Ask something" | 365.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Empty | Button "Send" | 40.0 × 40.0 | radius-md | accent | — | — | send 16.0px on-accent | space-3 |
| Conversation | Label "Where are my files?" | 141.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Conversation | Status | 320.0 × 52.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Conversation | Label "In Documents, sorted by the day you added them." | 320.0 × 44.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Answering | Button "Send" | 40.0 × 40.0 | radius-md | accent@45 | — | — | send 16.0px on-accent@45 | space-3 |

## API

Builder methods: `busy`, `examples`, `intro`, `new`, `on_link`, `on_listen`, `on_send`, `placeholder`.

`ChatAuthor`: `Assistant`, `Person`.

## Example

```rust
fn chat() -> impl View {
    /// Keeps the messages, here while the page rebuilds. A real app keeps them in its state.
    struct Conversation;

    impl View for Conversation {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let messages = cx.use_state(Vec::<ChatMessage>::new);
            let answers = messages.clone();
            let chat = Chat::new(messages.binding())
                .placeholder("Ask something")
                .intro("It does not know who answers: yours adds to the messages, now or later.")
                .examples(["How do I start?", "Where are my files?"])
                .on_send(move |text| {
                    // Marks, links and a file in the bubble are asked for message by message.
                    let reply = ChatMessage::assistant(format!("*You asked:* {text}"))
                        .formatted()
                        .media(ChatMedia::document("answer.pdf").size(48_000).on_open(|| {}))
                        .card(|| Box::new(Button::new("Open it").size(ButtonSize::Small)));
                    answers.update(|all| all.push(reply));
                });
            cx.child(&VStack::new((chat,)).height(380.0).width(420.0))
        }
    }
    Conversation
}
```

Every number above is in `Chat.json` beside this card.
