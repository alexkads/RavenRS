# flow-chat-bot

An automation builder for chat conversations, made only with the library's `FlowCanvas`, `FlowToolbox` and `FlowInspector`. It is here to check that the diagram component is enough for this kind of product.

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

## What the component did not cover

Each of these is an issue of its own:

- A block cannot show a list of its own inside it, such as the text of each button as a row with its way out beside it. Today the port's name stands in for the row.
- A block cannot be told to run, so the conversation cannot be tried in place. That belongs to the app, not to the canvas, but a way to mark the block running (a status) and the way taken would help.
