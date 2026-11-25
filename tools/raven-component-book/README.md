# RavenComponentBook

RavenComponentBook (RCB) is the book of every RavenRS component, kept the way Storybook keeps stories, and the guardian of the RavenRS design system. It has one mission: give a design tool, such as Claude Design, everything it needs to design screens that RavenRS builds exactly as they were drawn.

## The guardian

The design system is written whole by RCB, from the code, and the design tool takes it as it is: nothing is changed or improved on the way, and the design tool's page is locked (`"editing": "code"`). Anyone who wants a change reviews the components or the tokens:

- a component changes through its stories (`ComponentBook`) or the component itself;
- a color, a size, a corner, a font or a motion changes in `ravenrs-style`, where RCB reads every token.

RCB keeps it honest: `scripts/rcb-coverage.py` refuses a component without `ComponentBook`, a test refuses `docs/design-system/tokens.json` when it disagrees with `ravenrs-style` or leaves a token without words, and `--verify` proves a published copy is exactly what RCB writes. The copy everyone reads is in git, in `docs/design-system/export/`: `scripts/gen-design-system.py` rewrites it, `scripts/check.sh` refuses it out of date when a change reaches RCB (`--check`), and the Claude Design project is published from it, not from an export made on the spot (#1000).

## Why it exists

A design made "with the RavenRS components" is only buildable when the designer knows which components exist, what each one can be set to, and how big, round, filled and spaced RavenRS draws it. Before the book, the design system in Claude Design had three components and no measurements, so designs came back with pieces the library does not have.

## How it works

1. **Each component gives its stories.** A component implements `ravenrs::widgets::book::ComponentBook`, next to its code, in the `book` module of its crate (feature `book`). Its `Meta` says where it sits in the book (`"Actions and inputs/Button"`), its settings (`ArgType`: text, on/off, number or choice, each with a default) and its named states (`Story`: Primary, Disabled, All sizes). This is the `*.stories` file of Storybook, in Rust.
2. **The book measures, it does not photograph.** It draws each story with no window and reads what RavenRS laid out and painted: the accessibility tree (role, name, state and frame of every part) and the display list (each rectangle with its corners, fill, border and shadow, each run of text with its font, weight, size, color and the place of every letter, each icon and path). Every color is named by the theme's token (`accent`, `bg-300`, `accent@45` for the accent at 45%, `mix(ink, border, 25)` for a color between two tokens).
   - **Three themes**: Raven Dark (the reference), Raven Light and Raven Glass.
   - **Three widths**: each story is laid out in parents of 320, 760 (the reference) and 1200 pt, and in one that stretches its children, so its card says whether it fills its parent, hugs its content or takes a form column (`Sizing`). What measuring cannot tell, the `ComponentBook` declares with `Meta::sizing`.
   - **Interaction**: on the first control of the first story, the book brings the pointer over it, the focus with Tab and the button held down, and says which tokens change.
   - **Behavior**: it presses the arrows, Enter, Space, Escape, Home and End on the focused control and lists the keys that change what it shows, with the events (`on_*`), the accessibility role and the focus order.
3. **It writes what the design tool reads.**
   - `--json <folder>`: one `<Component>.json` per component (settings, API, code, every story in the three themes, anatomy, `sizing`, `interaction` and `behavior`) and an `index.json`, as Storybook's.
   - `--claude-design <folder>`: the whole design system under `<folder>/project/`: the index (locked to the code), a guide that says to compose only these components and how to change the system, the tokens read from `ravenrs-style` with their CSS, the Inter and RavenIcons fonts, a cover drawn from the tokens, and per component a card with its settings, its *Sizing*, *Interaction* and *Behavior*, and the anatomy of every story (sizes, radius, fill, border, text, icon, inner space, for the controls and the panels, titles and labels around them), a vector preview drawn back from the measurements with colors as `var(--token)`, and the JSON. `<folder>/files.txt` lists every file; a published file not in it is removed.
   - `--verify <folder>`: compares a copy of what is published with what RCB writes, and lists what changed, is missing or should not be there.
   - `--replay <screen.rcb.json>`: checks a screen drawn with the design system. The design marks each component with `data-rcb="Component/Story"` and the settings it changed in `data-rcb-args`; a script in the artboard (the guide gives it) saves every marked element with its box. RCB builds each one with its story and settings in a parent as wide as the box and lists what RavenRS would not build at that size (1 pt of tolerance, 3% for the width of what hugs a label), or the component, story, setting or option that does not exist. A setting `icon` takes any icon the card of `Icon` lists, not only the sample its control offers. It exits with 1 when something diverges. `tests/fixtures/fase0-novo-lancamento.rcb.json` is the screen of phase 0 of #768.
4. **The themes of the applications** come in too. An application writes each of its themes with `Theme::to_tokens_json` (Raven Finanças: `examples/family-finance/design/*.theme.json`, kept equal to its code by a test of its own), `tokens::APP_THEMES` lists the files, and the design system has them as `data-theme` ids beside the three of RavenRS.
5. **One language per story.** Component stories are in English, the system's language; the stories of the `Application` family are in Portuguese, as the applications that use it, with the `Labels`, the `Words` and what is announced in the same language.
6. **Every component gives its stories**, the widgets, the window of an application (`AppShell`) and the film components; a part of another component (the target line, `ChartReference`) is one of its stories. A component that gives none would be listed by its gallery example and marked pending, and `pendentes.json` would have to name its issue.

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

List it in the `stories()` of the crate's `book` module and take it out of `pendentes.json`. The book's tests check that a story only sets settings the component declares, and `scripts/rcb-coverage.py`, in `check.sh`, refuses a public component that neither implements `ComponentBook` nor is listed in `pendentes.json` with its issue (#754). It also refuses one whose stories leave out what the design tool needs (#768): a story for each option of a choice of up to 12 (an icon sample excepted, a story drawn its own way named after the setting, as "All sizes", counts for all), a story with each state it has (`disabled`, `read_only`, `loading`, `error`) on, and at least one story. What does not have them yet is listed with `"conteudo": true` and its issue; `--self-test` checks these rules on made-up sources.

## Limits

- It writes files and verifies a copy; publishing them to the design system artifact is done with the Artifact tool, sending the whole folder and removing what `files.txt` does not list.
- Previews draw Raven Dark at 760 pt, the first story beside its 320 pt version and the states it reacts to; Raven Light, Raven Glass and the other widths are in the JSON.
- Interaction and keys are measured on the first story's first control only; a key whose effect the story does not show (Enter on a button that only calls `on_click`) is not listed.

## End-to-end tests (Raven Test Studio)

The scenario of the book is in `tests/e2e/`, as a `.raventest` file the Raven Test Studio reads: it searches for TextField and sees its stories, then picks Button, changes its label and checks the story draws it again. The book brings it (`SCENARIOS`), and `tests/suite/scenarios.rs` plays it on every `cargo test`.

```bash
raven-test-studio --project tools/raven-component-book --run-all --no-cache --format json --out target/e2e/raven-component-book
```

The book answers the Studio when opened with `--e2e-serve`, with no window, no updates and no self-test in the status bar; its name on the channel is "RavenComponentBook" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`).

