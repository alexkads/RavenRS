# CommandPalette

Searchable list of commands, opened with Cmd/Ctrl+K.

**Family:** Overlays · **Rust:** `ravenrs::prelude::CommandPalette`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `open` | on/off | `false` | How the example starts. |

## Stories

- **Example**: 153 × 40 pt, settings `{"open":false,"placeholder":""}`
- **Open**: 644 × 276 pt, settings `{"open":true,"placeholder":""}`

## Sizing

Hugs its content: 153 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Open palette": fill accent → accent-hover.
- **Focus** on "Open palette": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open palette": fill accent → accent-pressed, rect [0.0,0.0,153.0,40.0] → [2.29,0.6,148.41,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Open palette".
- **Focus order** (Tab): Button "Open palette".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Open palette" | 153.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | command 16.0px on-accent | 18.0 |
| Open | Dialog "Command palette" | 560.0 × 152.0 | radius-lg | bg-200 | 1.0 border | Inter 400 15.0px ink-muted | search 16.0px ink-muted | 22.0 |
| Open | TextInput "Command palette" | 540.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | search 16.0px ink-muted | space-3 |
| Open | Label "PAGES" | 540.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Open | ListBoxOption "Go to reports" | 540.0 × 38.0 | 8.0 | accent@16 | — | Inter 400 15.0px ink | file-text 16.0px ink-muted | 10.0 |
| Open | ListBoxOption "Open settings" | 540.0 × 38.0 | 8.0 | transparent | — | Inter 400 15.0px ink | settings 16.0px ink-muted | 10.0 |

## API

Builder methods: `command`, `new`, `placeholder`.

## Example

```rust
fn command_palette() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Open palette").icon(IconName::Command).on_click(move || show.set(true)),
        CommandPalette::new(open.binding())
            .command(Command::new("Go to reports", || {}).icon(IconName::FileText).group("Pages"))
            .command(Command::new("Open settings", || {}).icon(IconName::Settings).group("Pages")),
    ))
}
```

Every number above is in `CommandPalette.json` beside this card.
