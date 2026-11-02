# Component parity: shadcn/ui, DevExpress, Telerik

RavenRS aims to be more complete than the three reference libraries. This table tracks every component family they offer and where RavenRS stands. It is updated with each batch.

Legend: ✅ available · 🟡 partial (what is missing is noted) · ⬜ planned. Columns S/D/T mark whether shadcn/ui, DevExpress or Telerik ships it.

## Summary

| | Families tracked | ✅ | 🟡 | ⬜ |
|---|---|---|---|---|
| All | 110 | 56 | 9 | 45 |

## Layout and structure

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Stack (VStack/HStack), Spacer | | | | ✅ |
| Grid layout | | ✓ | ✓ | ✅ `Grid` |
| Card | ✓ | ✓ | ✓ | ✅ `Card`, `GlassCard` (blur planned) |
| Separator | ✓ | | | ✅ `Divider` |
| Aspect ratio | ✓ | | | ✅ `AspectRatio` |
| Scroll area | ✓ | ✓ | ✓ | ✅ `ScrollView` (vertical; horizontal inside DataGrid) |
| Resizable panels / Splitter | ✓ | ✓ | ✓ | ✅ `Splitter` |
| Tile layout / Dashboard layout | | ✓ | ✓ | ⬜ |
| Docking / Dock manager | | ✓ | ✓ | ⬜ |
| Window (floating) | | ✓ | ✓ | ⬜ |
| App bar / Toolbar | | ✓ | ✓ | 🟡 composable from `HStack` + buttons; no overflow menu |
| Footer | | | | ✅ `Footer` |
| Status bar | | ✓ | ✓ | ✅ `StatusBar` |
| Tool rail | | | | ✅ `ToolRail` |
| Timeline with tracks | | | ✓ | ✅ `Timeline` |

## Navigation

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Tabs / TabStrip | ✓ | ✓ | ✓ | ✅ `Tabs` |
| Sidebar / Drawer navigation | ✓ | ✓ | ✓ | ✅ `Sidebar`: closes to icons or off the screen, header and footer, groups, submenus, actions, badges, skeleton, sheet in narrow windows, floating and inset variants. Drag to resize: `Splitter` |
| Breadcrumb | ✓ | ✓ | ✓ | ✅ `Breadcrumb` |
| Pagination / Pager | ✓ | ✓ | ✓ | ✅ `Pagination` |
| Accordion / PanelBar | ✓ | ✓ | ✓ | ✅ `Accordion`, `Disclosure` |
| Menu / Menubar | ✓ | ✓ | ✓ | ✅ `Menubar` |
| Ribbon (Office-style command bar) | | ✓ | ✓ | ✅ `Ribbon` (app menu, quick access, contextual tabs, collapse; key tips and gallery items planned) |
| Navigation menu (mega menu) | ✓ | | ✓ | ⬜ |
| Context menu | ✓ | ✓ | ✓ | ✅ `ContextMenu` |
| Stepper / Wizard | | ✓ | ✓ | ⬜ |
| Bottom navigation | | | ✓ | ⬜ |
| TreeView | | ✓ | ✓ | ⬜ |
| Command palette | ✓ | | | ✅ `CommandPalette` |

## Buttons and actions

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Button (variants, sizes, icons) | ✓ | ✓ | ✓ | ✅ `Button`, `IconButton` |
| Toggle button | ✓ | | ✓ | ✅ `ToggleButton` |
| Toggle group / Button group | ✓ | ✓ | ✓ | ✅ `ToggleGroup` |
| Dropdown button | ✓ | ✓ | ✓ | ✅ `DropdownMenu` |
| Split button | | ✓ | ✓ | ⬜ |
| Floating action button | | ✓ | ✓ | ⬜ |
| Speed dial | | ✓ | ✓ | ⬜ |

## Inputs and editors

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Text box | ✓ | ✓ | ✓ | ✅ `TextField` (selection, clipboard) |
| Text area (multi-line) | ✓ | ✓ | ✓ | ⬜ |
| Label | ✓ | ✓ | ✓ | ✅ `Label` |
| Form, field validation | ✓ | ✓ | ✓ | ✅ `FormField` + validators |
| Checkbox | ✓ | ✓ | ✓ | ✅ |
| Radio group | ✓ | ✓ | ✓ | ✅ |
| Switch | ✓ | ✓ | ✓ | ✅ `Toggle` |
| Slider | ✓ | ✓ | ✓ | ✅ |
| Range slider | | ✓ | ✓ | ⬜ |
| Select / Drop-down list | ✓ | ✓ | ✓ | ✅ `Select` |
| Combobox / Autocomplete | ✓ | ✓ | ✓ | ✅ `Combobox` |
| Multi-select / Tag box | | ✓ | ✓ | ⬜ |
| Numeric input / Spin editor | | ✓ | ✓ | 🟡 `Stepper`; a typed numeric field is planned |
| Masked input | | ✓ | ✓ | ⬜ |
| One-time code (OTP) | ✓ | | ✓ | ✅ `InputOtp` |
| Calendar | ✓ | ✓ | ✓ | ✅ `Calendar` |
| Date picker | ✓ | ✓ | ✓ | ✅ `DatePicker` |
| Date range picker | ✓ | ✓ | ✓ | ⬜ |
| Time picker / DateTime picker | | ✓ | ✓ | ⬜ |
| Color picker / palette / gradient | | ✓ | ✓ | ⬜ |
| Rating | | ✓ | ✓ | ⬜ |
| File upload | | ✓ | ✓ | ⬜ |
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
| Card view / Tile view | | ✓ | ✓ | 🟡 `Grid` of cards; no virtualization |
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
| Bar / column chart | ✓ | ✓ | ✓ | ✅ `BarChart`, `Chart` |
| Area chart, sparkline | ✓ | ✓ | ✓ | ✅ `AreaChart`, `Chart` |
| Line chart | ✓ | ✓ | ✓ | ✅ `Chart`, with the layout reports use |
| Pie / donut | ✓ | ✓ | ✓ | ✅ `Chart` |
| Scatter, bubble, radar, funnel, financial | | ✓ | ✓ | 🟡 scatter in `Chart`; the others ⬜ |
| Gauges (linear, circular, arc) | | ✓ | ✓ | 🟡 `ProgressBar`; circular gauges need paths |
| Maps | | ✓ | ✓ | ⬜ |
| Diagram / Org chart | | ✓ | ✓ | ⬜ |
| QR code / Barcode | | ✓ | ✓ | ✅ `Barcode`: QR Code, Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF |

## Overlays and feedback

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Dialog / Alert dialog | ✓ | ✓ | ✓ | ✅ `Dialog` |
| Sheet / Drawer (side panel) | ✓ | ✓ | ✓ | ✅ `Sheet` |
| Splash screen | | ✓ | ✓ | ✅ `SplashScreen` |
| Popover | ✓ | ✓ | ✓ | ✅ |
| Tooltip | ✓ | ✓ | ✓ | ✅ |
| Hover card | ✓ | | | ✅ `HoverCard` |
| Toast / Notification | ✓ | ✓ | ✓ | ✅ `Toaster` |
| Alert / Callout | ✓ | ✓ | ✓ | ✅ `Alert` |
| Code snippet with copy | | ✓ | | ✅ `CodeBlock` |
| Release notes | | | | ✅ `ReleaseNotes` |
| Training film and player | | | | ✅ `ravenrs::movie`: `Recorder`, `MoviePlayer`, with click and key sounds, a voice and boxes that explain each step |
| Question that opens a film at the right moment | | | | ✅ `MovieAssistant`, `MovieLibrary`: words the films hold, no model |
| Film editor with timeline | | | | ✅ `MovieEditor`; its preview has no sound |
| Bug report with a film | | | | ✅ `FlightRecorder`, `BugReporter` |
| Progress bar | ✓ | ✓ | ✓ | ✅ |
| Loader / Spinner | | ✓ | ✓ | ✅ `Spinner` |
| Skeleton | ✓ | ✓ | ✓ | ✅ |
| Badge | ✓ | ✓ | ✓ | ✅ |
| Chip / Chip list | | ✓ | ✓ | 🟡 `Badge`; removable chips via `Button` with an icon |

## Media and content

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Avatar | ✓ | ✓ | ✓ | ✅ |
| Icon set | ✓ | ✓ | ✓ | ✅ 200 Lucide icons |
| Image | | ✓ | ✓ | ✅ `Image` |
| Carousel / Scroll view | ✓ | ✓ | ✓ | ✅ `Carousel` |
| Kbd | ✓ | | | ✅ |
| Typography scale | ✓ | ✓ | ✓ | ✅ `Text` styles |
| PDF viewer | | ✓ | ✓ | ⬜ |
| Image editor | | | ✓ | ⬜ |
| Chat | | ✓ | ✓ | ⬜ |

## Reporting

| Component | S | D | T | RavenRS |
|---|---|---|---|---|
| Report engine (bands, groups, summaries, expressions) | | ✓ | ✓ | 🟡 `ravenrs-report`; see [REPORTING-PARITY.md](REPORTING-PARITY.md) |
| Document viewer (pages, zoom, search, thumbnails) | | ✓ | ✓ | ✅ `ravenrs-report-viewer` |
| End-user report designer | | ✓ | ✓ | 🟡 `examples/report-designer`, [live](https://alexkads.github.io/RavenRS/designer/) |
| Export: PDF, XLSX, DOCX, CSV, HTML, images | | ✓ | ✓ | 🟡 `ravenrs-report-pdf`, `ravenrs-report-xlsx` (XLSX and CSV), SVG per page, CSV from `DataGrid`; DOCX, HTML and PNG planned |

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
| Weekend shading, marked time ranges, a line at now, opening at now | ✅ |
| Collapsible groups of resources | ✅ |
| Keyboard for events; names and dates for screen readers | ✅ |
| Only what is in view is built | ✅ tested with 5,000 rows |
| Recurring events | ✅ `RecurrenceRule`; the app expands the days |
| Scrolling the wheel over the resources column | ⬜ |
| Scrolling by itself while dragging near an edge | ⬜ |
| Tooltip while hovering over or dragging an event | ⬜ |
| Scrolling a bar into view when keyboard focus moves to it | ⬜ |
| Dependencies between events | ⬜ |
| Resource tree of any depth, multiple assignment, extra columns | ⬜ |
| Working hours, non-continuous axis, vertical mode, nested events | ⬜ |
| Summaries and histograms per row or period | ⬜ |
| Dragging from a list or grid onto the timeline, or between schedulers | ⬜ |
| Undo and redo, built-in event editor and menu | ⬜ |
| Export to PDF, PNG, Excel and ICS, printing | ⬜ |
