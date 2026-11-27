# RavenRS

Beautiful native interfaces, powered by Rust. RavenRS draws every pixel itself on the GPU (wgpu), on the desktop and in the browser (WebAssembly + WebGPU), so this system is the single source of truth for how a RavenRS app looks everywhere. The Rust crate `ravenrs-style` mirrors these tokens value for value; when a token changes here, it changes there.

This system, as RavenComponentBook measures it (the index, the tokens, the fonts and, per component, its card, its preview and the anatomy of every story), is kept in the RavenRS repository in `docs/design-system/export/project/`, current with the code: `scripts/gen-design-system.py` rewrites it when a component changes, and `scripts/check.sh` refuses it out of date.

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
- `border` is the hairline of dividers and cards, and may be faint. The outline of a field or control at rest (text field, select, checkbox, radio, toggle, stepper) takes `control-border` instead, at 3:1 or more against `bg-000` to `bg-200`, so a field can be found before it is hovered or focused. A theme whose `control-border` falls under that is refused like one whose text is unreadable.
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
- What people fold and unfold many times a day, as a sidebar and its submenus, takes `base`. A spring's long tail reads as hesitation there.
- While a sidebar narrows its edge covers the content, which does not reflow: names never squeeze letter by letter.
- Exits are shorter than entrances: the user has already moved on.
- Scale subtly (0.96 and up). Popups grow from their anchor, with `Motion::anchored`.
- A surface with content comes in solid: opacity applies to each shape on its own, so a panel fading in shows the page through its fields. Leaving, they fade as one surface with `Motion::surface_exit`, content first and panel after. They take `fast`, without a spring, whose tail makes text shiver.
- A row in a virtualized list has no enter animation, because scrolling would set it off.

## Designing with this system

Every screen designed with this system must be buildable with RavenRS as it is. So:

- **Use only the components listed below**, by their names. Each one has a card with its settings, its stories and its anatomy: the sizes, corners, fills, borders, fonts and inner spaces RavenRS draws, measured from the library itself by RavenComponentBook. Draw them with those numbers.
- **Keep the marks.** Each picture in a card carries `data-rcb="Component/Story"`. Put the same attribute on the element that draws that component in a screen, with the story it starts from, the settings you changed in `data-rcb-args` as JSON (`data-rcb-args='{"label":"Cancel"}'`; `icon` takes any name the card of `Icon` lists, not only the sample a card offers), and `data-rcb-screen="<name>"` on the screen's root. RavenRS then checks the screen piece by piece (see *Checking a screen*).
- **Size them as the card says.** Its *Sizing* tells whether a component fills the width of its parent or hugs its content, with the size it takes in parents of 320, 760 and 1200 pt. A field that fills its column stretches with it; a button that hugs its label keeps its width.
- **Change a component only through its settings** (the `argTypes` of its card). A variant, size or state that is not there does not exist.
- **Compose, do not invent.** A piece the screen needs that no component gives goes on a note called *Missing from the library* on the artboard, with what it should do. It joins the library before the screen is built. Before you write a piece on that note, look for its card in the list below: a component that has a card is never missing, even when the story you need is not there yet (then the note asks for the story).
- **Draw each piece as its card does.** A simple piece (button, badge, field, alert, tabs, `Stat`) is drawn as the picture in its card, which is what RavenRS draws; a piece of data (`DataGrid`, `LogView`, `FlowCanvas`, `Scheduler`, `List`, `Sheet`) follows the anatomy of its card with the screen's data. Add no frame, border, header style, color or inner space that the card does not show.
- Colors are tokens (`var(--accent)`), never new colors; spacing follows the 4px grid (`space-*`); corners follow `radius-*`.
- Icons are RavenRS icons, by the names in the cards (`trash-2`, `search`); the `RavenIcons` font draws them.

## Checking a screen

An artboard that keeps the marks can be checked against RavenRS. Run this script in it: it saves `<screen>.rcb.json` with every marked element, its settings and its box, and `raven-component-book --replay <file>` says which pieces RavenRS would not build at that size (1 pt of tolerance; a little more for the width of a label, which a browser lays out wider), ending with `N de M peças divergem (K pontos)`.

**A screen is ready only when its replay finds no divergence.** A divergence that stays is written on the artboard with its reason. The replay checks sizes, not looks: a piece can have the right size and still carry a frame or a color RavenRS does not draw, so compare the artboard with the cards as well.

```js
const root = document.querySelector('[data-rcb-screen]') || document.body;
const origin = root.getBoundingClientRect();
const elements = [...root.querySelectorAll('[data-rcb]')].map((e) => {
  const r = e.getBoundingClientRect();
  return { rcb: e.dataset.rcb, args: JSON.parse(e.dataset.rcbArgs || '{}'),
           box: [r.x - origin.x, r.y - origin.y, r.width, r.height] };
});
const screen = root.dataset.rcbScreen || document.title;
const file = new Blob([JSON.stringify({ screen, elements }, null, 2)], { type: 'application/json' });
const link = Object.assign(document.createElement('a'), { href: URL.createObjectURL(file), download: screen + '.rcb.json' });
link.click();
```

## Changing this system

RavenComponentBook writes every file of this system from the RavenRS code, and the design tool takes them as they are: nothing here is edited by hand. To change a component, change its stories (`ComponentBook`) or the component itself; to change a color, a size, a corner or a font, change the tokens of `ravenrs-style`. Then RavenComponentBook writes the system again.

## Components

| Family | Component | What it is | Stories |
|---|---|---|---|
| Actions and inputs | `Button` | Clickable, focusable button with styles, sizes, icons, loading and selected states | 9 |
| Actions and inputs | `IconButton` | Square, icon-only button for toolbars; its label is read aloud and shown as a tooltip | 6 |
| Actions and inputs | `SegmentedControl` | Picks one of a few options laid side by side on a track | 2 |
| Actions and inputs | `Checkbox` | Check box with its label; clicking the label toggles it | 6 |
| Actions and inputs | `Toggle` | On and off switch for a setting that applies at once | 6 |
| Actions and inputs | `Slider` | Picks a number in a range by dragging, with keys and an optional value at the side | 2 |
| Actions and inputs | `ButtonGroup` | Buttons glued side by side, as one control | 1 |
| Actions and inputs | `ColorPair` | Foreground and background colors, one over the other, with a swap button | 1 |
| Actions and inputs | `ColorPicker` | A color from a palette, a shade square or typed | 3 |
| Actions and inputs | `ColorSwatch` | Round color to pick from a palette | 2 |
| Actions and inputs | `DateRangePicker` | A period in two clicks, or ready-made | 3 |
| Actions and inputs | `FavoriteToggle` | Heart or star that marks a favorite: filled when on | 1 |
| Actions and inputs | `NinePointAnchor` | Three by three grid for a point of reference, moved by the arrow keys | 1 |
| Actions and inputs | `RadioGroup` | One choice among several, shown as radio buttons | 1 |
| Actions and inputs | `ScrollEdgeArrow` | A strip that scrolls, with an arrow at each end that has more to show | 1 |
| Actions and inputs | `Stepper` | Decrement, value and increment for small quantities | 3 |
| Actions and inputs | `SwatchGrid` | Grid of color swatches; the handler gets the keys held with the click | 1 |
| Actions and inputs | `ThemeSwitcher` | Switches between Raven Dark, Raven Light and Raven Glass | 1 |
| Actions and inputs | `TitleBar` | Title, actions and window buttons drawn by the app; does not move the window | 3 |
| Actions and inputs | `ToggleButton` | Button that stays pressed, like bold or mute | 2 |
| Actions and inputs | `ToggleGroup` | Row of toggle buttons where one or several can be on | 1 |
| Actions and inputs | `ConfirmDialog` | Asks before an action that cannot be undone, or one worth a second look | 4 |
| Actions and inputs | `DateTimePicker` | A date and a time over one value | 3 |
| Actions and inputs | `DropZone` | Dashed area around any content, which takes dropped files | 2 |
| Actions and inputs | `ExpressionEditor` | Writes an expression with fields, functions and operators at hand | 1 |
| Actions and inputs | `FileUpload` | Files from the chooser or dropped, with a reason when refused | 3 |
| Actions and inputs | `FilterBuilder` | Builds a filter out of lines, in the expression language | 1 |
| Actions and inputs | `FloatingToolbar` | Translucent row of commands over its content, with a handle that moves it | 1 |
| Actions and inputs | `FormLayout` | Groups of fields in columns, tabs and a list of errors | 5 |
| Actions and inputs | `ImageField` | A picture chosen by the person | 3 |
| Actions and inputs | `InlineRename` | A name that turns into a field by a double click or F2 | 5 |
| Actions and inputs | `LookupField` | Picks a record from a list searched on demand | 3 |
| Actions and inputs | `MaskedField` | Text that follows a pattern, like a phone number | 3 |
| Actions and inputs | `NumberField` | A typed number or amount of money, formatted when you leave | 5 |
| Actions and inputs | `ParameterForm` | A form made from a schema of parameters: groups, lists of rows, options that depend on another field, and a value or an expression in each field | 3 |
| Actions and inputs | `SplitButton` | A main command with a menu of alternatives | 8 |
| Actions and inputs | `StepBadge` | The round mark of a step | 4 |
| Actions and inputs | `StepIndicator` | Where a task in steps stands, for an app's own layout | 1 |
| Actions and inputs | `TagBox` | Several values from a list, shown as tags | 3 |
| Actions and inputs | `TextArea` | Text on several lines, scrolling past its height or growing with the text | 4 |
| Actions and inputs | `ThemeEditor` | Changes the tokens of the theme on screen, with live contrast and JSON to copy and paste | 1 |
| Actions and inputs | `ThemePicker` | A small menu of themes that says which one is on | 1 |
| Actions and inputs | `TimePicker` | A time of day, typed or picked from a list | 3 |
| Actions and inputs | `Toolbar` | Commands that fold into a menu when out of room | 1 |
| Actions and inputs | `ValueSlider` | A value in small that opens a slider or can be typed | 2 |
| Actions and inputs | `Wizard` | A task in steps that can refuse to be left | 1 |
| Forms | `TextField` | Single-line text input with placeholder, icon, prefix, suffix and a clear button | 9 |
| Forms | `Select` | Picks one option from a list that opens under the field, with search when it is long | 8 |
| Forms | `Calendar` | Month grid to pick a date | 1 |
| Forms | `Combobox` | Select with a search box that filters the options | 3 |
| Forms | `DatePicker` | Field that opens a calendar in a popover | 3 |
| Forms | `FormField` | Labeled field with a hint and validation messages | 4 |
| Forms | `InputOtp` | Separate boxes for a one-time code | 2 |
| Forms | `Label` | Form label with an optional required marker and hint | 1 |
| Forms | `StatusBar` | One fixed line at the foot of an app window | 5 |
| Forms | `ToolRail` | A narrow bar of tools with their names under their icons | 1 |
| Forms | `CodeBlock` | A command to copy | 1 |
| Forms | `ModelPicker` | Local AI models to download, try, use and remove | 1 |
| Forms | `ReleaseNotes` | An app's versions and what each one brought | 1 |
| Forms | `ScanField` | Field for a barcode scanner: keeps the focus, tells a scanner from fingers and empties after each code | 2 |
| Forms | `EventCalendar` | A month with its events inside each day | 3 |
| Forms | `RecurrenceEditor` | Edits how something repeats: how often, on which days, until when | 1 |
| Forms | `Scheduler` | Timeline of resources: drag, resize and draw events, zoom the time axis | 4 |
| Forms | `Timeline` | Blocks along time on named tracks | 1 |
| Forms | `AttachmentStrip` | Files about to go with a message, with size, how to send each one and a way to take it out | 1 |
| Forms | `AudioPlayerButton` | Plays and stops a recording, with progress and a transcript to open | 2 |
| Forms | `Chat` | A conversation: messages, a field to write, examples to start and an answer on its way | 3 |
| Forms | `CodeEditor` | Edits code with colors, line numbers, completions and the problems marked where they are | 4 |
| Forms | `FloatingPanel` | A panel that floats over the app in a corner, for an assistant or a chat | 3 |
| Forms | `Playground` | Code beside what it makes, run as it is typed, with examples to start from | 3 |
| Forms | `RichText` | Bold, italic, code and links in plain text, with the marks chat apps use | 1 |
| Forms | `SelectionBar` | How many are chosen, a way out and what to do with them, with Escape to leave | 1 |
| Navigation | `Menubar` | Horizontal bar of menus such as File, Edit and View | 1 |
| Navigation | `Pagination` | Previous, numbered pages with ellipses, and Next | 2 |
| Navigation | `Tabs` | Switches between views of the same place; each tab builds when it is selected | 5 |
| Navigation | `Accordion` | Disclosures separated by hairlines | 2 |
| Navigation | `Breadcrumb` | Path of locations ending at the current page | 1 |
| Navigation | `Disclosure` | Header that shows or hides its content | 2 |
| Navigation | `Sidebar` | Navigation column with sections, items and counts | 9 |
| Navigation | `SidebarBrand` | The name of an app at the top of a sidebar | 1 |
| Navigation | `DocumentTabs` | Tabs for open documents that close, move, rename and say what was not saved | 1 |
| Navigation | `FileBrowser` | Folders and files from the app, with a breadcrumb, sorting and selection | 3 |
| Navigation | `YearView` | The twelve months of a year, with marks on the days | 1 |
| Navigation | `Ribbon` | Office-style command bar with tabs, groups and quick access | 1 |
| Overlays | `CommandPalette` | Searchable list of commands, opened with Cmd/Ctrl+K | 2 |
| Overlays | `ContextMenu` | Menu opened at the pointer on right click | 1 |
| Overlays | `Dialog` | Modal window over a dimmed backdrop | 3 |
| Overlays | `DropdownMenu` | Button that opens a list of actions, with submenus and check marks | 12 |
| Overlays | `HelpTip` | A small question mark that explains something | 1 |
| Overlays | `HoverCard` | Rich content shown while the pointer is over a trigger | 1 |
| Overlays | `Popover` | Floating content anchored below a view | 2 |
| Overlays | `Sheet` | Panel that slides over the UI from one edge | 3 |
| Overlays | `SplashScreen` | The screen an app shows while it opens: its name, an icon and how far it got | 2 |
| Overlays | `Toaster` | Stacked notifications that dismiss themselves | 7 |
| Overlays | `Tooltip` | Short label shown on hover, with an optional key cap | 1 |
| Data | `PatchPanel` | Jacks joined by cables that hang and swing: plug, refuse, take a plug elsewhere, pairs of cords, a front that turns around, all of it with the keys too | 2 |
| Data | `Stat` | Headline number with a label and a change indicator | 11 |
| Data | `CardView` | Records as cards, only those in sight are built | 3 |
| Data | `List` | Bordered list with icons, titles and trailing text | 5 |
| Data | `Table` | Simple table with a header, hover rows and numeric columns | 1 |
| Data | `DataInspector` | JSON items as a table, a tree, the JSON and a schema, with the field chosen kept between them | 1 |
| Data | `LogView` | A build log as it comes: ANSI colors, follows the end, search, sections that fold and lines pointed at by number | 7 |
| Data | `SortableList` | Rows put in order by a drag or with Alt and the arrows | 2 |
| Data | `TreeView` | Items inside items: opens, selects, checks and filters | 1 |
| Data | `VirtualList` | A list of any length with rows of any height, which jumps to a row and stays at the end | 2 |
| Data | `DataGrid` | Virtualized grid for 100,000 rows: sorting, filters, grouping and export | 9 |
| Data | `AreaChart` | Smooth area chart of evenly spaced values | 1 |
| Data | `BarChart` | Bar chart with labels and faint gridlines | 1 |
| Data | `Barcode` | QR Code and linear barcodes that scanners can read | 2 |
| Data | `Chart` | Area, bar, line, pie, doughnut, radar and radial charts in the patterns of the shadcn/ui catalog, with tooltips, keyboard, a focus that glides between categories, lines of reference and a fixed axis; scatter, sparklines and candlesticks too | 4 |
| Data | `ChartCard` | A chart's card: title, description, what changes it, tabs with totals, and how it trends | 2 |
| Data | `FlowCanvas` | Nodes joined by connections on an endless surface: pan, zoom, ports that check what connects, a map and keys for everything | 8 |
| Data | `FlowInspector` | The settings of the node or connection selected on a FlowCanvas | 3 |
| Data | `FlowLayers` | The nodes of a diagram as a list, to find, select and reorder them | 1 |
| Data | `FlowLibrary` | The diagrams kept beside a FlowCanvas: open, save, rename and start new ones | 2 |
| Data | `FlowToolbox` | The blocks a person drags onto a FlowCanvas, in groups, with a search | 3 |
| Data | `SankeyChart` | How much flows from what to what: ribbons between nodes, arrow keys, and a clear message when the flows go in a circle | 2 |
| Layout | `Card` | A grouped surface lifted off the page by its fill and a soft shadow | 7 |
| Layout | `HStack` | Lays its children out in a row, a set space apart | 4 |
| Layout | `VStack` | Lays its children out in a column, a set space apart | 3 |
| Layout | `Anchor` | A place a #name link scrolls to, gliding there | 1 |
| Layout | `AspectRatio` | Keeps content at a width-to-height ratio | 1 |
| Layout | `Collapse` | Animates its content's height as it opens and closes | 2 |
| Layout | `CountUp` | A number that counts up when it first shows on screen | 2 |
| Layout | `Divider` | Hairline separator | 1 |
| Layout | `Footer` | Band that ends a page or app, with left, center and right areas | 2 |
| Layout | `GlassCard` | Translucent card for Raven Glass compositions | 1 |
| Layout | `Grid` | Equal-width columns that wrap into rows | 2 |
| Layout | `Link` | Text that opens an address, or scrolls to an anchor | 2 |
| Layout | `PageHeader` | Title, description, badges and actions at the top of a page | 3 |
| Layout | `PropertyRow` | A label on the left and its value on the right | 3 |
| Layout | `ScrollLinked` | Fades, moves or scales its content by how far the page has scrolled | 1 |
| Layout | `ScrollScene` | A section that holds its content on screen for a length of scrolling, for steps that change as you scroll | 1 |
| Layout | `ScrollView` | Scrolling stack, vertical or horizontal | 2 |
| Layout | `Section` | Titled group of content with an optional description | 2 |
| Layout | `Spacer` | Flexible space that pushes its siblings apart | 1 |
| Layout | `SplitHandle` | A bar to drag that changes a size | 2 |
| Layout | `SplitPanes` | Any number of panes with a bar between each two, and limits | 3 |
| Layout | `Splitter` | Two panes with a draggable divider | 2 |
| Layout | `Sticky` | Stays below the top while its section scrolls past | 1 |
| Layout | `Ticker` | A strip of content that keeps moving sideways, still under reduced motion | 2 |
| Layout | `ZStack` | Views on top of each other, in a box as large as the largest | 1 |
| Layout | `QuadField` | A plane of squares that waves like a floor and stands up into a picture as the page scrolls | 4 |
| Layout | `TileLayout` | A dashboard of tiles to drag and resize | 2 |
| Layout | `DockArea` | Panels in a tree of splits and tabs that move by a drag, by keys or by a menu, resize, collapse, close, and save their arrangement as text | 3 |
| Display and feedback | `Badge` | Short label for a state or a count, in a tone that says what it means | 6 |
| Display and feedback | `Alert` | A message about the page, with a tone, an icon and an optional title | 5 |
| Display and feedback | `Avatar` | A person's initials or picture, in a circle or a rounded square | 4 |
| Display and feedback | `ProgressBar` | How far a task is, with an optional label, percent and detail | 6 |
| Display and feedback | `Kbd` | Keyboard key, for showing shortcuts | 1 |
| Display and feedback | `Skeleton` | Placeholder block shown while content loads | 1 |
| Display and feedback | `Spinner` | Indeterminate activity indicator | 2 |
| Display and feedback | `Text` | Text in the theme's type scale, with truncation | 4 |
| Display and feedback | `DaySeparator` | A line with a word in the middle that divides a list by day | 1 |
| Display and feedback | `Dot` | Small round mark of state, such as not saved | 5 |
| Display and feedback | `EmptyState` | What an area shows when it has nothing: icon, title, sentence and actions | 2 |
| Display and feedback | `HeroBanner` | Wide band in a strong color, readable from across the room | 6 |
| Display and feedback | `IconTile` | An icon in a square tinted with its color | 7 |
| Display and feedback | `KeyBar` | The function keys of a counter, each with what it does, the main one stressed | 3 |
| Display and feedback | `MultiProgress` | Several progress bars stacked, for work with more than one part | 1 |
| Display and feedback | `OverlayBadge` | A small view on a corner of another | 5 |
| Display and feedback | `Rating` | Stars for a score, to read or to give with the pointer and the keyboard | 6 |
| Display and feedback | `ShimmerText` | Text with a light passing over it, still when motion is reduced | 2 |
| Display and feedback | `ShortcutHintBar` | Line of key hints under what they act on | 1 |
| Display and feedback | `ShortcutSheet` | Dialog listing the keys of a screen, in groups | 2 |
| Media | `Canvas` | Box that draws whatever its closure appends to the scene | 1 |
| Media | `Carousel` | Slides with buttons, position dots and optional autoplay | 2 |
| Media | `Icon` | Vector icon from the built-in Lucide set | 2 |
| Media | `Image` | Shows an image, with fit modes | 5 |
| Media | `QuarterTurn` | QuarterTurn | 1 |
| Media | `ZoomControl` | Fit, 1:1, the percentage and buttons that zoom in and out | 4 |
| Media | `Filmstrip` | A strip of thumbnails of any length, whose cells come from the app | 3 |
| Media | `PagedDocumentView` | A bar with page and zoom over pages built only when they show | 1 |
| Media | `ColorWheel` | Hue and saturation on a disc, by a drag or the arrow keys | 2 |
| Media | `CompareView` | Two pictures side by side or across a divider you drag | 3 |
| Media | `CropOverlay` | A crop with a veil, proportion and angle, returning the rectangle and the angle | 2 |
| Media | `CurveEditor` | Points you drag over a smooth curve, with an optional histogram behind | 3 |
| Media | `Eyedropper` | An armed button that takes the color of the point clicked, supplied by the caller | 5 |
| Media | `Histogram` | Overlapping channels from ready-made bins, linear or logarithmic, with clip marks | 3 |
| Media | `ImageLightbox` | A picture over the whole app, with zoom, arrows, Esc, caption and counter | 1 |
| Media | `Lottie` | Plays a Lottie animation, as LottieFiles keeps them, drawn by RavenRS | 3 |
| Media | `Marquee` | Marching ants around a rectangle, a path or loose segments | 2 |
| Media | `Minimap` | Small copy of the content with a draggable rectangle around what is in view | 1 |
| Media | `Protractor` | A band of ticks along an arc that sets an angle | 2 |
| Media | `RubberBand` | A rectangle drawn with a drag, in the units of the content, selected on release | 2 |
| Media | `RuleOfThirdsGrid` | Composition lines over a picture: thirds, divisions, squares or the golden ratio | 2 |
| Media | `Ruler` | Horizontal or vertical ruler in your units, with a round step for any zoom | 1 |
| Media | `TransformHandles` | Eight handles, a turn and a pivot over a box, returning its geometry | 2 |
| Media | `ZoomPanView` | Zoom and pan with pinch, wheel, drag, keys and inertia, within limits | 1 |
| Application | `AppShell` | The window of an application made from its model: navigation by module, lists, forms and the theme menu | 3 |
| Application | `EntityForm` | A form made from an entity of the model, in a window: each field with the editor its kind asks for, the rules checked on save | 5 |
| Application | `MoviePlayer` | Plays a film of the app, drawn as vectors, with chapters, captions and the sound it computes | 2 |
| Application | `MovieAssistant` | Finds the part of a film that answers a question, by its chapters, and plays it | 1 |
| Application | `BugReporter` | The dialog that turns the last minutes on screen into a bug report: its kind, what happened, a box over the problem, the film to watch and cover, and all that goes along | 8 |

Charts take their series from `chart-1` to `chart-5`; alignment guides take `guide`.

## Patterns

Small screens made only of the components above, measured like them, under `patterns/`: start a screen from the one closest to it, and keep its spaces and what stretches.

| Pattern | What it is |
|---|---|
| `FormPattern` | A form: labeled fields in two columns, a hint, an error and the actions at the end, the one that matters accented |
| `SidebarPage` | A page beside its navigation: the sidebar on the left, the page header and the content on the same margin |
| `FilteredGrid` | A list of records: the page header, a bar with search and filters, and the grid under it, filling the width |
| `Confirmation` | Asking before an action that cannot be undone: the question, what follows and the two answers |
| `EmptyPage` | A page with nothing yet: what will be here, how to start and a way to learn more |

## Themes of the applications

Applications made with RavenRS choose their own colors. A screen designed for one of them takes its theme: put `data-theme="<id>"` on the screen's root, and every `var(--token)` follows it. Only the colors and shadows change; sizes, corners and type are the system's.

| Theme | `data-theme` | Accent |
|---|---|---|
| Raven Finanças Claro | `financas-claro` | `#7ac70c` |
| Raven Finanças Escuro | `financas-escuro` | `#b2ea3c` |
| Raven Finanças Vidro | `financas-vidro` | `#b2ea3c` |

## Screens

These rules came out of reviewing `tools/raven-feature-center` screen by screen, and they hold for every component and application.

- **One action stands out.** A window accents the one action that matters most: Save on a form, New on a list. Everything else in the bar stays a ghost button.
- **One page margin.** Title, commands, description and content all start on the same line, `space-4` from the page's edge.
- **No space by accident.** Stacks default to `space-4`. Put spacing in on purpose where pieces of a window meet, or it shows up as an empty band under a toolbar or a gap beside the navigation.
- **Title or trail.** One level is a page title in `headline`. A breadcrumb appears only when there is somewhere to go back to.
- **Fields fill their column.** References, colors and date with time take the same width as the text fields around them. A fixed width is a choice made at the call site.
- **Say it once.** A collection in a tab has no heading or label of its own, and a nested list leaves out the column of its owner. An `Alert` has no title when one would only repeat where it is: use `Alert::message`.
- **Long lists are striped.** `DataGrid::striped` tints every other row with `bg-300` at 40%, under the pointer at 85%, and the accent stays for the selection at 13%, so the three never get confused. Lists generated from a model do it by themselves.
- **Grids show data first.** The filter row is flat, with each filter's text starting where its column's does, at the end for numbers. The group panel shows only when something is grouped or when asked for with `group_panel(true)`; the Group menu is always in the bar.
- **Nothing moves under the pointer.** Selecting a row does not change the height of what is above it. Prefer replacing a status for adding to it.
- **One piece, no frame.** A dashboard with a single item has no tile around it. What only helps while handling something, such as a tile's resize corner, shows only under the pointer.
- **One language.** Every label follows the `Labels` of the application, including those in report viewers.

## Writing

Name things the way people see them: "Get started", "Save changes", "Revenue". Buttons start with a verb. Errors say what happened and how to fix it.
