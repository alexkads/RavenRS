# RavenRS

Beautiful native interfaces, powered by Rust. RavenRS draws every pixel itself on the GPU (wgpu), on the desktop and in the browser (WebAssembly + WebGPU), so this system is the single source of truth for how a RavenRS app looks everywhere. The Rust crate `ravenrs-style` mirrors these tokens value for value; when a token changes here, it changes there.

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

A generated application has a theme menu in the footer of its navigation, unless it asks for `AppShell::without_theme_menu`. `ThemeEditor` changes colors, corners and text sizes by the token names below, and warns when text falls under 4.5:1. What it changes is kept as JSON holding only the differences from the base theme. Spacing is not offered: components still take it from a shared constant, so editing it would change only part of a screen.

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

| Use | Token (`theme.motion`) | Rust API | Parameters |
|---|---|---|---|
| Hover, press, focus color | `fast` | `Transition::fast()` | `duration-fast`, ease-out |
| Small layout and opacity changes | `base` | `Transition::base()` | `duration-base`, ease-in-out |
| Things changing place or size | `snappy` | `Transition::snappy()` | stiffness 380, damping 30 |
| Panels and sheets | `gentle` | `Transition::gentle()` | stiffness 170, damping 26 |

Rules for components:

- Use `theme.motion`, not numbers of your own, and declare motion with `cx.motion`.
- Exits are shorter than entrances: the user has already moved on.
- Scale subtly (0.96 and up). Popups grow from their anchor, with `Motion::anchored`.
- A surface with content stays solid while it moves: dialogs, menus and panels never fade. Opacity applies to each shape on its own, so a fading panel shows the page through its fields. Only its backdrop fades.
- A row in a virtualized list has no enter animation, because scrolling would set it off.

## Components

| Family | Components |
|---|---|
| Text and icons | `Text`, `Icon`, `Label`, `Kbd`, `Badge` |
| Themes | `ThemeSwitcher`, `ThemePicker`, `ThemeEditor` |
| Actions | `Button`, `IconButton`, `ToggleButton`, `ToggleGroup`, `DropdownMenu`, `Ribbon`, `Menubar` |
| Forms | `TextField`, `Select`, `Combobox`, `DatePicker`, `Calendar`, `Checkbox`, `RadioGroup`, `Toggle`, `Slider`, `Stepper`, `SegmentedControl`, `InputOtp`, `ColorSwatch`, `FormField` |
| Surfaces | `Card`, `GlassCard` (translucent fill; blur needs the compositor), `Section`, `Divider`, `Splitter`, `Accordion`, `Disclosure` |
| Navigation | `Sidebar`, `SidebarItem`, `Tabs`, `Breadcrumb`, `Pagination`, `CommandPalette` |
| Overlays | `Dialog`, `Sheet`, `Popover`, `Tooltip`, `HoverCard`, `ContextMenu`, `Toaster` |
| Data | `DataGrid`, `Table`, `List`, `Stat`, `Chart`, `AreaChart`, `BarChart`, `Barcode`, `Avatar`, `Image`, `Carousel` |
| Feedback | `Alert`, `ProgressBar`, `Skeleton`, `Spinner` |

Charts take their series from `chart-1` to `chart-5`, which each theme defines to read on its background. They mean nothing on their own, unlike `success` and `danger`, so a series never looks like a warning. Use a status color for a value only when it says that status, such as a budget over its limit.

Modules of RavenRS itself, such as the report designer, follow the same rule as any app: they use these components and tokens, and what is missing joins the library first. Printed paper is the one surface that stays light in every theme, drawn with Raven Light tokens.

## Screens

These rules came out of reviewing `examples/feature-center` screen by screen, and they hold for every component and application.

- **One action stands out.** A window accents the one action that matters most: Save on a form, New on a list. Everything else in the bar stays a ghost button.
- **One page margin.** Title, commands, description and content all start on the same line, `space-4` from the page's edge.
- **No space by accident.** Stacks default to `space-4`. Put spacing in on purpose where pieces of a window meet, or it shows up as an empty band under a toolbar or a gap beside the navigation.
- **Title or trail.** One level is a page title in `headline`. A breadcrumb appears only when there is somewhere to go back to.
- **Fields fill their column.** References, colors and date with time take the same width as the text fields around them. A fixed width is a choice made at the call site.
- **Say it once.** A collection in a tab has no heading or label of its own, and a nested list leaves out the column of its owner. An `Alert` has no title when one would only repeat where it is: use `Alert::message`.
- **Grids show data first.** The filter row is flat, with each filter's text starting where its column's does, at the end for numbers. The group panel shows only when something is grouped or when asked for with `group_panel(true)`; the Group menu is always in the bar.
- **Nothing moves under the pointer.** Selecting a row does not change the height of what is above it. Prefer replacing a status for adding to it.
- **One piece, no frame.** A dashboard with a single item has no tile around it. What only helps while handling something, such as a tile's resize corner, shows only under the pointer.
- **One language.** Every label follows the `Labels` of the application, including those in report viewers.

## Writing

Name things the way people see them: "Get started", "Save changes", "Revenue". Buttons start with a verb. Errors say what happened and how to fix it.
