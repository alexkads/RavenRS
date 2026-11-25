# TextField

Single-line text input with placeholder, icon, prefix, suffix and a clear button.

**Family:** Forms · **Rust:** `ravenrs::prelude::TextField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `value` | text | `` |  |
| `placeholder` | text | `Full name` |  |
| `size` | Mini · Small · Medium · Large | `Medium` |  |
| `icon` | none · plus · check · trash-2 · pencil · search · settings · download | `none` | An icon at the start. |
| `prefix` | text | `` | Words before the text, as a currency. |
| `suffix` | text | `` | A badge after the text, as a unit. |
| `clearable` | on/off | `false` | A button that empties it. |
| `secure` | on/off | `false` | Hides what is typed, for passwords. |
| `flat` | on/off | `false` | No fill or border, inside a grid or a toolbar. |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Empty**: 728 × 40 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":false,"secure":false,"size":"Medium","suffix":"","value":""}`
- **Filled**: 728 × 40 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":false,"secure":false,"size":"Medium","suffix":"","value":"Ana Lima"}`
- **With icon**: 728 × 40 pt, settings `{"clearable":true,"disabled":false,"flat":false,"icon":"search","placeholder":"Search","prefix":"","read_only":false,"secure":false,"size":"Medium","suffix":"","value":""}`
- **Price**: 728 × 32 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"$","read_only":false,"secure":false,"size":"Small","suffix":"per month","value":"49,90"}`
- **Password**: 728 × 40 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Password","prefix":"","read_only":false,"secure":true,"size":"Medium","suffix":"","value":"secret"}`
- **Disabled**: 728 × 40 pt, settings `{"clearable":false,"disabled":true,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":false,"secure":false,"size":"Medium","suffix":"","value":"Ana Lima"}`
- **Read only**: 728 × 40 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":true,"secure":false,"size":"Medium","suffix":"","value":"Ana Lima"}`
- **Mini**: 728 × 22 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":false,"secure":false,"size":"Mini","suffix":"","value":""}`
- **Large**: 728 × 48 pt, settings `{"clearable":false,"disabled":false,"flat":false,"icon":"none","placeholder":"Full name","prefix":"","read_only":false,"secure":false,"size":"Large","suffix":"","value":""}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Empty:

- **Hover** on "Full name": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Full name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Full name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_cancel`, `on_leave`, `on_submit`.
- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `TextInput` "Full name".
- **Focus order** (Tab): TextInput "Full name".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Empty | TextInput "Full name" | 728.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Filled | TextInput "Full name" | 728.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| With icon | TextInput "Search" | 728.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | search 16.0px ink-muted | space-3 |
| Price | TextInput "Full name" | 728.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-2 |
| Price | Label "$" | 10.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Price | Label "per month" | 64.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 0.0 |
| Disabled | TextInput "Full name" | 728.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Read only | TextInput "Full name" | 728.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Mini | TextInput "Full name" | 728.0 × 22.0 | radius-md | bg-300 | 1.0 control-border | Inter 600 12.0px ink-muted | — | space-2 |
| Large | TextInput "Full name" | 728.0 × 48.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |

## API

Builder methods: `autofocus`, `clearable`, `compact`, `disabled`, `end_aligned`, `flat`, `icon`, `intercept_keys`, `keeps_focus`, `new`, `on_cancel`, `on_leave`, `on_submit`, `placeholder`, `prefix`, `read_only`, `recall`, `secure`, `select_on_open`, `size`, `suffix`, `width`.

## Example

```rust
fn text_field() -> impl View {
    let name = State::new(String::new());
    let price = State::new(String::from("49,90"));
    let shown = name.clone();
    VStack::new((
        TextField::new(name.binding()).placeholder("Full name").icon(IconName::User).clearable(),
        TextField::new(price.binding())
            .placeholder("Price")
            .size(ButtonSize::Small)
            .prefix(Text::new("R$").foreground(Semantic::InkMuted))
            .suffix(Badge::new("per month")),
        Text::dynamic(move || format!("Hello, {}", shown.get())).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `TextField.json` beside this card.
