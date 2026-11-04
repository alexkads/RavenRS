# Finding out why a screen is slow

This guide is for anyone using RavenRS. Budgets, and how the library keeps to them, are in [QUALITY.md](QUALITY.md).

## Open the panel

Press Ctrl+Shift+D, or Cmd+Shift+D on a Mac, in any app. On the desktop, `RAVENRS_DIAG=1` also opens it at start. While it is closed, none of this costs anything. While it is open, it also measures itself, so expect slightly higher numbers.

## Read it in this order

**1. Latest frames.** Each bar is a frame, split by where its time went: views, layout, paint, GPU work and waiting. The line across is the 120 Hz budget. Bars under the line are fine. If only a few spikes cross it, look at what you were doing at that moment.

**2. Why the screen was rebuilt.** Each rebuild runs every view on the screen, so its cost follows the size of the screen. This list says what caused them:

| Cause | What to check |
|---|---|
| Data changed | A `State` is being set more often than it changes. Use `set_if_changed`. |
| Pointer moved | Normal for views that ask `is_hovered`. Do not ask unless the view looks different. |
| A view's timer | `wake_at` rebuilds everything. For drawing that moves, use `Paint::Live`, which repeats only the painting. |
| Scrolled | Expected for a virtualized list, not for plain content. |

**3. Costliest views.** The types of view that took the longest in the latest build, without counting the views inside them. Your own components appear under their own names. If one is at the top, look for work done in `build` that could happen once.

**4. Draw calls split by.** Text and shapes alternating split them by kind, which is normal. Many splits by image mean many separate images on screen.

**5. GPU resources.** Atlas resets should stop once the screen has been shown. If they keep rising, too many different glyph sizes are in use.

## Take a capture

"Copy capture for Perfetto" copies the latest frames. Save them to a `.json` file and open it at <https://ui.perfetto.dev> to see every phase on a timeline, or attach it to a bug report.

## What makes a screen cheap

- **Rebuilds are the expensive thing.** They cost about 3 µs per element on a recent laptop, most of it in layout. Scrolling and animating do not rebuild, unless a view asks to.
- **What is off screen costs little.** It is built and laid out but not painted, so a long list scrolls smoothly. Beyond about a thousand elements, use a virtualized view such as `DataGrid`, because each rebuild still runs them all.
- **Text is shaped once.** The same text in the same style is reused between rebuilds, so showing a changing number costs little.
- **Charts are pictures.** Their cost comes when their data or size changes. One that returns unchanged is not drawn again.

## Measuring in this repository

See [the runbook](../.claude/skills/ravenrs-desempenho/SKILL.md).
