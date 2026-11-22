# ravenrs-e2e

End-to-end tests for RavenRS screens. A test runs the real view with no window, finds a part of the screen the way a person or a screen reader would, acts on it with real input events, and checks what changed. When a step fails, the error says why with a stable code, shows the screen as it was and lists what came close, so the failure explains itself without running the test again.

This is the session of the end-to-end tool planned in [#627](https://github.com/alexkads/RavenRS/issues/627). The scenario files and the channel to an app in another process are in [`ravenrs-e2e-wire`](../ravenrs-e2e-wire), and the Studio in [`tools/raven-test-studio`](../../tools/raven-test-studio).

## How it finds things

Everything is found in the accessibility tree, the same one screen readers read, so a test does not depend on how a component is built inside, and a screen that is accessible is also testable.

| Query | Finds | Use it when |
|---|---|---|
| `Query::test_id("save")` | the node the app tagged with `.test_id("save")` | the part matters to a test: it survives reordering, wrapping and new wording |
| `Query::role(Role::Button).name("Save")` | the button named "Save" | reading the test should read like the screen |
| `Query::text("Total")` | the node whose name, or value when it is not secret, is "Total" | checking what is shown |
| `Query::by_ref("e7")` | the node a snapshot showed as `[ref=e7]` | a tool or an agent picked it from the snapshot |

`within(parent)` looks only inside another node, such as one dialog, and `nth(i)` picks one when several match. There is no query by position on the screen.

A test id on a plain container, one that screen readers do not see, goes to the first accessible node inside it. Components can tag their own parts with `BuildCx::test_id`.

`test_id` is a `ViewExt` modifier, like `padding` or `tooltip`: it wraps the view, so it goes last, after the component's own methods: `Button::new("Save").on_click(save).test_id("save")`.

## How it acts

Actions are real input events at the middle of the node, so they take the same path as a person's: `click`, `double_click`, `right_click`, `hover`, `type_text` (it clicks the node first when it does not have the focus), `press` and `drag_to`. Before acting, the session checks that the node responds to input, has a size inside the window and is not under something else; a node that fails any of these is not acted on.

While a modal dialog is open, only the dialog is in the tree. A query for something behind it fails with `E2E_COVERED`, naming the dialog.

## How it checks

`locator.expect()` gives `to_have_value`, `to_have_name`, `to_be_checked`, `to_be_disabled`, `to_be_enabled`, `to_be_focused`, `to_be_visible` and `to_have_count`. A secret value (a password field) is never compared or shown: `to_have_value` fails with `E2E_PROTECTED_VALUE`, and the snapshot writes `value=protected`.

## The screen as text

`E2e::snapshot()` writes one line per node, children indented under their parents:

```text
- text_input "Name" #name [ref=e1] value="Ana"
- button "Save" #save [ref=e2] focused
```

Each node keeps its reference for the whole session. It is how a report, a log or a coding agent reads the screen without an image.

## Error codes

| Code | Meaning |
|---|---|
| `E2E_NOT_FOUND` | nothing matches the query |
| `E2E_AMBIGUOUS` | several match and the query does not say which |
| `E2E_DISABLED` | the target does not respond to input |
| `E2E_COVERED` | something else is on top, or a modal covers the screen |
| `E2E_NOT_VISIBLE` | outside the window, or no size |
| `E2E_PROTECTED_VALUE` | the value is secret |
| `E2E_EXPECTATION` | a check did not hold |

## Example

```rust,no_run
use ravenrs_core::Key;
use ravenrs_e2e::{E2e, E2eResult};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, TextField, VStack, ViewExt};

fn renaming() -> E2eResult {
    let name = State::new(String::new());
    let mut e2e = E2e::new(
        VStack::new((
            TextField::new(name.binding()).placeholder("Name").test_id("name"),
            Button::new("Save").test_id("save"),
        )),
        Theme::raven_dark(),
    );
    e2e.by_test_id("name").type_text("Anna")?;
    e2e.by_test_id("name").press(Key::Backspace)?;
    e2e.by_test_id("name").expect().to_have_value("Ann")?;
    e2e.by_test_id("save").click()?;
    Ok(())
}
```

## Waiting, and actions past a click

Time is the session's ([#658](https://github.com/alexkads/RavenRS/issues/658)). It moves only with `E2e::advance(seconds)`, a frame at a time, so animations run and timers fire the same on every machine; with no `advance`, an animation never starts. `E2e::wait_until(timeout, check)` looks again until the check holds, moving the clock with real time between looks, so a timer that fires and work done on another thread (which wakes the screen with its `UiWaker`) both show. When time runs out it fails with `E2E_TIMEOUT` and the screen of the last look. `E2e::patiently(patience, step)` tries a step again only while waiting could still fix it (not found, covered, disabled, out of sight, or a check that does not hold yet). When time runs out, it fails with the step's own code. `Locator::wait_for` and `wait_to_mention` are the short forms.

A node marked busy (`BuildCx::busy`, or `busy_while` with a `BusySignal` for work away from the screen) is waited for before any action: first on the session's clock, so work that ends on a timer passes with no real wait, then up to 5 s in real time. Still busy, the action fails with `E2E_TIMEOUT`, saying it was busy. The snapshot writes `busy` on it.

Beyond clicks and keys, a locator can:

- `fill(text)`: put the text in a field in place of what it has, or move a slider or spin button to a number with the arrow keys;
- `select(option)`: open a combo box or menu and pick the option, or pick it inside a list, tab list or group shown in place;
- `expand()` and `collapse()`: open or close an item, a tree item with the arrow keys, a group row with a click; a part that says nothing about being open fails with `E2E_EXPECTATION`;
- `set_value(text)`, `show_menu()`, `dismiss()` and `perform(action)`: ask the node for an action it declares with `BuildCx::on_access`, as assistive technology does, with no typing or clicking. A node that does not declare it fails with `E2E_DISABLED`, naming the action. `expand()`, `collapse()` and `select()` use the declared action when there is one, and input as a person gives it when there is not;
- `scroll_into_view()`: turn the wheel over the part that scrolls until the node's middle is in sight. It also works for a row a long list has not built yet: it scrolls each list to its end and back until the row shows.

Rows with keys have test ids that stay with the data when it sorts, filters or scrolls:

- `row:<key>` for a `VirtualList` with `key`;
- `row:<value of the key column>` for a `DataGrid` with `key_column`;
- `item:<key>` for a `TreeView` item;
- `node:<id>` for a `FlowCanvas` node.

## Watching a session

`E2e::observe` hands a `FrameObserver` every frame the session draws, each pointer move, click and key, and each step a test marks with `E2e::mark_step`. `ravenrs-movie`'s `SessionFilm` uses it to film a test, and its `Recorder` scripts a training on a session with the raw input (`pointer_to`, `mouse_down`, `mouse_up`, `key_down`, `scroll_at`, `focus`, `frame_at`) and `find_all`. With no observer nothing is called and nothing is kept.

`E2e::primitives()` reads the current display list without advancing time or changing the observer. It is what the channel's frame adapter encodes with the same `SessionFilm` writer; protected fields are already hidden in the drawing.

## Finding a target's declaration

Enable `e2e-source` in the app under test (`ravenrs`, `ravenrs-widgets`, `ravenrs-e2e` and `ravenrs-e2e-wire` expose the feature). `E2e::source(&query)` returns the static Rust caller location of `ViewExt::test_id` or `BuildCx::test_id`. It follows the test id when a container passes it to its first accessible child, and a child's own id keeps its own origin. Without an explicit test tag, `Button::new` and `Text::{new,bound,dynamic}` record their constructor's caller. Queries by role, text or position use that same recorded location. An explicit tag takes priority. The origin stays the same when the screen is reordered or reactive text changes.

Without the feature, no location is stored. A target without a recorded origin fails with `E2E_NO_SOURCE`; missing or ambiguous targets keep their normal query error. Origins are metadata in the UI's test map, not fields read by screen readers or included in the text snapshot. Other untagged constructors are not instrumented yet; their implementation location is never guessed. Paths are as recorded by the compiler and may reflect path remapping; the editor integration comes later.

## Limits for now

- It runs in the test's process. An app in another process is driven through [`ravenrs-e2e-wire`](../ravenrs-e2e-wire).
- `fill` clears a field with Command+A and Backspace; a field that does not take the shortcut keeps its text.

It depends only on `ravenrs-core`, `ravenrs-style` and `ravenrs-widgets-base`, never on a widget family, and builds for `wasm32-unknown-unknown`.
