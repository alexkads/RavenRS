# RavenComponentBook

RavenComponentBook is the book of every RavenRS component, kept the way Storybook keeps stories. It has one mission: give a design tool, such as Claude Design, everything it needs to design screens that RavenRS builds exactly as they were drawn.

## Why it exists

A design made "with the RavenRS components" is only buildable when the designer knows which components exist, what each one can be set to, and how big, round, filled and spaced RavenRS draws it. Before the book, the design system in Claude Design had three components and no measurements, so designs came back with pieces the library does not have.

## How it works

1. **Each component gives its stories.** A component implements `ravenrs::widgets::book::ComponentBook`, next to its code, in the `book` module of its crate (feature `book`). Its `Meta` says where it sits in the book (`"Actions and inputs/Button"`), its settings (`ArgType`: text, on/off, number or choice, each with a default) and its named states (`Story`: Primary, Disabled, All sizes). This is the `*.stories` file of Storybook, in Rust.
2. **The book measures, it does not photograph.** It draws each story with no window and reads what RavenRS laid out and painted: the accessibility tree (role, name, state and frame of every part) and the display list (each rectangle with its corners, fill, border and shadow, each run of text with its font, weight, size, color and the place of every letter, each icon and path). Every color is named by the theme's token (`accent`, `bg-300`, `accent@45` for the accent at 45%).
3. **It writes what the design tool reads.**
   - `--json <folder>`: one `<Component>.json` per component (settings, API, code, every story in Raven Dark and Raven Light, anatomy) and an `index.json`, as Storybook's.
   - `--claude-design <folder>`: the files of the Design System type under `<folder>/project/`: a guide that says to compose only these components, a card per component with its settings and the anatomy of every story (sizes, radius, fill, border, text, icon, inner space), a vector preview drawn back from the measurements with colors as `var(--token)`, the JSON, the tokens and the RavenIcons font.
4. **Components that give no stories yet** are listed by their example in the component gallery, measured the same way, and marked as pending, until each implements `ComponentBook`.

## Use it

```bash
cargo run -p raven-component-book
```

```bash
cargo run -p raven-component-book -- --claude-design target/design-system --sha "$(git rev-parse --short HEAD)"
```

```bash
cargo run -p raven-component-book -- --json target/component-book
```

The window lists the components by family, with a search; the story on screen is the real component, live; its settings change it at once (Controles), its measurements are below it (Medidas) and so is its code (Código). The status bar says how many components give their stories and how many are pending.

## Give a component its stories

```rust
impl ComponentBook for Badge {
    fn book() -> Meta {
        Meta::new("Display and feedback/Badge", badge)
            .summary("Short label for a state or a count, in a tone that says what it means.")
            .arg(ArgType::text("text", "New"))
            .arg(ArgType::choice("tone", TONES, "Accent"))
            .story("Accent", Args::new())
            .story("Danger", Args::new().with("tone", ArgValue::choice("Danger")).with("text", "Overdue"))
    }
}
```

List it in the `stories()` of the crate's `book` module. The book's tests check that a story only sets settings the component declares.

## Limits

- 14 components give their stories so far; the rest are pending and show one story, their gallery example, with no settings.
- The book writes the files; publishing them to the design system in Claude Design is done with the Artifact tool (or by hand).
- Previews are measured at 760 pt wide in Raven Dark; Raven Light is measured too and is in the JSON.
