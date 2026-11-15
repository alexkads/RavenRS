# flow-chat-bot

An automation builder for chat conversations, made only with library components (`FlowCanvas`, `FlowToolbox`, `FlowInspector`, `FlowLibrary` and more). It is here to check that the diagram component is enough for this kind of product.

```bash
cargo run -p flow-chat-bot
```

## What it shows

- **Blocks by group** (triggers, messages, logic, actions) in a `FlowToolbox`, dragged onto the conversation or placed with Enter.
- **A question with one way out per answer**: each button of a message is an output port with a name, shown by the port, and each leads somewhere else.
- **Rules on what joins what** with `FlowCanvas::validate`: nothing goes into a trigger, an action ends the way. The reason shows by the pointer.
- **The next block from the keyboard** with `next_node` (Cmd/Ctrl+Enter, or the arrow by a block): a message after anything but an action.
- **Properties beside it** in a `FlowInspector` sharing the canvas's selection and history, so a change there is undone like one made on the canvas.
- **Saving the conversation** as PNG, SVG or PDF with `ravenrs-flow-export`.

## What it needed, now in the library

Building it showed what the component lacked, and each piece went into the library first:
port names shown by the ports (one way out per answer), `FlowToolbox::title`, the run statuses
`FlowNodeStatus::Running` and `Success` that the test run marks, and `FlowLibrary` to keep and
open the conversations. Trying a conversation runs here a step at a time along the first way
out; a real bot would answer through the app.

## In the browser, on Linux and on Windows

In the browser, the diagrams are kept in the browser's own storage:

```bash
scripts/build-web.sh flow-chat-bot
python3 -m http.server -d examples/flow-chat-bot/web 8080
```

On Linux and Windows, run `cargo run -p flow-chat-bot --release` and check that you can drag a block from the left onto the conversation; join two blocks; Testar conversa, then Próximo passo until the end; Salvar, close and open again: the conversation is there.
