# Sidebar

Navigation column with sections, items and counts.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Sidebar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `open` | on/off | `true` | Open with names; closed, as the collapsible mode leaves it (icons only, or hidden). |
| `collapsible` | Fixed · Icons · Hidden | `Icons` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `side` | Left · Right | `Left` |  |
| `variant` | Standard · Floating · Inset | `Standard` |  |

## Stories

- **Example**: 438 × 420 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Left","variant":"Standard"}`
- **Collapsed**: 250 × 420 pt, settings `{"collapsible":"Icons","label":"","open":false,"side":"Left","variant":"Standard"}`
- **Collapsible: Fixed**: 438 × 420 pt, settings `{"collapsible":"Fixed","label":"","open":true,"side":"Left","variant":"Standard"}`
- **Collapsible: Hidden**: 438 × 420 pt, settings `{"collapsible":"Hidden","label":"","open":true,"side":"Left","variant":"Standard"}`
- **Side: Right**: 438 × 420 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Right","variant":"Standard"}`
- **Variant: Floating**: 438 × 412 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Left","variant":"Floating"}`
- **Variant: Inset**: 438 × 420 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Left","variant":"Inset"}`
- **Compact, with sections**: 248 × 442 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Left","variant":"Standard"}`
- **Items with a state and an action**: 248 × 304 pt, settings `{"collapsible":"Icons","label":"","open":true,"side":"Left","variant":"Standard"}`

## Sizing

Hugs its content: 248 × 420 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Customers": fill transparent → ink@5; color ink-muted → ink; fill mix(bg-300, control-border, 97) → mix(ink, bg-300, 7); fill mix(bg-300, control-border, 97) → mix(ink, bg-300, 7).
- **Focus** on "Overview": adds a rect (border 2.0 focus-ring); color mix(ink, accent, 35) → ink-muted; color mix(ink, accent, 35) → ink-muted; fill mix(bg-300, control-border, 97) → mix(ink, bg-300, 7).
- **Pressed** on "Customers": fill transparent → ink@5; color ink-muted → ink; fill mix(bg-300, control-border, 97) → mix(ink, bg-300, 7); fill mix(bg-300, control-border, 97) → mix(ink, bg-300, 7).

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Enter`, `Space`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Navigation` "Navigation".
- **Focus order** (Tab): Button "Overview" → Button "Customers" → Button "Settings" → Button "Projects" → Button "Toggle sidebar".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Navigation "Navigation" | 248.0 × 420.0 | 0.0 | bg-100 | — | Inter 600 12.0px ink-muted | layout-dashboard 16.0px mix(ink, accent, 35) | 22.0 |
| Example | Button "Overview" | 224.0 × 36.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | layout-dashboard 16.0px mix(ink, accent, 35) | 10.0 |
| Example | Button "Customers" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | users 16.0px ink-muted | 10.0 |
| Example | Button "Settings" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | settings 16.0px ink-muted | 10.0 |
| Example | Button "Projects" | 224.0 × 28.0 | radius-md | transparent | — | Inter 600 12.0px ink-muted | chevron-down 14.0px ink-muted | 10.0 |
| Example | Image "Ana Lima" | 36.0 × 36.0 | radius-full | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 14.0px mix(ink, focus-ring, 50) | — | 9.0 |
| Example | Label "Ana Lima" | 57.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Toggle sidebar" | 32.0 × 32.0 | radius-md | transparent | — | — | panel-left 14.0px ink | 9.0 |
| Example | Label "Or press Cmd/Ctrl+B." | 153.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Collapsed | Navigation "Navigation" | 60.0 × 420.0 | 0.0 | bg-100 | — | Inter 600 14.0px mix(ink, focus-ring, 50) | layout-dashboard 16.0px mix(ink, accent, 35) | 21.0 |
| Collapsed | Button "Overview" | 36.0 × 36.0 | radius-md | bg-300 | — | — | layout-dashboard 16.0px mix(ink, accent, 35) | 10.0 |
| Collapsed | Button "Customers" | 36.0 × 36.0 | radius-md | transparent | — | — | users 16.0px ink-muted | 10.0 |
| Collapsed | Button "Settings" | 36.0 × 36.0 | radius-md | transparent | — | — | settings 16.0px ink-muted | 10.0 |
| Variant: Floating | Navigation "Navigation" | 232.0 × 404.0 | radius-lg | bg-100 | 1.0 border | Inter 600 12.0px ink-muted | layout-dashboard 16.0px mix(ink, accent, 35) | 22.0 |
| Variant: Inset | Navigation "Navigation" | 248.0 × 420.0 | — | — | — | Inter 600 12.0px ink-muted | layout-dashboard 16.0px mix(ink, accent, 35) | 22.0 |
| Compact, with sections | Navigation "Navigation" | 248.0 × 442.0 | 0.0 | bg-100 | — | Inter 600 15.0px ink | wallet 16.0px ink | 17.0 |
| Compact, with sections | Label "Raven" | 45.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Compact, with sections | Label "Family finances" | 96.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Compact, with sections | Button "Search commands" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |
| Compact, with sections | Label "DAY TO DAY" | 76.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Compact, with sections | Button "Overview" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | layout-dashboard 16.0px ink-muted | space-2 |
| Compact, with sections | Button "Transactions" | 224.0 × 32.0 | radius-md | bg-300 | — | Inter 600 13.0px ink | list 16.0px mix(ink, accent, 35) | space-2 |
| Compact, with sections | Button "Bills" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | credit-card 16.0px ink-muted | space-2 |
| Compact, with sections | Button "Reports" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | chart-pie 16.0px ink-muted | space-2 |
| Compact, with sections | Button "Settings" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | settings 16.0px ink-muted | space-2 |
| Compact, with sections | Button "Fold menu" | 224.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | panel-left 16.0px ink-muted | space-2 |
| Items with a state and an action | Navigation "Navigation" | 248.0 × 304.0 | 0.0 | bg-100 | — | Inter 600 12.0px ink-muted | plus 14.0px ink | 22.0 |
| Items with a state and an action | Button "New workspace" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Items with a state and an action | Button "shop-api" | 188.0 × 36.0 | — | — | — | Inter 600 15.0px ink | layers 16.0px mix(ink, accent, 35) | 10.0 |
| Items with a state and an action | Button "Delete shop-api" | 32.0 × 32.0 | radius-md | transparent | — | — | trash-2 14.0px ink | 9.0 |
| Items with a state and an action | Button "website" | 188.0 × 36.0 | — | — | — | Inter 400 15.0px ink-muted | layers 16.0px ink-muted | 10.0 |
| Items with a state and an action | Button "Planner" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | sparkles 16.0px ink-muted | 10.0 |
| Items with a state and an action | Button "Builder" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | code 16.0px ink-muted | 10.0 |
| Items with a state and an action | Button "Tests" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | terminal 16.0px ink-muted | 10.0 |

## API

Builder methods: `collapsible`, `footer`, `header`, `label`, `narrow_below`, `new`, `rail`, `shortcut`, `side`, `state`, `variant`, `width`.

`SidebarCollapse`: `Fixed`, `Hidden`, `Icons`.

`SidebarVariant`: `Floating`, `Inset`, `Standard`.

## Example

```rust
fn sidebar() -> impl View {
    let state = SidebarState::new(true);
    HStack::new((
        Sidebar::new((
            SidebarGroup::new(
                "Workspace",
                (
                    SidebarItem::new(IconName::LayoutDashboard, "Overview").selected(true),
                    SidebarItem::new(IconName::Users, "Customers").badge(Badge::new("12")),
                    SidebarItem::new(IconName::Settings, "Settings")
                        .item(SidebarItem::text("General").compact())
                        .item(SidebarItem::text("Billing").compact()),
                ),
            ),
            SidebarGroup::new("Projects", (SidebarSkeleton::new(2),)).collapsible(),
        ))
        .collapsible(SidebarCollapse::Icons)
        .state(state.clone())
        .footer(
            HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                .spacing(12.0)
                .align(Align::Center),
        ),
        VStack::new((SidebarTrigger::new(state), Text::new("Or press Cmd/Ctrl+B.")))
            .spacing(12.0)
            .padding(16.0)
            .grow(1.0),
    ))
    .height(420.0)
    .align(Align::Stretch)
}
```

Every number above is in `Sidebar.json` beside this card.
