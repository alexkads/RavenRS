# InputOtp

Separate boxes for a one-time code.

**Family:** Forms · **Rust:** `ravenrs::prelude::InputOtp`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `code` | text | `` | What was typed so far. |
| `disabled` | on/off | `false` | Locked while the code is verified: no typing, no completing again. |

## Stories

- **Example**: 304 × 52 pt, settings `{"code":"","disabled":false}`
- **Verifying**: 304 × 52 pt, settings `{"code":"482913","disabled":true}`

## Sizing

Hugs its content: 304 × 52 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Verification code" does not change.
- **Focus** on "Verification code": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Verification code": border {color:control-border,width:1.0} → {color:accent,width:1.5}.

## Behavior

- **Events**: `on_complete`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `TextInput` "Verification code".
- **Focus order** (Tab): TextInput "Verification code".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Verification code" | 304.0 × 52.0 | — | — | — | — | — | — |
| Verifying | TextInput "Verification code" | 304.0 × 52.0 | — | — | — | Inter 600 20.0px ink@45 | — | 15.0 |

## API

Builder methods: `disabled`, `new`, `on_complete`.

## Example

```rust
fn input_otp() -> impl View {
    let code = State::new(String::new());
    InputOtp::new(code.binding(), 6)
}
```

Every number above is in `InputOtp.json` beside this card.
