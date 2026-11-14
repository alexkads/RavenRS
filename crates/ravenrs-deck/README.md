# ravenrs-deck

Headless presentation engine for [RavenRS](https://github.com/alexkads/RavenRS): a slide model
with masters, layouts, shapes, text, tables, charts, animations and transitions; reading and
writing of `.pptx` and of a native format; slides drawn to pixels on the CPU; PDF export with
notes pages and handouts; embedded audio and video probed and given poster frames; and the
commands, selection and undo history an editor drives. Nothing here depends on a window, so it
runs on servers and in WebAssembly too.

```rust
use ravenrs_deck::{model, pptx, render};

// A new presentation, saved as PPTX, read back and its first slide drawn to a PNG.
let deck = model::defaults::new_presentation(None);
let bytes = pptx::export(&deck).unwrap();
let again = pptx::import(&bytes).unwrap();
let opts = render::RenderOpts { scale: 0.25, ..Default::default() };
let png = render::render_slide(&again, 0, &opts).to_png();
assert!(png.starts_with(b"\x89PNG"));
```

The engine comes from [DeckCraft](https://github.com/storytold/deckcraft), with the
[craft-libs](https://github.com/storytold/craft-libs) crates it uses and krilla copied in as
modules; see `NOTICE`.

Licensed under MIT or Apache-2.0.
