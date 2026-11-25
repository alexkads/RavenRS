# SidebarPage

A page beside its navigation: the sidebar on the left, the page header and the content on the same margin.

**Family:** Patterns · **Rust:** `ravenrs::prelude::SidebarPage`

## Stories

- **Pattern**: 712 × 420 pt, settings `{}`

## Sizing

Fills the width of its parent: 272, 248, 248 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Navigation` "Navigation".
- **Focus order** (Tab): Button "Overview" → Button "Customers" → Button "Settings" → Button "New customer".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Pattern | Navigation "Navigation" | 248.0 × 420.0 | 0.0 | bg-100 | — | Inter 600 12.0px ink-muted | layout-dashboard 16.0px ink-muted | 22.0 |
| Pattern | Button "Overview" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | layout-dashboard 16.0px ink-muted | 10.0 |
| Pattern | Button "Customers" | 224.0 × 36.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | users 16.0px mix(ink, accent, 35) | 10.0 |
| Pattern | Button "Settings" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | settings 16.0px ink-muted | 10.0 |
| Pattern | Heading "Customers" | 117.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Pattern | Label "Everyone who bought this year." | 194.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Pattern | Button "New customer" | 166.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | plus 16.0px on-accent | 18.0 |
| Pattern | Label "The content of the page." | 384.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

Every number above is in `SidebarPage.json` beside this card.
