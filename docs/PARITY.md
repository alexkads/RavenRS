# Component parity: shadcn/ui, DevExpress, Telerik

RavenRS aims to be more complete than the three reference libraries. This table tracks every component family they offer and where RavenRS stands. It is updated with each batch.

Legend: ✅ available · 🟡 partial (what is missing is noted) · ⬜ planned. Columns S/D/T mark whether shadcn/ui, DevExpress or Telerik ships it.

## Summary

| | Families tracked | ✅ | 🟡 | ⬜ |
|---|---|---|---|---|
| All | 122 | 82 | 7 | 33 |

## Layout and structure

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Stack (VStack/HStack), Spacer | | | | ✅ |
| Overlap (ZStack) | | | | ✅ `ZStack`: as large as the largest layer |
| Page header, property row | | | | ✅ `PageHeader`, `PropertyRow` |
| Grid layout | | ✓ | ✓ | ✅ `Grid` |
| Card | ✓ | ✓ | ✓ | ✅ `Card` with a selected state, `GlassCard` (blur planned) |
| Separator | ✓ | | | ✅ `Divider` |
| Aspect ratio | ✓ | | | ✅ `AspectRatio` |
| Scroll area | ✓ | ✓ | ✓ | ✅ `ScrollView` (vertical; horizontal inside DataGrid) |
| Resizable panels / Splitter | ✓ | ✓ | ✓ | ✅ `Splitter` for two panes, `SplitPanes` for any number with limits, `SplitHandle` for a bar of your own; all move with the keyboard |
| Tile layout / Dashboard layout | | ✓ | ✓ | ✅ `TileLayout`: drag and resize, also from the keyboard; fewer columns when narrow; a layout to keep |
| Docking / Dock manager | | ✓ | ✓ | ⬜ |
| Window (floating) | | ✓ | ✓ | 🟡 `FloatingPanel::movable`: moves and reduces, does not resize |
| App bar / Toolbar | | ✓ | ✓ | ✅ `Toolbar`, `ToolbarAction`: what does not fit folds into a menu |
| Toolbar floating over content | | | ✓ | ✅ `FloatingToolbar`: translucent, placed by side, moved by a handle or the arrows |
| Arrows on the ends of a scrolling strip | | | ✓ | ✅ `ScrollEdgeArrow`: only where there is more to show, with the keyboard kept at the end |
| Custom title bar | | | ✓ | 🟡 `TitleBar`: title, actions and the window's buttons, drawing and calling back only. Dragging the window and hiding the system's frame belong to the platform layer, which offers neither yet |
| Footer | | | | ✅ `Footer` |
| Status bar | | ✓ | ✓ | ✅ `StatusBar` |
| Tool rail | | | | ✅ `ToolRail`: variants in a menu at the side, lines between groups, icons only, several columns |
| Timeline with tracks | | | ✓ | ✅ `Timeline` |

## Navigation

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Tabs / TabStrip | ✓ | ✓ | ✓ | ✅ `Tabs`: underlined or segmented, sizes, the bar alone, disabled while waiting |
| Sidebar / Drawer navigation | ✓ | ✓ | ✓ | ✅ `Sidebar`: closes to icons or off the screen, a rail along its edge, a brand, keys on a row, header and footer, groups, submenus, actions, badges, skeleton, sheet in narrow windows, floating and inset variants. Drag to resize: `Splitter` |
| Breadcrumb | ✓ | ✓ | ✓ | ✅ `Breadcrumb`: locations can be links, an icon at the root |
| Pagination / Pager | ✓ | ✓ | ✓ | ✅ `Pagination` |
| Accordion / PanelBar | ✓ | ✓ | ✓ | ✅ `Accordion`, `Disclosure`: any view as title, a mark, an on/off switch, open state kept by the app |
| Menu / Menubar | ✓ | ✓ | ✓ | ✅ `Menubar` |
| Ribbon (Office-style command bar) | | ✓ | ✓ | ✅ `Ribbon` (app menu, quick access, contextual tabs, collapse; key tips and gallery items planned) |
| Navigation menu (mega menu) | ✓ | | ✓ | ⬜ |
| Context menu | ✓ | ✓ | ✓ | ✅ `ContextMenu`: submenus, check marks and any view in a row, like `DropdownMenu` |
| Stepper / Wizard | | ✓ | ✓ | ✅ `Wizard`: a step can refuse to be left or be marked with a problem; steps done can be reopened. `StepIndicator`, `StepBadge` for an app's own layout |
| Bottom navigation | | | ✓ | ⬜ |
| TreeView | | ✓ | ✓ | ✅ `TreeView`: keyboard, selection, three-state checks, filter, items on demand, virtualized |
| Command palette | ✓ | | | ✅ `CommandPalette` |

## Buttons and actions

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Button (variants, sizes, icons) | ✓ | ✓ | ✓ | ✅ `Button`, `IconButton`: loading that holds the width, selected, key cap in the label's color |
| Toggle button | ✓ | | ✓ | ✅ `ToggleButton`: count and pill shape |
| Favorite mark | | | | ✅ `FavoriteToggle`: heart or star |
| Swatch grid, color pair, nine-point anchor | | | | ✅ `SwatchGrid` (the handler gets the modifiers), `ColorPair`, `NinePointAnchor` |
| Toggle group / Button group | ✓ | ✓ | ✓ | ✅ `ToggleGroup`, and `ButtonGroup` for buttons glued side by side |
| Dropdown button | ✓ | ✓ | ✓ | ✅ `DropdownMenu`: submenus opened by hover, click or the Right arrow, check marks and any view in a row |
| Split button | | ✓ | ✓ | ✅ `SplitButton` |
| Floating action button | | ✓ | ✓ | ⬜ |
| Speed dial | | ✓ | ✓ | ⬜ |
| Floating action button with a panel | | ✓ | ✓ | ✅ `FloatingPanel`: a round button that drags between corners, or placed anywhere by its header with the place kept by the app; reduces to its header |

## Inputs and editors

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Text box | ✓ | ✓ | ✓ | ✅ `TextField` (selection, clipboard, views before and after the text, clear button, sizes) |
| Text area (multi-line) | ✓ | ✓ | ✓ | ✅ `TextArea`: wraps, scrolls, builds only the lines in view, grows with the text between two limits |
| Label | ✓ | ✓ | ✓ | ✅ `Label` |
| Form, field validation | ✓ | ✓ | ✓ | ✅ `FormField` + validators |
| Form layout (groups, columns, tabs, error list) | | ✓ | ✓ | ✅ `FormLayout`, `FormGroup` |
| Checkbox | ✓ | ✓ | ✓ | ✅ |
| Radio group | ✓ | ✓ | ✓ | ✅ |
| Switch | ✓ | ✓ | ✓ | ✅ `Toggle`, with a small size |
| Slider | ✓ | ✓ | ✓ | ✅ `Slider`: marks, colored track, fill from a neutral value that a double click returns to, value at the right |
| Value in small that opens a slider | | | | ✅ `ValueSlider`: a button, or a field with `field`, and the keyboard goes into the slider |
| Rename in place | | | ✓ | ✅ `InlineRename`: double click, F2 or Enter; Enter and leaving confirm, Escape cancels |
| Range slider | | ✓ | ✓ | ⬜ |
| Select / Drop-down list | ✓ | ✓ | ✓ | ✅ `Select`: by index or by value (`Select::of`), search, sizes |
| Combobox / Autocomplete | ✓ | ✓ | ✓ | ✅ `Combobox` |
| Filter builder | | ✓ | ✓ | ✅ `FilterBuilder`: lines and groups over the expression language; what it cannot show stays as text; also in the `DataGrid` toolbar |
| Lookup (records loaded on demand) | | ✓ | ✓ | ✅ `LookupField`: from memory or a callback, ignores stale answers |
| Multi-select / Tag box | | ✓ | ✓ | ✅ `TagBox`: search, new values, a limit |
| Numeric input / Spin editor | | ✓ | ✓ | ✅ `NumberField` (range, step, separators, money in whole cents), `Stepper` |
| Masked input | | ✓ | ✓ | ✅ `MaskedField` |
| One-time code (OTP) | ✓ | | ✓ | ✅ `InputOtp` |
| Calendar | ✓ | ✓ | ✓ | ✅ `Calendar` |
| Date picker | ✓ | ✓ | ✓ | ✅ `DatePicker` |
| Date range picker | ✓ | ✓ | ✓ | ✅ `DateRangePicker`: two months, ready-made periods |
| Time picker / DateTime picker | | ✓ | ✓ | ✅ `TimePicker`, `DateTimePicker` |
| Theme picker and token editor | | | ✓ | ✅ `ThemePicker`: menu of themes. `ThemeEditor`: colors, corners and text sizes by token name, applied at once, with live contrast and JSON to copy and paste |
| Color picker / palette / gradient | | ✓ | ✓ | 🟡 `ColorPicker`: palette, shade square, hue and opacity bars, hexadecimal; gradients planned |
| Rating | | ✓ | ✓ | ✅ `Rating`: read-only at the cost of one element, or given with the pointer, arrows, Home, End and digits |
| File upload | | ✓ | ✓ | ✅ `FileUpload`, `ImageField`: the system's chooser and drops, on desktop and web; sending is up to the app |
| Drop zone | | ✓ | ✓ | ✅ `DropZone`: around any content, with the rules of `FileUpload`; every zone lights up during the drag, since platforms say only that files are over the window |
| Signature | | | ✓ | ⬜ |
| Rich text editor | | ✓ | ✓ | ⬜ |
| Code editor | | ✓ | | ⬜ |
| Spreadsheet | | ✓ | ✓ | ⬜ |

## Data

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Table | ✓ | ✓ | ✓ | ✅ `Table` |
| Data grid | ✓ | ✓ | ✓ | 🟡 `DataGrid`; see its parity table in ARCHITECTURE.md |
| Tree list / Tree grid | | ✓ | ✓ | ⬜ |
| Pivot grid | | ✓ | ✓ | ⬜ |
| List / List view | | ✓ | ✓ | ✅ `List`, `ListItem` |
| List box (with reordering) | | ✓ | ✓ | ⬜ |
| Card view / Tile view | | ✓ | ✓ | ✅ `CardView`: virtualized, as many columns as fit, keyboard and selection |
| Scheduler (resource timeline) | | ✓ | ✓ | 🟡 `Scheduler`; gaps listed under "Scheduler" below |
| Calendar with events | | ✓ | ✓ | ✅ `EventCalendar` |
| Recurrence editor | | ✓ | ✓ | ✅ `RecurrenceEditor`, `RecurrenceRule` (reads and writes iCalendar's `RRULE`) |
| Gantt | | ✓ | ✓ | ⬜ |
| Kanban / Task board | | | ✓ | ⬜ |
| Timeline | | | ✓ | ⬜ |
| File manager | | ✓ | ✓ | ⬜ |

## Charts and visualization

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Bar / column chart | ✓ | ✓ | ✓ | ✅ `Chart`, `BarChart`: horizontal, grouped, stacked, negative, labels, active, focus that fades the rest, gradient, least length, growing from any side, categories on the line of zero |
| Area chart, sparkline | ✓ | ✓ | ✓ | ✅ `Chart`, `AreaChart`: stacked, expanded, gradient, curves; a sparkline with a tooltip and cursor when asked |
| Line chart | ✓ | ✓ | ✓ | ✅ `Chart`: natural, monotone, linear and step curves, dots, icons, labels |
| Pie / donut | ✓ | ✓ | ✓ | ✅ `Chart`: rings, active slice, a slice that comes forward under the pointer, center text, labels, guides with an elbow, a real gap between slices, a radius per slice |
| Radar, radial | | ✓ | ✓ | ✅ `Chart`: polygon and circle grids, filled, tracks, stacked |
| Chart tooltip, legend, keyboard | ✓ | ✓ | ✓ | ✅ `ChartTooltip` (share, color by value, free content, either side on both axes), `ChartLegend`, arrow keys; cursor and dots that glide and fade |
| Chart axes | ✓ | ✓ | ✓ | ✅ `Chart`: dashed grid, columns, steps of the axis, labels measured with the real font, aligned ends, lines of reference, fixed domain |
| Scatter, bubble, funnel, financial | | ✓ | ✓ | 🟡 scatter in `Chart`, with a tooltip and cursor when asked; candlestick in `Chart`, with an axis of round numbers, outlines for what rose and the keyboard; bubble and funnel ⬜ |
| Sankey | | ✓ | ✓ | ✅ `SankeyChart`: alignment, ribbons in a gradient, the rest fading around the focus, arrow keys, every flow read aloud, a clear message when the flows go in a circle |
| Gauges (linear, circular, arc) | | ✓ | ✓ | ✅ `ProgressBar`, `Chart` with `RadialBar` |

The whole [shadcn/ui chart catalog](https://ui.shadcn.com/charts) can be built with `Chart` and `ChartCard`: all 70 patterns are in the gallery's Charts page, under their names (`examples/component-gallery/src/charts.rs`).
| Maps | | ✓ | ✓ | ⬜ |
| Diagram / Org chart | | ✓ | ✓ | ⬜ |
| QR code / Barcode | | ✓ | ✓ | ✅ `Barcode`: QR Code, Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF |

## Overlays and feedback

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Dialog / Alert dialog | ✓ | ✓ | ✓ | ✅ `Dialog`, `ConfirmDialog`: Escape, the backdrop and the X can each be turned off |
| Sheet / Drawer (side panel) | ✓ | ✓ | ✓ | ✅ `Sheet` |
| Splash screen | | ✓ | ✓ | ✅ `SplashScreen` |
| Tray icon, closing to it | | ✓ | ✓ | ✅ `AppIdentity::close_to_tray` |
| System notifications | | ✓ | ✓ | ✅ `SystemNotifier` |
| Popover | ✓ | ✓ | ✓ | ✅ `Popover`: any placement, with or without the panel |
| Tooltip | ✓ | ✓ | ✓ | ✅ `Tooltip` with a key cap, and `.tooltip()` on any view |
| Hover card | ✓ | | | ✅ `HoverCard` |
| Toast / Notification | ✓ | ✓ | ✓ | ✅ `Toaster`: six places, per stack or per toast, and toasts that stay until closed |
| Alert / Callout | ✓ | ✓ | ✓ | ✅ `Alert`: with a compact strip |
| Code snippet with copy | | ✓ | | ✅ `CodeBlock` |
| Release notes | | | | ✅ `ReleaseNotes` |
| Training film and player | | | | ✅ `ravenrs::movie`: `Recorder`, `MoviePlayer`, with click and key sounds, a voice and boxes that explain each step |
| Question that opens a film at the right moment | | | | ✅ `MovieAssistant`, `MovieLibrary`: words the films hold, no model |
| Film editor with timeline | | | | ✅ `MovieEditor`; its preview has no sound |
| Bug report with a film | | | | ✅ `FlightRecorder`, `BugReporter` |
| Progress bar | ✓ | ✓ | ✓ | ✅ `ProgressBar`: label, percentage, detail, height, color |
| Loader / Spinner | | ✓ | ✓ | ✅ `Spinner` |
| Lottie animation | | ✓ | ✓ | ✅ `Lottie`; no expressions, text layers, 3D, blend modes or effects other than Fill yet, and `.lottie` archives need unpacking first |
| Skeleton | ✓ | ✓ | ✓ | ✅ |
| Badge | ✓ | ✓ | ✓ | ✅ |
| Chip / Chip list | | ✓ | ✓ | 🟡 `Badge` (tone or own color, outline, height); removable chips via `Button` with an icon |

## Media and content

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Avatar | ✓ | ✓ | ✓ | ✅ `Avatar`: initials or picture, with a ring |
| Icon set | ✓ | ✓ | ✓ | ✅ 200 Lucide icons |
| Image | | ✓ | ✓ | ✅ `Image`: dissolves into the next one if asked, and asks for frames only meanwhile |
| Carousel / Scroll view | ✓ | ✓ | ✓ | ✅ `Carousel` |
| Kbd | ✓ | | | ✅ takes the color of what it is in, on request |
| List of shortcuts | | | | ✅ `ShortcutSheet`, `ShortcutHintBar`, over `Kbd` and `Dialog` |
| Empty state | ✓ | | | ✅ `EmptyState`, over `IconTile` |
| Status dot, badge on a corner | | | ✓ | ✅ `Dot`, `OverlayBadge` |
| Banner, stacked meters, text with a shine | | | | ✅ `HeroBanner`, `MultiProgress`, `ShimmerText` (still with reduced motion) |
| Day divider in a list | | | | ✅ `DaySeparator` |
| Typography scale | ✓ | ✓ | ✓ | ✅ `Text` styles |
| PDF viewer | | ✓ | ✓ | ⬜ |
| Histogram | | | | ✅ `Histogram`: channels from ready-made bins, added up as light, linear or logarithmic, with marks at clipped ends |
| Ruler | | | | ✅ `Ruler`: horizontal or vertical, in the units of the caller, with a round step for the zoom, origin and marker; vertical numbers stack, since text does not rotate |
| Color wheel | | | | ✅ `ColorWheel`: hue and saturation on a disc that is computed once and shared, with arrow keys and Shift to keep the hue |
| Curve editor | | | | ✅ `CurveEditor`: points over a monotone curve, added with a press, removed with a double click or Delete, moved with the arrows; baseline and histogram behind |
| Zoom control | | | | ✅ `ZoomControl`: fit, 1:1, percentage, + and −, through a `ZoomView` shared with `ZoomPanView` and `Minimap` |
| Zoom and pan | | | | ✅ `ZoomPanView`: pinch, Cmd/Ctrl with the wheel, wheel and drag to move, double click, keys, inertia that respects reduced motion, limits. Content is an image or a closure: the runtime has no transform for a subtree |
| Minimap | | | | ✅ `Minimap`: a small copy with the rectangle of what is in view, by the same `ZoomView` as `ZoomPanView`, with drag and arrow keys |
| Image editor | | | ✓ | ⬜ |
| Chat | | ✓ | ✓ | ✅ `Chat`; `MovieChat` over training films |
| Choosing local AI models | | | | ✅ `ModelPicker`; `ravenrs_ai::ModelManager` offers only what fits |

## Reporting

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Report engine (bands, groups, summaries, expressions) | | ✓ | ✓ | 🟡 `ravenrs-report`; see [REPORTING-PARITY.md](REPORTING-PARITY.md) |
| Document viewer (pages, zoom, search, thumbnails) | | ✓ | ✓ | ✅ `ravenrs-report-viewer` |
| End-user report designer | | ✓ | ✓ | 🟡 `examples/report-designer`, [live](https://alexkads.github.io/RavenRS/designer/) |
| Export: PDF, XLSX, DOCX, CSV, HTML, images | | ✓ | ✓ | 🟡 `ravenrs-report-pdf`, `ravenrs-report-xlsx` (XLSX and CSV), SVG per page, Excel, PDF and CSV from `DataGrid`; DOCX, HTML and PNG planned |

## Platform capabilities

| Capability | S | D | T | RavenRS |
|---|---|---|---|---|
| Themes (light, dark, custom tokens) | ✓ | ✓ | ✓ | ✅ Raven Dark, Light, Glass |
| Keyboard navigation and focus | ✓ | ✓ | ✓ | ✅ |
| Screen reader support | ✓ | ✓ | ✓ | ✓ AccessKit on desktop, ARIA mirror on the web |
| Right-to-left | ✓ | ✓ | ✓ | ⬜ (Parley supports bidi; layout mirroring planned) |
| Localization (formats, strings) | | ✓ | ✓ | 🟡 number formats; `Labels` and `Localized` for component text (Ribbon, Carousel and InputOtp still fixed) |
| Animations | ✓ | ✓ | ✓ | ✅ springs, transitions, shared layout, enter and exit; 🟡 no rotation or group opacity yet |
| Native desktop and browser from one codebase | | | | ✅ (none of the three does both) |

## Scheduler

The reference is [Bryntum Scheduler](https://bryntum.com/products/scheduler/). The behavior is the model and the implementation is our own.

| Capability | Status |
|---|---|
| Rows of resources, time axis with two header rows, bars by time | ✅ |
| Hours, days, weeks and months, with zoom keeping the same time | ✅ |
| Drag along time and between rows, resize by the ends, draw on an empty stretch | ✅ |
| Snap to the scale, refusal through `can_drop`, locked and fixed-length events | ✅ |
| Overlapping events on lanes, with the row growing | ✅ |
| Rows sharing the height left over (`fill_rows`) | ✅ |
| A full look: tinted bars with a stripe, a second line on bars, avatars, today's column (`card`, `event_detail`, `avatars`) | ✅ |
| Weekend shading, marked time ranges, a line at now, opening at now | ✅ |
| Collapsible groups of resources | ✅ |
| Keyboard for events; names and dates for screen readers | ✅ |
| Only what is in view is built | ✅ tested with 5,000 rows |
| Recurring events | ✅ `RecurrenceRule`; the app expands the days |
| Scrolling the wheel over the resources column | ✅ |
| Scrolling by itself while dragging near an edge | ✅ |
| Tooltip while hovering over or dragging an event | ✅ with `card` |
| Scrolling a bar into view when keyboard focus moves to it | ✅ |
| Dependencies between events | ⬜ |
| Resource tree of any depth, multiple assignment, extra columns | ⬜ |
| Working hours, non-continuous axis, vertical mode, nested events | ⬜ |
| Summaries and histograms per row or period | ⬜ |
| Dragging from a list or grid onto the timeline, or between schedulers | ⬜ |
| Menus on events and on empty stretches (`event_menu`, `lane_menu`), zoom with a pinch or Ctrl and the wheel | ✅ |
| Undo and redo | 🟡 the app keeps the moves, as in Raven Finanças' agenda; none built in |
| Built-in event editor | ⬜ |
| Export to PDF, PNG, Excel and ICS, printing | ⬜ |
