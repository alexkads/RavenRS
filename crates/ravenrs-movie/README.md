# ravenrs-movie

Training videos of a RavenRS app, small enough to ship inside it.

A film here has no pixels. It stores what the app drew, the same rectangles and shaped text the
renderer gets, so it stays sharp at any size and zoom, and a minute costs kilobytes instead of
megabytes. Built-in fonts are referenced, not copied.

**Recording** runs the real app from a script. The script clicks by the names screen readers see,
so it needs no GPU and no permission from the system, and it runs again when the screens change.

```rust,ignore
use ravenrs_movie::Recorder;

let mut take = Recorder::new(app, Theme::raven_light(), Size::new(1200.0, 760.0));
take.chapter("A new expense", "How to write down what you spent.");
take.note("Balance", "Your balance", "The sum of all accounts.", 2.0)?;
take.guide("New", "Open the menu", "Click New to begin.")?;
take.click("New")?;
take.type_into("Description", "Groceries")?;
std::fs::write("manual.ravenmovie", take.finish()?.to_bytes())?;
```

**Editing** is non-destructive, in the style of a screen-recording editor. A `Project` is a list of
clips and of overlays on tracks: cuts, speed, title cards, callouts, captions, spotlight, hiding
what should not show, zoom and pan, fades and a highlight around the pointer. `MovieEditor` is its
timeline, canvas and properties. The edits travel with the film, so there is nothing to render.

**Watching** is `MoviePlayer`, with chapters, a position bar and steps that wait for a click:

| What the script says | What it becomes in the player |
|---|---|
| `chapter` | An entry in the list of chapters |
| `note` | A mark on the screen whose text appears while it plays, and again on a click |
| `guide` | The film stops and waits for a click on that spot |

There is no sound: RavenRS has no audio yet.
