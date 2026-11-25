# AttachmentStrip

Files about to go with a message, with size, how to send each one and a way to take it out.

**Family:** Forms · **Rust:** `ravenrs::prelude::AttachmentStrip`

## Stories

- **Example**: 420 × 60 pt, settings `{}`

## Sizing

Hugs its content: 420 × 60 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 412 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Remove report.pdf": fill transparent → ink@7.
- **Focus** on "Remove report.pdf": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Remove report.pdf": fill transparent → ink@12, rect [390.0,4.0,22.0,22.0] → [390.33,4.33,21.34,21.34].

## Behavior

- **Events**: `on_remove`.
- **Accessibility**: role `List` "Attachments".
- **Focus order** (Tab): Button "Remove report.pdf" → Button "Send as photo" → Button "Remove beach.jpg".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Remove report.pdf" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |
| Example | Button "Send as photo" | 121.0 × 22.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink | chevron-down 12.0px ink | space-2 |

## API

Builder methods: `new`, `on_remove`.

## Example

```rust
fn attachment_strip() -> impl View {
    // The app holds the list, and changes it when told.
    AttachmentStrip::new([
        Attachment::new("report.pdf").size(120_000),
        Attachment::new("beach.jpg").size(2_400_000).icon(IconName::Image).send_as(
            ["photo", "document"],
            0,
            |_how| {},
        ),
    ])
    .on_remove(|_index| {})
    .width(420.0)
}
```

Every number above is in `AttachmentStrip.json` beside this card.
