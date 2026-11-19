# RavenComponentBook

RavenComponentBook (RCB) is the book of every RavenRS component, kept the way Storybook keeps stories, and the guardian of the RavenRS design system. It has one mission: give a design tool, such as Claude Design, everything it needs to design screens that RavenRS builds exactly as they were drawn.

## The guardian

The design system is written whole by RCB, from the code, and the design tool takes it as it is: nothing is changed or improved on the way, and the design tool's page is locked (`"editing": "code"`). Anyone who wants a change reviews the components or the tokens:

- a component changes through its stories (`ComponentBook`) or the component itself;
- a color, a size, a corner, a font or a motion changes in `ravenrs-style`, where RCB reads every token.

RCB keeps it honest: `scripts/rcb-coverage.py` refuses a component without `ComponentBook`, a test refuses `docs/design-system/tokens.json` when it disagrees with `ravenrs-style` or leaves a token without words, and `--verify` proves a published copy is exactly what RCB writes.

## Why it exists

A design made "with the RavenRS components" is only buildable when the designer knows which components exist, what each one can be set to, and how big, round, filled and spaced RavenRS draws it. Before the book, the design system in Claude Design had three components and no measurements, so designs came back with pieces the library does not have.

## How it works

1. **Each component gives its stories.** A component implements `ravenrs::widgets::book::ComponentBook`, next to its code, in the `book` module of its crate (feature `book`). Its `Meta` says where it sits in the book (`"Actions and inputs/Button"`), its settings (`ArgType`: text, on/off, number or choice, each with a default) and its named states (`Story`: Primary, Disabled, All sizes). This is the `*.stories` file of Storybook, in Rust.
2. **The book measures, it does not photograph.** It draws each story with no window and reads what RavenRS laid out and painted: the accessibility tree (role, name, state and frame of every part) and the display list (each rectangle with its corners, fill, border and shadow, each run of text with its font, weight, size, color and the place of every letter, each icon and path). Every color is named by the theme's token (`accent`, `bg-300`, `accent@45` for the accent at 45%).
3. **It writes what the design tool reads.**
   - `--json <folder>`: one `<Component>.json` per component (settings, API, code, every story in Raven Dark and Raven Light, anatomy) and an `index.json`, as Storybook's.
   - `--claude-design <folder>`: the whole design system under `<folder>/project/`: the index (locked to the code), a guide that says to compose only these components and how to change the system, the tokens read from `ravenrs-style` with their CSS, the Inter and RavenIcons fonts, a cover drawn from the tokens, and per component a card with its settings and the anatomy of every story (sizes, radius, fill, border, text, icon, inner space), a vector preview drawn back from the measurements with colors as `var(--token)`, and the JSON. `<folder>/files.txt` lists every file; a published file not in it is removed.
   - `--verify <folder>`: compares a copy of what is published with what RCB writes, and lists what changed, is missing or should not be there.
4. **Every component gives its stories**, the widgets, the window of an application (`AppShell`) and the film components; a part of another component (the target line, `ChartReference`) is one of its stories. A component that gives none would be listed by its gallery example and marked pending, and `pendentes.json` would have to name its issue.

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

List it in the `stories()` of the crate's `book` module and take it out of `pendentes.json`. The book's tests check that a story only sets settings the component declares, and `scripts/rcb-coverage.py`, in `check.sh`, refuses a public component that neither implements `ComponentBook` nor is listed in `pendentes.json` with its issue (#754).

## Limits

- It writes files and verifies a copy; publishing them to the design system artifact is done with the Artifact tool, sending the whole folder and removing what `files.txt` does not list.
- Previews are measured at 760 pt wide in Raven Dark; Raven Light is measured too and is in the JSON.
