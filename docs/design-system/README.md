# RavenRS

Beautiful native interfaces, powered by Rust. RavenRS draws every pixel itself on the GPU (wgpu), on the desktop and in the browser (WebAssembly + WebGPU), so this system is the single source of truth for how a RavenRS app looks everywhere. The Rust crate `raven-style` mirrors these tokens value for value; when a token changes here, it changes there.

## Principles

- **Calm surfaces, one sheen.** Raven feathers read black until light hits them and a blue-violet sheen appears. Interfaces work the same way: deep neutral surfaces (`bg-000` to `bg-300`) and a single accent (`accent`) reserved for the one action or selection that matters on a screen.
- **Depth from light, not lines.** Cards separate from the canvas through fill (`bg-200`) and a soft shadow (`shadow-soft`). The `border` hairline supports that edge; it is never the only cue.
- **Legible first.** Text meets 4.5:1 on its ground in every theme (`ink` 15:1+, `ink-muted` 5.5:1+), focus is always visible (`focus-ring`), and hit targets are at least 32px tall (44px on touch).
- **Motion that explains.** Springs move things that change place; color tweens (`duration-fast`) acknowledge input. With reduced motion requested, springs collapse to instant changes and fades.

## Themes

| Theme | Use it for | Notes |
|---|---|---|
| Raven Dark | The default. Tools, editors, dashboards used for hours. | Neutrals carry a slight blue bias toward the accent. |
| Raven Light | Documents, forms, bright environments. | Shadows are fainter; `border` does more of the edge work. |
| Raven Glass | Overlays, sidebars and hero cards over rich backgrounds. | Surfaces are translucent white over `bg-000`. Background blur is applied by the renderer where the platform supports it and degrades to the translucent fill otherwise. |

## Color

- `bg-000` is the canvas. `bg-100` holds panels and sidebars, `bg-200` cards, `bg-300` controls.
- `ink` for content, `ink-muted` for labels and metadata. Never put `ink-muted` on `bg-300` for body text.
- `accent` fills the primary button and marks selection. Text on it is always `on-accent`. Hover and press shift lightness (`accent-hover`, `accent-pressed`), not hue.
- `success`, `warning` and `danger` signal state only and always come with a label or icon.

## Typography

Inter, embedded in every build (the web has no system fonts to fall back on), at two weights: 400 and 600. Scale: `metric` 42, `display` 32, `title` 22, `headline` 17, `body` 15, `callout` 13, `caption` 12. Numbers in `metric` use tabular figures. `caption` is uppercase with +0.4px tracking.

## Space and shape

Spacing is a 4px grid: `space-1` (4) through `space-12` (48). Screens pad by `space-8`, cards by `space-6`, stacks default to `space-4`. Radii grow with the size of the object: `radius-md` (10) for buttons, `radius-xl` (24) for cards, `radius-full` for pills.

## Motion

| Use | Rust API (planned, Motion Engine) | Parameters |
|---|---|---|
| Hover, press, focus color | `Transition::fast()` | `duration-fast`, ease-out |
| Things changing place or size | `Spring::snappy()` | stiffness 380, damping 30 |
| Panels and sheets | `Spring::gentle()` | stiffness 170, damping 26 |

## Components

| Component | Status in RavenRS |
|---|---|
| Text | Implemented |
| Button | Implemented (primary; hover, pressed, focus states) |
| Card | Implemented (rounded fill, soft shadow, hairline) |
| GlassCard | Planned (translucent fill ships with Raven Glass; blur needs the compositor) |

## Writing

Name things the way people see them: "Get started", "Save changes", "Revenue". Buttons start with a verb. Errors say what happened and how to fix it.
