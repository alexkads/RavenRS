# Changelog

All notable changes to RavenRS are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). From 26.2.0 on, versions are `YY.R.P`: breaking changes only in each year's `.1` release, additions in `.2`, fixes in maintenance releases, always valid Cargo SemVer. See [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

## [27.1.15] - 2026-11-06

### Breaking

- `ThemeSwitcher` is `Clone` but no longer `Copy`, `Send` or `Sync`, since it can hold a callback. Like every other view, build it on the UI thread.

### Added

- `ThemeSwitcher::on_change(action)`: runs with the theme the user picks, so the app can remember it for the next launch.

## [27.1.14] - 2026-11-05

### Breaking

- `ThemeSwitcher` is no longer a unit struct. Write `ThemeSwitcher::new()` where you wrote `ThemeSwitcher`.

### Added

- `ThemeSwitcher::themes(dark, light, glass)`: an app offers its own three themes in the switcher instead of the standard ones.

## [27.1.13] - 2026-11-05

### Breaking

- `ProgressBar` is no longer `Copy`, because it now holds a label and a detail. Clone it where you copied it.
- `Breadcrumb` and `Section` are no longer `Send`, `Sync`, `UnwindSafe` or `RefUnwindSafe`, because they now hold closures and views. Views were already built and kept on one thread.

### Added

- `DockArea` and `DockLayout`: panels in a tree of splits and groups of tabs at the left, the right, the bottom and the center. A tab goes to another group, to one side of it or to a border with a drag and a mark on the target, or by Alt and Shift with an arrow and the "Move to…" of its menu; it reorders with a drag or Alt and an arrow. Bars resize within limits by pointer and keys, docks collapse from the strips on their edges, and panels close and come back where they were. The arrangement holds ids alone, and what a panel shows comes from a function of the app. `DockLayout::to_text` and `parse` save it as versioned JSON: an unknown id is left out, and a text that cannot be read is a clear error.
- `ChatMessage::media` and `ChatMedia`: a photo that holds its place until it arrives, a document as a card or a recording with `AudioPlayerButton`, in the bubble, above the text. `ChatMessage::formatted` reads the text as a `RichText`, and `Chat::on_link` gets its links. Messages that ask for neither stay as they were.
- `monotone_slopes` in `ravenrs-core`: the slopes of a curve that never swings past its points, which charts and `CurveEditor` now share.
- `SelectionBar`: how many things are chosen, a way out by a button or Escape, and actions over a `Toolbar`, which go to a menu when they do not fit and are disabled while nothing is chosen.
- `AudioPlayerButton`: plays a recording and stops it over `Sound::speak_clip`, with progress drawn without rebuilding, audio the app can deliver after the press, and a transcript to open or ask for. Another recording sends it back to rest.
- `AttachmentStrip`: the files about to go with a message, as rows with an icon or thumbnail, a name that gives way, the size, a menu such as "Send as" when there is a choice, and a button to take each one out.
- `RichText`: bold, italic, struck text, code, lists, quotes and links in plain text, with the marks chat apps use. It wraps between words, links take a click, Enter or Space and go to `on_link`, and text with nothing to format costs what a `Text` costs. Italics are a muted color and code a chip, because `ravenrs-text` has one style per text and embeds no italic or monospace font.
- `DocumentTabs`: tabs for open documents that close with a button, Delete or the menu, change places by a drag or Alt and the arrows, are renamed with a double click, F2 or the menu, and carry a color and a dot for what was not saved. Tabs that do not fit scroll with arrows, and the menu takes the app's own items.
- `SortableList`: rows put in order by a drag, with a line at the target, or with Alt and the arrows, Home and End. Each move is announced to screen readers, and Escape calls a drag off. The drag arithmetic lives in one module, which `DocumentTabs` shares.
- `YearView`: the twelve months of a year, with marks on the days, a click on any of them and one Tab stop that walks past the end of the year. As many months go on a line as fit.
- `FileBrowser`: an explorer of folders and files from the app's data, with a clickable breadcrumb, sorting with folders first, selection with modifiers, icons, sizes and dates. It never touches the file system, and folders of any size stay virtual.
- `PagedDocumentView`: a bar with page, total and `ZoomControl` over a column of pages of which only those that show are built, with the pictures handed over by the app page by page and a placeholder until each is ready.
- `Filmstrip`: a horizontal strip of thumbnails of any length, whose cells come from the app. Single or multiple selection with modifiers, the arrows, Home and End, arrows on the ends that have more to show, and the selected cell kept in view.
- `VirtualList`: a list of any length with rows of any height, measured as they appear and remembered by key. It jumps to a row, can stay at the end as a chat does, and holds what is on screen still when rows come in above. It can also run sideways.
- `BuildCx::keyed_child_frame`, for a view that places its children by the room each one took, and `BuildCx::wheel_scrolls_sideways`, for a strip that a plain wheel should move.
- `ImageLightbox`: a picture over the whole app, with zoom by `ZoomPanView`, the arrows to go on, Escape to close, a caption and a counter. Each picture starts fitted, and an arrow pans once it is zoomed in.
- `CompareView`: two pictures side by side, with one that can be chosen, or across a divider that a drag and the arrows move. A drag paints without a build.
- `Eyedropper`: a button that arms, and `EyedropperArea` over a picture, which asks the caller for the color of the point clicked. `EyedropperState::pick` tells it directly. Escape disarms. It does not capture the screen.
- `CropOverlay`: a crop over a picture, built on `TransformHandles`, with a veil outside, thirds inside, a fine grid while it turns, a fixed proportion and an angle. It stays inside the content and hands back `CropArea`; the frame turns over a still picture, since the renderer has no transforms.
- `Protractor`: a band of ticks along an arc that sets an angle by a drag from anywhere on it, with a double click back to zero, the keyboard and the value in words.
- `TransformHandles`: eight handles, a turn and a pivot around a `BoxTransform`, in the units of a `ContentSpace`. Corners hold the proportions until Shift, Alt resizes around the pivot, and the keyboard moves, resizes and turns. It hands back geometry and turns no picture. A drag paints without a build.
- `Marquee`: the animated outline of a selection around a rectangle, a closed path or loose segments, which reads over any picture and asks for no frames with reduced motion.
- `RubberBand`: a rectangle drawn with a drag, in the units of the content, that goes through a `Binding` and selects on release. Escape cancels, and the keyboard moves and resizes it. A drag paints without a build.
- `RuleOfThirdsGrid`: composition lines over a picture, as thirds, equal divisions, squares or the golden ratio, in the units of a `ContentSpace` that can follow a `ZoomPanView`. It takes no input.
- `Minimap`: a small copy of the content with a rectangle around what is in view, by the same `ZoomView` as `ZoomPanView`, which a press, a drag or the arrow keys move.
- `ZoomPanView`: zoom and pan over an image or a drawing, by pinch, Cmd/Ctrl with the wheel, the wheel and a drag to move, a double click and the keyboard, with a coast that stops asking for frames and respects reduced motion. Gestures paint without a build.
- `ZoomControl`: fit, 1:1, the percentage and the buttons that zoom in and out, through a `ZoomView` that `ZoomPanView` and `Minimap` can share.
- `CurveEditor`: points you drag over a curve that never swings past them. A press adds one, a double click or Delete removes it, and the keyboard chooses, moves and adds. It can show a baseline and a histogram behind, and `CurveEditor::value_at` gives the same curve for applying it.
- `ColorWheel`: hue and saturation on a disc, by a drag or the arrow keys, with Shift to keep the hue and a double click to take the color out. The disc is computed once per size and shared.
- `Ruler`: horizontal or vertical, in the units of the caller, with scale, origin and marker, and a round step that keeps marks apart at any zoom.
- `Histogram`: overlapping channels from ready-made bins, whose colors add up as light does, in a linear or logarithmic scale, with a mark at each end that clips.
- `DropZone`: a dashed area around any content that takes dropped files, by the same rules as `FileUpload`, and says when files are over the window with a stronger edge and words. `choosable` lets the keyboard bring files through the system's chooser, and `quiet` keeps the dashes off until then.
- `ScrollEdgeArrow`: a strip that scrolls, horizontal or vertical, with an arrow over a fade at each end that has more to show. `Labels` gains `scroll_back` and `scroll_forward`.
- `FloatingToolbar`: a translucent row of commands over its content, placed by side, that takes the clicks that land on it; `movable` adds a handle, with the arrow keys and a double click that brings it home. `TitleBar`: title, actions and the window's buttons, drawing and calling back only. It does not drag the window nor hide the system's frame, which belong to the platform layer. `Labels` gains `window_minimize`, `window_maximize` and `window_restore`.
- `ValueSlider`: a value in small, as "100% ▾", that opens a `Slider` in a popover with the keyboard in it; with `field` the number can be typed. `InlineRename`: a name that turns into a field by a double click, F2 or Enter; Enter and leaving confirm, Escape cancels. `TextField::select_on_open`. `Labels` gains `value_adjust`, `rename` and `rename_hint`.
- `Chart`: under the pointer the other bars and slices fade, a slice comes forward, and the cursor and its dots glide between categories and fade out. `no_hover_focus` and `no_hover_motion` turn this off; less motion in the system does the same for the movement.
- `ChartTooltip`: `show_percent`, `value_color`, `no_indicator_for` and `content`. It takes the side with room on both axes and stays off what is in focus.
- `Chart`: `grid_dashed`, `grid_columns`, `value_ticks`, `align_axis_ends`, `value_domain`, `reference_line` and `reference` with `ChartReference`.
- `Chart`: `label_guides` puts the labels of a pie or a doughnut in columns with elbowed lines, none over another, and `slice_radii` gives each slice its own radius.
- `Chart`: `bars_from` with `ChartBarFrom` makes bars grow from the top or the right, `min_bar_length` keeps a least length, `labels_at_zero` puts categories on the line of zero on the other side from the bar, and `gradient` now fills bars from tip to base too.
- `Chart`: a `ChartKind::Scatter` or a sparkline given a `tooltip` gets the tooltip, the cursor, the dots in focus and the arrow keys. Without one it stays a plain picture that takes no focus.
- `ChartKind::Candlestick` and `Chart::candles`, with an axis of round numbers that does not have to start at zero. Rising candles are outlines and falling ones are solid. `Labels` gains `candle_open`, `candle_high`, `candle_low` and `candle_close`. Reports can draw the kind, as shapes, though none offers it yet.
- `SankeyChart`: how much flows from what to what, with `SankeyAlign`, ribbons in a gradient, the rest fading around the focus, arrow keys, and every flow read aloud. Flows in a circle say which nodes they go through in place of the diagram. Its layout is `ravenrs_graphics::sankey`; `Labels` gains `sankey_cycle`.
- `Semantic::ChartGrid` (`chart-grid`), `Placement::RightEnd`, `LeftStart` and `LeftEnd`.

### Changed

- The handle of a `Splitter` is now a stop in the Tab order, so its panes can be resized from the keyboard.
- `Chart` measures its labels with the real font, and its gridlines land on whole pixels. Reports draw as before.
- The gap between slices of a `Chart` is now really empty, so what is behind the chart shows through it, rather than being painted in the color of the surface.
- `Chart` draws bars as quads, with no picture, and gridlines stop at their sides, so a bar that fades shows none through it. A line with more points than columns draws straight lines through the extremes of each column, its picture covers only what it draws, and a new size costs far less. See `docs/QUALITY.md`.

### Fixed

- A stack in a `Chart` rounds whatever is on its ends, also when the last series is zero or the stack goes below zero, and an empty series leaves no line across it.
- Categories closer together than a pixel no longer pile up on the axis of a `Chart`.

## [27.1.12] - 2026-11-04

### Fixed
- Shapes of curves past what the coverage atlas holds in a frame are no longer left out. Only about 25 shapes the size of a 2560x1600 window fit before; now the main pass splits to draw more, in scene order.

### Added
- `examples/calculator`, a small screen for comparing performance between versions, measured by the gate.
- `ravenrs-perf capacity`: how much of each kind of work fits in a frame on this machine. `ravenrs-perf` also measures the Feature Center.

### Changed
- The finance example reads its history of versions only while the dialog showing it is open.

## [27.1.11] - 2026-11-04

### Breaking
- `FrameSample` has new fields, `work` and `render_work`, and `RenderStats` has new fields for draw call splits, bytes uploaded, the glyph atlas and GPU time. Building one field by field needs `..Default::default()`.

### Added
- The diagnostics panel shows a timeline of the latest frames, why the screen was rebuilt, the costliest types of view, draw call splits and GPU resources, and copies a capture that Perfetto opens. It now opens on the web too, with Ctrl/Cmd+Shift+D. [docs/PERFORMANCE.md](docs/PERFORMANCE.md) explains how to read it.
- `Ui::stats`, `Ui::take_frame_work`, `Ui::set_phase_clock`, `Ui::phase_times`, `Ui::set_view_profiling`, `Ui::view_costs`, `TextSystem::stats` and `Renderer::set_gpu_timing`, which measure the work of a frame. They cost nothing until asked for.
- `View::type_name`, the name a view's cost is shown under.
- `ravenrs-perf`, not published: a gate of exact counters for every scene against `perf/baseline.json`, part of `scripts/check.sh`, plus frame times by phase.

### Changed
- What is outside its clip is not painted. A scrolling list of 1,000 cards went from 68,904 instances per frame to 482, and scrolling it from 5.7 ms to 0.66 ms on the CPU.
- `TextSystem` keeps recent layouts, so a rebuild does not shape the same text again, and text cut to fit is not measured again on every paint.
- The pointer moving over views that never asked about it no longer rebuilds the screen.
- `Lottie` repeats only the painting while it plays, instead of rebuilding the screen. It now asks for frames at 60 Hz, like other animated drawing.
- A chart that returns to the screen unchanged keeps its picture: opening the finance example's dashboard again went from 25.4 ms to 1.1 ms.
- `RenderStats::draw_calls` now counts the draw call for the coverage of shapes.

## [27.1.10] - 2026-11-03

### Breaking
- `BugReport` has new fields, `kind`, `area`, `elements` and `errors`, so building one field by field needs `..BugReport::default()`. Older `.ravenbug` files still open.

### Added
- `ravenrs-lottie` and the `Lottie` widget, with the `lottie` feature, on by default: a Lottie player of our own, with no third-party vector code. It draws Bodymovin JSON as curves on the GPU, with shapes, masks, mattes, precompositions, trims, dashes, repeaters, gradients, embedded images and the Fill effect, and `LottieAnimation::unsupported` says what a file asks for that it does not do yet. Against lottie-web, on 16 files straight from LottieFiles, 13 match and 3 come close.
- `Primitive::Path` and `Scene::push_path`: any filled shape made of curves, with solid, gradient or image paint and a clip of another shape. Each pixel gets the exact area covered, with no tessellation or multisampling.
- `SplashScreen::reveal_after` holds the name and status back while an animated logo plays, keeping their places.
- `BugReporter` lets the person choose what kind of report it is and drag a box around the problem, which names what is under it and lights it in the film. All that goes along is listed before sending.
- `RecentErrors`: a `tracing` layer that keeps the last warnings and errors, for `BugReporter::errors`.
- `DataGrid::even_row`, `odd_row` and `horizontal_lines`.
- `SystemNotifier`, with the `notifications` feature: the system's own notifications on macOS, Windows and Linux.
- `BuildCx::ask_for_access_tree` and `UiActivity::Described`: what is on screen, for an observer, only when asked for.
- `BuildCx::keyed_overlay`: a layer that stays the same one when other layers before it come and go.
- `AppIdentity::close_to_tray` and `SystemTray`, with the `tray` feature: closing the window keeps the app running behind an icon beside the clock, and opening it again, or clicking its icon in the Dock on macOS, brings the same window back. Where the system shows no such icon, such as plain GNOME, closing the window closes the app.

### Changed
- `DataGrid::striped` is now visible in every theme, tinting rows with the control color, and starts counting again after each group row. Lists generated by `ravenrs-app-ui` are striped. Rows under the pointer and selected rows are a little stronger, to stay apart from the stripes.
- A sidebar opens and closes in 200 ms instead of over a second, and its names no longer squeeze on the way: the edge covers them. Submenus and other folding parts also take 200 ms.

### Fixed
- Dialogs and menus open in 120 ms without shivering, and leave by fading as one surface instead of vanishing. `Motion::surface_exit` does it for panels of your own.
- A selected pill or tab marker no longer comes loose when the layout around it moves, as when a sidebar closes.
- `SplashScreen` no longer starts over, showing the page behind it, when an app with layers of its own appears under it.
- A native window stays hidden until the GPU is ready and, on macOS, until its first frame is drawn, so nobody sees it empty or black.

## [27.1.9] - 2026-11-03

### Breaking
- `Query` in `ravenrs-app` has a new field, `only`, so building one field by field needs `..Query::default()`.
- A `FieldKind::Criteria` field is out of lists by default and in a group of its own on a form, as it takes the height it needs.
- Saving a form of a `transient` entity no longer goes to storage.
- `ActionResult` has a new variant, `RecordsChanged`, and `EntityInfo` new fields, `state_machines`, `reminder` and `editor_settings`.
- `SpaceError` has a new variant, `Denied`, and `FieldInfo` a new field, `secret`.
- `DataGrid` shows its group panel only when something is grouped, or with `group_panel(true)`; the Group menu takes its place in the toolbar. While rows are selected, the toolbar says how many instead of adding to the row count.
- `LookupField` and `ColorPicker` fill the width they are given, unless told a `width`. `DocumentViewer` follows the language of the application unless given `labels`.
- A `FrameTemplate` gets the divider inside `TemplateParts::header`, and `MainTemplate` adds no space between the parts.

### Changed
- **Screens reviewed against `examples/feature-center`**, with the rules now in `docs/design-system/README.md`: one accented action in a window, one page margin, page titles, a flat filter row lined up with its columns, fields that fill their column, collections with one heading and no owner column, full-width panels with their title and Close at the top, and a list that says it is empty instead of repeating its name.

### Fixed
- A long `Sidebar` scrolls and keeps its footer in the window, instead of stretching the whole window past the bottom of the screen.
- In the browser, a field that takes the keyboard when the page opens gets what is typed from the start, without a click first.
- A filter or rule in the model can use a function the application registered; checking the model refused it.
- Any `FieldKind::Guid` field gets its value when a record is created, and not only a key.
- A new form nobody typed in no longer asks about losing changes.
- A `DateTime` field shows as a date and a time on a list, and a yes-or-no column follows the application's language.

### Added
- **`ThemePicker`**, a menu of themes, and **`ThemeEditor`**, which changes the tokens of the theme on screen, says the contrast of text, and copies what changed as JSON or takes it back. `AppShell` shows the menu in the footer of its navigation, unless told `without_theme_menu`, and `examples/feature-center` has the editor. In `ravenrs-style`: `Theme::builtin`, `base`, `changed_tokens`, `to_tokens_json`, `from_tokens_json`, and tokens by name on `Semantic`, `Palette`, `Radii`, `TextStyle` and `Typography`.
- **`TextField::flat` and `end_aligned`**, and **`Alert::message`** for an alert with no title.
- **Rules that only warn.** `Rule::warning` asks and lets people go on, and `Rule::information` leaves a note on the field. A rule's `message` can say what the record holds, with `{field}` and `{reference.field}`.
- **Concurrent changes.** Saving a form keeps what someone else changed in other fields, with `merge_changes`, and asks about the same field. A record someone deleted says so.
- **Editing on a list.** `EntityInfo::list_editable` changes values right on the list, checking rules and marking cells that fail.
- **`DataAccess`** says how a list reads its records: all of them, as needed, or only its columns, through the new `Query::only`.
- **Custom fields.** A model difference adds stored or calculated fields, which get a column and a place on the form. `App::administrator_settings` and `App::storage_for` bring them to storage.
- **`FieldKind::Guid`** and **`FieldKind::Tags`**, and `predefined`, `edit_mask`, `criteria_of`, `display_format`, `group` and the `Progress` editor on fields.
- **`Message`** and `ActionContext::message` tell people something from an action. `ToastExtras` gives a toast a button and something to run once it is gone. `FormField::note_text` says something that is not an error.
- **`ActionCategory::InForm`** and `EntityInfo::form_actions` put actions inside a form's layout.
- **Records no storage keeps** get `use_records` and `use_collection`, can point to stored records, and a navigation item can open a form directly, with `ObjectKey` for one by its key.
- **`DashboardInfo::variant`** changes a dashboard to another from a menu.
- **`export_captions`** and **`import_captions`** take what people see out to CSV and back, for translating outside the code.
- **Security.** `SecuredObjectSpace` applies `Role`s to every read and write, with or without a screen: a policy per role, permissions on entities, records and fields, with criteria, and on navigation and actions; several roles combine in any or in all. A field marked `secret` never leaves it.
- **Signing in.** `SecuredApp` puts sign-in in front of an application, with users and roles managed on generated screens, Argon2id password hashes, lockout after five wrong passwords, a first administrator on a new installation and `Authenticator` for other ways in.
- **`AuditedObjectSpace`** writes who changed what, from what to what, in the same commit, with the modes `Full`, `Lightweight` and `CreationOnly`, fields left out by `AuditSettings`, and changes in collections shown on their owner.
- **`ModelEditor`**: the model as a tree, with its settings beside it, kept in the person's own layer, for a role that `can_edit_model`.
- **Lists shown other ways.** A view picks `Chart`, `Calendar`, `Scheduler` or `Tree` by `EditorType` in the model, or a `ViewVariant` by `editor`, with what each needs under `ChartSettings`, `CalendarSettings`, `SchedulerSettings` or `TreeSettings`, set in code with `EntityInfo::editor_setting`. A record moved on a timeline is kept with its rules and permissions. `examples/quick-plan` shows all of them.
- **`examples/feature-center`**: every feature of the application framework, one small demonstration each, with what to try on each list, and a test that goes through them all as three people, in memory and in SQLite.
- **`EntityInfo::description`** says what a list is there for, above its records.
- **State machines.** `EntityInfo::state_machine` declares states and the ways between them. Any other change is refused when it is kept, in memory and in SQLite, with no screen around it; each way gets its own action, and `state_appearance` goes by the state. `check_transitions` is for whoever writes another `ObjectSpace`.
- **Reminders.** `EntityInfo::reminder` brings up what is due in a window, to open, put off 15 minutes or dismiss. Nothing runs in between: the screen asks to be built again at the next time due. `ravenrs-app-library`'s events have one.
- **`local_now`** is the time on this machine as a clock reads it. The audit trail, lockout and the day pickers of the screens use it, as `DateTime::now` is UTC on the desktop.
- **Reports people keep.** `StoredReportsModule` keeps reports as records, on the data of an entity: a first layout comes from its list, `Designer::embedded` opens the designer inside the application, and showing one asks for its parameters and exports to PDF and XLSX. A list shows its records in a report with `ShowInReport`. Permissions on the records say who sees or changes a report, and a report only reads what the person can read.
- **Panels people put together.** `AnalyticsModule` keeps panels as records, with tiles of numbers and charts over the entities, edited on generated forms.
- **`ravenrs-report-designer`**: the report designer as a crate, out of `examples/report-designer`, which now just runs it.
- **`Session::show_full`** shows something in place of the view, with the navigation around it. `chart_view` and `summary_text` draw what `Chart` draws from settings given directly.
- **`Tabs`** go to another line when they do not fit, instead of running over each other.
- **`ravenrs-app-library`**: people, organizations, addresses, countries, tasks, events, notes and files, ready to use or extend.

## [27.1.8] - 2026-11-02

### Breaking
- Until 28.1.0 any release may hold breaking changes, listed here. Pin an exact version if that matters to you. See [docs/VERSIONING.md](docs/VERSIONING.md).
- `EntityInfo` in `ravenrs-app` is no longer `Send`, `Sync` or unwind-safe, since rules and appearance can hold closures.

### Added
- **`#[derive(BusinessObject)]`** and **`#[derive(BusinessEnum)]`** declare records as Rust structs and options as enums, in the new `ravenrs-app-derive`. A wrong attribute is a compile error that points at it, and a calculated field's expression is checked there too. `ObjectSpaceExt` reads and writes these structs.
- **`App`** in `ravenrs-app-ui` puts an application together from its records in a few lines, with the language of everything in it. [`examples/quick-app`](examples/quick-app) is a whole one in under 100 lines, on desktop and in the browser. The guide is [docs/APP-FRAMEWORK.md](docs/APP-FRAMEWORK.md), and [docs/APP-PARITY.md](docs/APP-PARITY.md) says what exists.
- **Validation.** `Rule` covers required, range, pattern, text and value comparison, criteria, is true, unique, unique together, exists, referenced and custom, by context and target criteria. A generated form that fails keeps nothing, and says what is wrong on each field and in a list on top. `#[field(required, unique, min, max, matches)]` and `#[rule(...)]` declare them.
- **Conditional appearance.** `AppearanceRule` changes font, background, weight, enabled and visible on fields, groups and actions, by criteria or function, in order of priority. `#[appearance(...)]` declares them.
- `DataGrid::cell_style` decides how each cell looks from its row. `FormField` gains `error_text`, `label_ink` and `tint`, and `Label` gains `ink`.
- **`ravenrs-app-sqlite`** keeps records in SQLite, on tables it creates or on tables an application already owns, sharing its connection. Filters run in the database as bound values, and what the database refuses comes back in words.
- **`EntityPane`** is a list beside its form, for a screen an application made itself.
- `DataGrid::name_column` says what names a row for its buttons, and a filling column leaves room for them. `Select::fill` takes the width of its parent. `ChangeSet` and `run_query` are there for whoever writes an `ObjectSpace`.
- **Relations.** `FieldKind::Collection` shows on a form the records that point to it, with `Link` and `Unlink`. A reference with the `Select` editor is picked from a list, narrowed by `lookup_filter` from the record being edited, with `none_caption` for empty.
- **`EntityForm`** is a form in a window that checks its rules and hands over the record, for what an application keeps through its own rules. **`ActionButton`** shows one action as a button, and `Session::embedded` lets a screen made by hand use lists and actions of the framework.
- **Popups with forms.** `Action::popup` on a detail view asks for a record to be filled in, started by `on_prepare` and received as `ActionInput::Record`. A parametrized action reads what is typed as the kind it asks for, and `confirmation_words` says what its question says.
- **`DashboardInfo`** and **`DashboardView`** put views and registered controls together, each in a frame of its own. **`ViewVariant`** gives a list ways to be seen, picked from a menu, with the choice in the person's layer of the model.
- `ListView` gains `filter`, `sort`, `column_shape`, `grid_shape`, `single_selection`, `plain_toolbar` and `remote`, which brings rows through `RemoteRows`. Pictures and files on a form have real controls.
- **`ravenrs-app-report`** gives every list an action that lays it out as a report and shows it in the viewer. `Session::list_table` and `ravenrs_grid_export::document_of` are what it stands on.
- **List editors.** `register_list_editor` adds a way to show a list, picked by the model. `ListFilter`, `list_criteria` and `ViewVariant::columns` put filters and columns in the model, and `register_function` adds functions to every expression.
- **The model does more.** Rules can be set under an entity, `translation` and `set_language` change its words, and `SqliteModelStore` or any `ModelStore` keeps a person's settings through `use_model_store`.
- **`CloneModule`** clones what says it can be, with `non_cloneable` and `aggregated`. **`AttachmentStore`** keeps files apart from records, with `MemoryAttachmentStore` and `SqliteAttachmentStore`, and a form saves a file back out.
- `SqliteBuilder::migration` runs something once, `managed` adds columns for new fields, and `entities_from_schema` starts from an existing database.
- `checks_on_leave` validates as fields are left, through `on_leave` on `TextField` and `NumberField`. `transient`, `ListView::source`, `ApplicationBuilder::service` and `Module::initial_records` complete the framework's core. The derive has attributes for all of it, with `#[variant]` and `#[list_filter]`.
- `DatePicker::fill` takes the width of its parent. `ravenrs_report::base64` is public.
- `FilePicker::save` hands a file to the person: the system's dialog on a desktop, a download in a browser. Exports from generated lists use it unless the app takes them.

### Changed
- The registers of the Raven Finanças example are generated from what they are, and say what is wrong when a name is missing instead of doing nothing.
- The transactions and bills of the Raven Finanças example, their lists, forms and commands, and its plans and payments are generated from what they are too, with every write still going through its database rules.

### Fixed
- A field with options starts on the first one, which is what it shows, instead of holding nothing.

## [27.1.7] - 2026-11-02

### Added
- **Editors for business forms.** `TextArea`, `NumberField` (also money in whole cents), `MaskedField`, `TimePicker`, `DateTimePicker` and `LookupField`, whose options can come from a database or a server.
- **`FormLayout`** arranges fields in groups and columns that collapse on narrow windows, with tabs and a list of errors.
- **Commands.** `Toolbar` folds what does not fit into a menu, `SplitButton` offers alternatives to a main command, and `ConfirmDialog` asks before something final.
- **`FilterBuilder`** writes filters from lines of field, comparison and value, in the expression language, and keeps what it cannot show as a line.
- **`DataGrid` over records.** Columns can name their field (`GridColumn::field`) and hold dates (`GridColumn::date`, `Value::Date`). `key_column` and `selected_keys` keep a selection across data changes, `on_row_open` gives the row's values, `on_cell_edit` can refuse an edit and say why, and `row_action`, `on_add_row` and `on_delete_rows` add commands for rows. `GridData` gains `insert` and `remove`.
- **`Hotkey`** makes a key combination run a command, as `ToolbarAction::hotkey`, also from a folded toolbar.
- **`TreeView`** opens, selects, checks in three states, filters, brings items on demand and builds only the lines in sight. Screen readers hear it through `Role::Tree` and `Role::TreeItem`.
- **`TagBox`** picks several values as tags, **`ColorPicker`** offers a palette, a shade square, hue and opacity bars and hexadecimal, and **`DateRangePicker`** takes a period in two clicks or ready-made.
- **`Wizard`** goes through a task in steps, each able to refuse to be left and say why.
- **Files.** `FileUpload` and `ImageField` take files from the system's chooser or dropped on them, on desktop and web, and say why they refuse one. `BuildCx::file_picker`, `BuildCx::on_file_drop` and `Ui::set_file_picker` let any view do the same.
- **`TileLayout`** arranges a dashboard of tiles to drag and resize, also from the keyboard. **`CardView`** shows records as cards in as many columns as fit, and builds only those in sight.
- **`DataGrid` row details.** `row_detail` opens a view below a row, with the arrow or the Right key.
- **`DataGrid` to Excel and PDF.** `on_export_file` shows a menu of formats and hands over what the grid shows; the new `ravenrs-grid-export` turns it into a file through the same exporters as reports. `wizard::table_report_with` lets a table report take captions and formats from the caller.
- `DataGrid::filter_builder` puts a `FilterBuilder` in the toolbar, and rows on a server can be edited from the keyboard, with Enter or F2.
- **`ravenrs-app`**, the start of an application framework with no GUI yet. `EntityInfo` declares a kind of record; `generate_model` gives it a list, a list to pick from, a form and a place in the navigation; `ModelLayers` changes that from modules, an administrator and a user, each as a difference in JSON; `ObjectSpace` and `MemoryDatabase` read and write records in a unit of work. `Action` comes in five kinds, each on or off by reasons, by the view it targets, by what is selected and by criteria over it; `ViewController` and `WindowController` bring actions and react to views; `Frame` holds a view and turns them on and off in a fixed order; `Module` and `Application` put it all together and say what is wrong before any screen opens.
- **`ravenrs-app-ui`** turns the model into screens: `AppShell` with navigation, `ListView` over `DataGrid`, `DetailView` over `FormLayout` with a control for each kind of field, `ActionContainer` over `Toolbar`, and `SystemModule` with new, save, cancel, delete, refresh, back and moving between records. `FrameTemplate` rearranges the window, and `EditorRegistry` changes a field's control. `ObjectSpace::insert` refuses a key already in use.
- `BuildCx::report_frame` tells a view the room it got, for those that arrange themselves by it.
- `disabled` and `read_only` on `TextField`, `Select`, `Combobox`, `DatePicker`, `Checkbox`, `Toggle` and `Stepper`. Screen readers hear about read-only controls through `AccessNode::read_only`.

### Changed
- `Ribbon`, `Carousel` and `InputOtp` take their words from `Labels`, so they follow `Localized`.

### Fixed
- A view that builds from the size of its scroll container, as `DataGrid` does, gets a new build in the same frame when that size changes, instead of waiting for the next input.

## [27.1.6] - 2026-11-02

### Added
- **Touchpad pinches on Wayland.** winit 0.30 delivers them only on macOS and iOS, so the new crate `ravenrs-wayland-gestures` listens on winit's own connection. It holds the workspace's only `unsafe` call, and goes away with a winit that delivers them.

### Changed
- `Chart`'s tooltip glides from one place to the next, and follows the pointer along what is free, instead of jumping.

### Fixed
- `DocumentViewer` adds up pinch movements too small to change a whole percent, which a touchpad can send and were being lost.

## [27.1.5] - 2026-11-02

### Added
- **Every pattern of the shadcn/ui chart catalog.** `Chart` draws all 70 of them, in the gallery under their names: `curve`, `stack`, `gradient`, `horizontal`, `vary_colors`, `negative_color`, `active`, `dots`, `dot_icon`, `value_labels`, `category_labels`, `grid`, `center_text`, `concentric`, and more.
- `ChartKind::Radar` and `ChartKind::RadialBar`, in charts and in what reports draw.
- `ChartTooltip` says what is under the pointer or the arrow keys, with indicators, formats, icons and a total. `Chart` takes keyboard focus for it; `hide_tooltip` turns it off.
- `ChartCard` is a chart's card: title, description, what changes it, tabs with totals, and how it trends.
- `Semantic::Chart1` to `Chart5` are series colors for each theme. Charts no longer use status colors, so a series does not look like an error.
- `ravenrs_graphics::plot` lays these charts out, and says where each value landed. `vector::rasterize_layers` adds gradients and dashes to what a picture can have.
- `BuildCx::on_pointer_move` says where the pointer is over an element.
- `Pagination::compact` shows "1 / 12", so its buttons stay in place.
- **`DataGrid` grouped by dragging.** A header dropped on the group panel groups by that column; chips change places by dragging, and leave by dragging off the panel. Expand all and collapse all. Group rows line their summaries up under their columns, and a grouped column shows in its groups.
- `DataGrid` columns move by dragging, stay in place with `GridColumn::pinned`, and have a menu on a right click or from their header. A filter by value lists what is in a column, and a panel says what is being filtered.
- **Scenarios.** `DataGrid::scenarios` saves how a grid is arranged under a name, with `GridScenario` and `GridLayout` as JSON; `DataGrid::layout` keeps the arrangement outside.
- **Summaries with formulas.** A click on a total changes what it sums up, or takes a formula such as `Sum([Seats]) / Count()`, checked as it is typed, over each group's rows. `GridColumn::summary_formula` gives one in code.
- **Pages and rows on a server.** `DataGrid::page_size` shows pages. `DataGrid::remote` with `RemoteRows` asks for what is on the screen once scrolling rests, keeps a limited number of chunks, drops late answers, needs no total, and hands the loader the row before a chunk so that a query can continue from it.
- `examples/grid-poc` shows all of this working.
- `Chat::busy` holds what is sent until an answer arrives.
- **Narration recorded ahead of time.** `Narration` holds recordings of what films say, compressed to about 8 KB a second, and `MoviePlayer::narration` plays them in place of the system's voice, which still says whatever has no recording. `Sound::speak_clip` says a recording that `stop_speaking` can stop.

### Changed
- The legend of a `Chart` is real text under or over it, and numbers on the axis are optional: `show_value_axis`.
- A screen reader hears every series of a `Chart`, not only the first.
- `DataGrid` has a quieter look: lower header and footer, gridlines only between rows, column dividers only in use, and rows that go in under their groups.
- `DocumentViewer` uses the compact pagination, so another document moves nothing in its toolbar.

## [27.1.4] - 2026-11-02

### Added
- **Speech with LFM3 Audio.** `SpeechEngine` runs the 2B model on the CPU in plain Rust, as its model card says: 19 languages and locales, two voices, and the card's sampling values. `LocalVoice` says text a sentence at a time on its own thread, `VoiceManager` is the screen to choose the model, language, voice and fine settings, and `Chat::on_listen` puts a button to hear each answer.
- `Scheduler::card` is the look of a full scheduler, as a card among other cards: a tinted header with the month over the days of the week, a line between the rows' names and the timeline, today's column marked, and bars in a soft tint with a stripe of their color. `Scheduler::event_detail` puts a second line on the bars, and `Scheduler::avatars` the initials beside the rows' names.
- `Scheduler` scrolls by itself while a drag is near an edge, zooms with a pinch or Ctrl and the wheel, scrolls with the wheel over its names, and brings into view the event keyboard focus reaches. `BuildCx::scroll_with_widget` and `shortcut_with_shift` are new.
- `Scheduler::event_menu` and `lane_menu` offer menus on a right click, and with `card` a tooltip shows an event's whole name and where a drag is taking it, and the row under the pointer lights up.

- `HelpTip` is a small "?" that explains what something is and how to use it, under the pointer, on keyboard focus and, after a click, until a click elsewhere.
- `Sidebar::rail` opens and closes it from a strip along its edge, `SidebarBrand` is an app's name that keeps only its mark when reduced to icons, and `SidebarItem::shortcut` shows the keys that do the same.

- `MoviePlayer` plays at three speeds, jumps with the arrow keys, shows where chapters begin on its timeline, which step of how many it is at, the chapters already passed, and at the end offers to watch again or `next`. `Slider::marks` draws the notches.

### Changed
- `FloatingPanel` no longer gets in the way of what is on the page: `BuildCx::floating_overlay` keeps it under every menu and dialog, wherever it is built, and it keeps what is written when they open. Escape closes the last menu or dialog before anything that listens for it.
- `LocalAssistant` stops waiting after a minute, and `too_slow` says so: from then on it answers nothing, at once, so an app can go on without it. A choice also ends as soon as its JSON closes.
- `Recorder::guide` brings the pointer to what a step asks for before stopping there.
- A scrollbar shows only around a scroll and then fades, so it no longer sits over what it scrolls at rest. `ScrollView::scrollbar_outside` puts it in the margin around the view.

### Fixed
- What leaves the screen keeps its stacking: a closing dialog could pass under its own backdrop, depending on the run.
- `Avatar`'s initials can be read in a light theme.
- Dialogs, menus, panels and toasts stay solid while they come and go: fading, they showed the page through their fields, in a gray like metal that stands out in a light theme.

## [27.1.3] - 2026-11-02

### Added
- **`Chat` and `FloatingPanel`.** `Chat` is a conversation over a field to write in, with examples and cards to act on; it does not know who answers. `FloatingPanel` is a round button in the corner of the window that opens a panel over the app, which stays usable behind it. Dragging it or pressing an arrow key sends it to another corner, and it asks for frames only while the pointer or the focus is on it. `Labels` gains `send`.
- **Local AI models.** `ModelPicker` lists models to download, try, use and remove. The new crate `ravenrs-ai`, desktop only, does the work: `ModelManager` offers only the models that fit the machine's memory and disk and tests their speed, `ModelFolder` downloads each when someone asks, checking its hash, and removes it, `LocalEngine` runs one on the CPU with no extra build tools, and `LocalAssistant` keeps it on its own thread, and `tool_rules` and `read_tool_call` let it choose among what the app offers, which is all it can do. Nothing else talks to the network.
- **`MovieChat`.** A `Chat` that answers from the training films: each answer is a card with the moments that open. It looks only at the words the films hold, on this machine, and `MovieChat::writer` can have a local model write the answer about what it found. `ChatMessage::assistant_writing` shows an answer as it arrives, and `MovieAnswers` answers about the films for a chat that also does other things. `MovieAssistant::cue` lets it open a moment on the same player.
- `Alert::action` puts a button beside the message, giving an `Alert<AlertAction>`; an alert without one stays `Send`.
- **LFM3 in `ravenrs-ai`.** The catalog offers LFM3 1.7B, which `LocalEngine` runs with its own implementation, since candle stops at LFM2. It needs 4 GB of memory at the peak and chose the right tool in 27 of 32 requests. Its license allows commercial use only below 10 million dollars of annual revenue.
- `GridColumn::fill` takes the width the columns leave, and `DataGrid::empty_text` says what no data means, apart from filters leaving nothing.
- `Scheduler::fill_rows` shares the height left among the rows.
- `Stat::large` for the one number a screen is about.
- `MovieAssistant::height` and `MoviePlayer::max_film_height` keep a film and its controls in view; the film's choice becomes a select when its names do not fit. A film that has not started shows a play button over its picture.

### Changed
- `FloatingPanel`'s button is smaller and calm: the theme's surface with an accent icon, in line with its panel, which lines up with the page's margin.
- `Chat` sets apart only what the person asked; the answer sits on the panel, and a card takes the whole width.
- `MovieChat` answers in words first, then the moments; it and `MovieAssistant` leave out the steps inside the chapters they found. Answers open over the film instead of pushing it down.
- A film's chapters show how long each one takes, and the current one what it teaches; a player with boxes over its film no longer repeats that under it.
- `Badge` text moves toward the ink, to keep 4.5:1 on its tint.
- `read_tool_call` accepts a tool's name written with accents or capitals, as models do.

## [27.1.2] - 2026-11-01

### Added
- **Sound.** `Sound` plays short clips and speaks with the system's voice, through `BuildCx::sound` and `Ui::set_sound`, on the desktop and in the browser. `SoundClip::click` and `SoundClip::key` are computed, so they add nothing to an app's size. On Linux, compiling now needs ALSA's headers.
- **Films that talk.** `MoviePlayer` plays the clicks and keys of a film, reads what it says aloud, and shows it in a box next to what it talks about, with a switch for each sound. `Recorder::intro` and `Recorder::explain` time their boxes to the text. `Recorder::guide` now also waits for its text to be read, which the player skips while it stops at steps; a film recorded before this plays as it did, with no key sounds.
- **Bug reports from the diagnostics panel.** `AppIdentity::new_issue_url` adds a button that opens a new issue in the browser with the report filled in, cut to a safe length, for the person to review and send. Nothing is sent by the app, and the panel says what the report has before it opens. `ravenrs::native::open_url` opens a web page, and `ravenrs_core::new_issue_url` and `redact_user_paths` are the plain functions behind the button.
- **Performance test.** The diagnostics panel has a "Test performance" button, and `Diagnostics::start_test` starts it from an app's own screen: it redraws the window for 5 seconds and says in plain words whether the app is fluid, how many frames per second it drew, and whether the time goes to drawing or to building the screen. `WindowOptions::diagnostics` shares the panel with the app.
- **Diagnostics on the desktop.** Ctrl+Shift+D (Cmd+Shift+D on a Mac) or `RAVENRS_DIAG=1` opens a panel in any app with the GPU in use, GPUs left out for their drivers, what is wrong and what recent frames cost, with a report to copy into a bug report. `ravenrs::native::diagnose` gives the same GPU report without showing a window. Closed, the panel costs nothing. `WindowOptions::labels` sets the words of the whole UI, this panel included.
- **Expressions in the grid's filter row.** A `DataGrid` filter can be a `ravenrs-criteria` expression over column titles, such as `[Seats] > 10 And [Country] = 'Brazil'`. The short syntax (`>=15`, `5..12`, `sim`, plain text) works as before.
- `ravenrs_criteria::NumberStyle` lays out digits with any separators and affixes; `Culture` and the grid's `NumberFormat` both use it.
- `ravenrs_core::csv::push_field` quotes a CSV field; the grid and report exports both use it.
- **Charts in the UI.** `Chart` draws line, area, bar, pie, doughnut, scatter and sparkline charts with the layout reports use. The shapes become a picture at the screen's density and the labels are real text. Screen readers hear the values.
- **Barcodes in the UI.** `Barcode` shows a QR Code, Code 128, Code 39, EAN-13, EAN-8, UPC-A or Interleaved 2 of 5 with every module on whole pixels. Text a format cannot hold shows why.
- `Paint::Rich` draws with a `PaintCx`, for views that shape text while painting or need the device scale.
- `charts` feature on `ravenrs` and `ravenrs-widgets`, on by default. `ravenrs` also forwards `decode`.
- `ChartKind::SparkArea`, a smoothed line over a filled area, and `ChartStyle` with `chart::draw_styled` for colors, bar corners and the widest bar.
- **`MovieAssistant`.** Someone types a question and the moment of a training film that answers it opens. `MovieLibrary` searches the chapters and steps the films already hold, ignoring accents and case and accepting near forms of a word and typing slips, with no model and no network, on the desktop and in the browser. `MovieCue` opens a moment of a `MoviePlayer` from outside it. `ravenrs_text::fold` is the folding that searches compare with, shared with `DocumentViewer`.
- **Training films (`ravenrs::movie`, crate `ravenrs-movie`).** `Recorder` runs the real app from a script and stores what it drew, with no pixels, no GPU and built-in fonts by reference, so films are small enough to ship inside an app and stay sharp at any zoom. `MoviePlayer` plays them with chapters, marks to click and steps that wait. `MovieEditor` edits them on a timeline in the style of screen-recording editors: cuts, speed, title cards, callouts, captions, spotlight, hiding regions, zoom and pan, fades, pointer highlight and clickable spots. There is no sound yet.
- **`Timeline` and `ToolRail`.** `Timeline` shows blocks along time on named tracks, under a ruler with markers and a playhead, and reports moves and trims. `ToolRail` is a narrow bar of tools with their names under their icons. `IconButton::disabled` makes an icon button inert.
- **Bug reports with a film.** `FlightRecorder` keeps the last minutes on screen in memory, and `BugReporter` is the dialog that lets someone watch them, cover or cut parts, and describe the problem. Nothing leaves without them, and where it goes is up to the app.
- `BuildCx::observe_activity` watches painted frames, the pointer and clicks across the whole UI. `ravenrs_text::builtin_font` and `builtin_font_number` name the built-in fonts across runs.
- `ravenrs::native::OpenDialog` shows the system's dialog for choosing a file to open.
- `ravenrs::native::run_as` opens the window as an `AppIdentity`, whose id ties it to the app's `.desktop` file. Without it, GNOME on Wayland shows no icon for the window, since Wayland windows cannot carry one.
- **`StatusBar`.** One fixed line at the foot of an app window: its sides keep their width, the center takes the rest and cuts what does not fit, and `center_tone` tints only the center. `ButtonSize::Mini`, 22 px, is what fits on it, and `Toaster::bottom_inset` lifts toasts above it.
- **`ReleaseNotes` and `CodeBlock`.** `ReleaseNotes` shows an app's versions, the list on the left and what the chosen one brought on the right, with buttons that walk through them. `CodeBlock` shows a command to copy. `Labels` gains their words.
- **Waking the UI from another thread.** `UiWaker`, from `BuildCx::ui_waker`, asks for a rebuild from any thread, so background work needs no polling. `Ui::set_remote_waker` connects it to a platform.
- `ravenrs::native::SaveDialog` shows the system's dialog for choosing where to save, and `ravenrs::native::open_with_system` opens a file with the system's program.
- **Pinch to zoom.** `Event::Pinch` carries a trackpad pinch, or Ctrl with the wheel, and `BuildCx::on_pinch` receives it. `DocumentViewer` zooms with it.
- **Printing from the viewer.** `DocumentViewer` has a print button, on by default, and `ravenrs_report_viewer::print` prints a document on the desktop and in the browser. The designer uses the same code.
- `Color::faded` makes a color less opaque relative to what it already is, which is what a theme's translucent tokens need.
- `ravenrs::native::run_with_icon` opens the window with an icon, for the title bar and task bar on Windows and Linux.
- **`Scheduler`.** A timeline of resources on the model of Bryntum Scheduler: events as bars that drag along time and between rows, resize by their ends and are drawn on an empty stretch, with `can_drop` to refuse a place, zoom from months to hours, lanes for overlaps, shaded weekends, collapsible groups, keyboard control, and only what is in view built. `TimeAxis` and `TimePoint` describe its time. What is still missing is listed in [docs/PARITY.md](docs/PARITY.md).
- **`EventCalendar`.** A month with its events inside each day.
- **Recurrence.** `RecurrenceRule` says on which days something repeats, with no UI, and reads and writes iCalendar's `RRULE`. `RecurrenceEditor` edits one and says it in words.

### Changed
- `ravenrs-widgets` now depends on `ravenrs-criteria`, without its JSON feature.
- A filter-row text that names a column in brackets and parses, such as `[Seats] > 3`, is now an expression instead of a search for those characters. Other text in brackets, such as `[draft]`, is still a search.

### Fixed
- **GPU choice on Linux.** The native window gives wgpu its display, so the GL backend can present on Wayland when no Vulkan driver is usable, instead of leaving only software rendering. A warning now says when the app renders on the CPU, and `RUST_LOG=ravenrs_platform_native=debug` logs how long each frame's update, acquire, render and present took.
- **The app could draw on the CPU with a real GPU present.** No power preference was asked for, so wgpu took the first adapter it found: on an old Intel GPU on Linux, whose Vulkan driver it hides, that was software Vulkan, at about 11 frames per second, instead of the GPU on GL. It now asks for the GPU that spends the least, and `WGPU_POWER_PREF` overrides that. An adapter that refuses the limits no longer stops the app while another one works.
- The wheel over an open `Dialog` or `Sheet` no longer scrolls the page behind it, as a pinch already did not.
- In the glass theme, the `DataGrid`'s header, footer and group rows and the lines of `DataGrid`, `Ribbon` and `Scheduler` came out far too opaque, with the header text unreadable: `with_alpha` replaced the token's own transparency.
- A `Select`, `DatePicker` or menu inside a `Dialog` or `Sheet` opened in the window's corner, behind the dialog's backdrop. Overlays opened from inside another now come after it: they sit by their anchor and on top, keep focus, and Escape closes only them.

## [27.1.1] - 2026-11-01

### Added
- **Family finance example.** `examples/family-finance` is a desktop app for a family's expenses and bills to pay on SQLite, with reports, expression filters in SQL and updates that compile on the user's machine. It exists to show the framework and to find what is missing in it; see its [README](examples/family-finance/README.md).
- **Components in other languages.** `Labels` holds the words the built-in components use and `Localized` applies them to everything inside, so an app sets its language once. `Labels::pt_br` comes ready.
- **A fuller `Sidebar`.** It closes to a rail of icons or off the screen with `SidebarCollapse`, by Cmd/Ctrl+B or a `SidebarTrigger`, sharing a `SidebarState` that an app can drive with its own binding. It has `header` and `footer` around content that scrolls, `side`, `SidebarVariant::Floating` and `Inset` with `SidebarInset`, and with `narrow_below` small windows show it as a sheet. `SidebarGroup` folds and carries actions; `SidebarItem` gains `text`, `badge`, `disabled`, `action` and submenus with `item`; `SidebarSkeleton` holds the place while items load. On the rail, labels become tooltips, on hover and on keyboard focus. Arrow keys move between rows and fold them. Sidebars that use none of this look as they did.
- `Role::Navigation`, for navigation landmarks, and `Placement::RightCenter` and `LeftCenter`.
- `SplashScreen` covers the window while the app loads, with a logo or icon, the app's name, a status that screen readers announce, and a spinner or progress bar. It leaves with a fade, without the zoom under reduced motion.
- `Footer` ends a page or an app with left, center and right areas and a hairline on top. The center stays centered between uneven sides, and in a narrow window the areas stack. `Role::ContentInfo` is its landmark for screen readers.
- `Align::Fit` keeps a child's own size without overriding a parent that centers its children.
- `Stat::delta_toned` separates a change's direction from whether it is good, for numbers such as spending.
- **Report server.** `ravenrs-report-server` stores layouts with versions, renders them over HTTP, and sends scheduled reports by email or to a folder as PDF, XLSX or CSV. Jobs have cron schedules with time zones, parameters computed by expression, skip if empty, retries that do not send twice, retention, and a history of who received what. See [docs/REPORT-SERVER.md](docs/REPORT-SERVER.md).
- `ReportEngine::count_rows` says how many rows a report would print.
- **Document viewer.** `ravenrs-report-viewer` has `DocumentViewer`, with pages, zoom, fit to width, thumbnails, and search with highlights that ignores case and accents, and `ParameterPanel`, with a control for each parameter kind and apply and reset. The report designer's preview uses both.
- **Charts in reports.** `ControlKind::Chart` draws bar, line, area, pie, doughnut, scatter and sparkline charts from the rows in scope or a nested list, adding up rows with the same argument. PDF and SVG write them as vectors with real text; the screen and XLSX get a picture, with the text still in cells. The designer has a chart button and a "Painel de vendas" template.
- **Barcodes in reports.** `ControlKind::Barcode` prints Code 128, Code 39, EAN-13, EAN-8, UPC-A, Interleaved 2 of 5 and QR Code from fixed text or an expression, adding check digits. PDF and SVG draw them as vectors and XLSX as one picture.
- **Shared drawing logic.** `ravenrs-graphics` holds vector shapes, chart layout and barcodes, and depends on no engine. `Item::Vector` carries its shapes to every output.
- **One way to export.** `ravenrs-report-export` turns a `Document` into a file with its name and media type; the designer and the server both use it.
- `Scope::current_row` and `Scope::current_rows` read what a scope holds.
- **Pictures in reports.** `ControlKind::Picture` shows a PNG or JPEG from the layout or a field, fitted by `PictureMode`. A PDF keeps a JPEG's original file and stores an image once however often it prints; XLSX and SVG show them too. `decode_picture` and `picture_rect` are public, and the designer inserts and chooses images in the browser.
- **Links and outline.** `ControlKind::Link` opens an `https:`, `http:` or `mailto:` address from a PDF, a spreadsheet or SVG; any other scheme is refused and reported. `ControlKind::Bookmark` adds entries, nested by level, to the PDF's outline. `Item` gains `Image`, `Link` and `Bookmark`.

### Fixed
- A control that took keyboard focus while scrolled out of view stayed hidden. Tab, arrow keys and `request_focus` now scroll the containers around it, nested ones included, until it shows; a click still does not scroll.
- A full-window layer with nothing to focus left keyboard focus on the page behind it, where Enter could still press a button.
- `Button`, `Badge`, `DatePicker` and other controls ignored a stack's `Align::Center` and sat at its start.
- `None` and other empty views took a gap in their stack.
- `DocumentViewer` worked out its width from the whole window, so beside a sidebar the page was off center and scrolled sideways.
- A pie or doughnut chart listed categories with no slice in its legend.

### Changed
- The site roadmap, `docs/PARITY.md` and `ARCHITECTURE.md` now reflect what shipped.
- **`BarChart` and `AreaChart` look different.** Their API is the same, but they now draw through `Chart`. `BarChart` gains a numeric axis and has flat bars instead of a gradient. `AreaChart` has a flat fill instead of a gradient, and it no longer reaches the frame's left and right edges. Their boxes are the same size.
- `BarChart`, `AreaChart` and the new components need the `charts` feature. Apps that use the default features need no change. If you set `default-features = false` on `ravenrs-widgets` and use these charts, add `features = ["charts"]`. On `ravenrs`, `default-features = false` now also turns off `decode` and `charts`, which it could not do before.

## [27.1.0] - 2026-11-01

### Migrating from 26.2

This release is ahead of the usual calendar because these changes are breaking, and breaking changes ship only in a `YY.1` release. Most apps need no change.

- `Theme` has a new field, `motion`. A `Theme` written out field by field needs it: add `motion: Theme::raven_dark().motion`, or start from a built-in theme and change what you need.
- `Document` has `culture`, `Page` has `page_header` and `page_footer`, and `TextBlock` has `align`, `value` and `format`. Code that builds these by hand needs the new fields; with `Document` and `Page`, `..Default::default()` covers them.
- `Paint` has a new variant, `Live`, and is now `#[non_exhaustive]`. A `match` on it needs a `_` arm.
- `Card` and `GlassCard` are no longer `Send`, `Sync`, `UnwindSafe` or `RefUnwindSafe`, because they can hold a click handler. Views live on the UI thread, so this matters only to code that required those traits explicitly.

### Added
- **PDF export.** `ravenrs-report-pdf` writes a report `Document` as vector pages with subsets of its fonts embedded and selectable text, in pure Rust, on servers and in the browser.
- **XLSX and CSV export.** `ravenrs-report-xlsx` lays a `Document` out on a grid that follows the page, with real numbers and dates in the cells and report formats translated to Excel's. CSV uses the culture's separator.
- The report designer exports to PDF, Excel and CSV from the ribbon, the File menu and the preview toolbar.
- `Document::culture`, `Page::page_header`, `Page::page_footer`, and `TextBlock::align`, `value` and `format`, for exporters. `TextLayout::glyphs` reports shaped glyphs with the text they stand for.
- **A page for each component on the site.** Every component has a live example and its code. `samples.rs` in the gallery holds the examples, the gallery shows one alone with `?component=Name`, and `scripts/gen-component-pages.py` builds the pages at deploy, failing if a listed component has no example.
- **Component list on the site.** `site/components/` lists every component by category, with what it does, a link to its API, an A to Z index and a filter. `scripts/check-component-list.py` fails the site deploy when a link breaks or a public component is missing.
- **Motion.** The new `ravenrs-animation` crate has closed-form springs, CSS-compatible curves, OKLab color mixing and `Animated<T>`, which changes target with no jump in position or velocity. `BuildCx::motion` and `Motion` declare paint and layout transitions, shared layout, effects, and enter and exit animations. While only animations run, frames repeat the paint phase alone. Also new: `Collapse`, `BuildCx::animated`, `Paint::Live`, `Effect`, `Scene::apply_effect`, `Theme::motion`, `Ui::is_animating` and `Ui::set_reduced_motion`, which the browser now drives from `prefers-reduced-motion`.
- Built-in components animate: buttons, toggles, check boxes, radio buttons, segmented controls, tabs, disclosures, accordions, sidebars, text fields, menus, popovers, tooltips, dialogs, sheets, the command palette, toasts, carousels, progress bars, spinners and skeletons. Switching themes crossfades.
- **Skill and MCP server for AI assistants.** The Claude Code plugin in `plugins/ravenrs` brings a skill on building with RavenRS and `ravenrs-mcp`, a read-only server that works offline. Its tools are `list_components`, `component_api`, `component_examples`, `design_tokens`, `parity_status`, `validate_expression` and `expression_functions`. It is installed from git, not published to crates.io, and nothing in the framework depends on it. See the [README](README.md#ai-assistants).
- `ColorSwatch`, a named color to pick from a palette.
- `Text::truncate` and `BuildCx::text_truncated` keep text on one line that ends with `…`; screen readers still get the whole text. `TextSystem::layout_ellipsized` is the underlying call.
- `Card::on_click` makes a whole card a button, with hover, focus and keyboard support.
- `SidebarItem::compact`, `Dialog::width`, `Dialog::close_label`, `DropdownMenu::size`, `Checkbox::leading` and `Pagination::labels`.
- `Semantic::Scrim` and the `scrim` token, the backdrop behind dialogs, sheets and the command palette.
- `Report::parameter_label`.

### Changed
- `TextField`, `Select`, `Combobox`, `DatePicker`, `Stepper` and `InputOtp` fill with the `bg-300` control token, as the design system specifies.
- Compact text fields are 32 px tall with `radius-md`, the same as small buttons, and the data grid toolbar now has one height throughout.
- The ribbon's application menu is neutral; the accent is kept for the selection.
- Sidebar items truncate long labels instead of wrapping them over their neighbors.
- Dialogs, sheets and the command palette share one backdrop opacity.
- Sample reports and the report wizard use Raven Light colors.
- The report designer is built from RavenRS components and tokens, and follows the Dark, Light and Glass themes; only the paper stays light. The preview and JSON screens drop the ribbon and side panels.

### Fixed
- `Splitter` panes showed nothing: their height was forced to zero.
- The report preview's back button jumped to the first page instead of the previous one.

## [26.2.1] - 2026-11-01

### Fixed
- The documentation of `ravenrs` and `ravenrs-platform-native` builds on docs.rs again. On Linux they depend on `zbus`, which the nightly compiler docs.rs uses cannot compile, so docs.rs now builds them for Windows and macOS. Code and behavior are unchanged, and Linux builds on stable are not affected.

## [26.2.0] - 2026-11-01

The first release under the `YY.R.P` versioning model. It follows 0.1.0 directly; the jump in the number marks the new model, not a long gap.

### Breaking
- Enums that will grow are now `#[non_exhaustive]`: `Event`, `Key`, `PointerButton`, `CursorIcon`, `Brush`, `Primitive`, `Semantic`, `TextStyle`, `Material`, `Axis`, `Align`, `Justify`, `Available`, `DragPhase`, `Placement`, `ButtonStyle`, `ButtonSize`, `Tone`, `SelectionMode`, `Value`, `Summary`, `CellAlign`, `ContentMode`, `RibbonSize`, `Side` and `WebError`. Matches on them need a wildcard arm; future variants will then be additions.

### Added
- **`ravenrs-criteria`**, a criteria and expression language independent of any UI:
  - A public tree that parses from text, can be built in code, and prints back to text that parses to the same tree.
  - Evaluation over any `Record` (JSON, maps, your own rows), with calculated fields, parameters, custom functions and a fixed clock for tests.
  - Over 130 functions and aggregates, in a catalog with signatures and descriptions.
  - `validate` checks fields, parameters and functions against a `Schema`.
  - `to_sql` translates to parameterized SQL for PostgreSQL, MySQL, SQL Server and SQLite.
- **RavenRS Reporting** (`ravenrs-report`, first phase; export comes next):
  - Reports defined as data: JSON layouts (`Report::from_json`) that carry their data sources, or a Rust builder.
  - Bands: report and page headers and footers, nested groups with subtotals, detail, master-detail reports.
  - Controls: label, table, check box, line, shape, panel.
  - Expressions from `ravenrs-criteria`, with aggregates over a group, a page, the report or a nested collection, and `ReportEngine::register_function`.
  - Conditional formatting, named styles, zebra rows, .NET-style formats with pt-BR and en-US cultures.
  - Pagination with growing text, keep-with-next, repeated group headers and page breaks: 10,000 rows in 80 ms natively, about 200 ms in the browser.
  - A wizard that builds a table report from a choice of fields, SVG output per page, and `Document::print_html`, a print-ready page with the fonts embedded.
- **Report Designer** (`examples/report-designer`, [live](https://alexkads.github.io/RavenRS/designer/)): template gallery, wizard, field list with drag and drop, bands, move and resize, property grid, undo and redo, instant preview, printing (the browser's dialog can also save a PDF), JSON view.
- `LayoutStyle::absolute` places a box at an offset outside the flow; `ScrollView::horizontal` scrolls sideways; `TextLayout::lines` reports line ranges and baselines.
- Screen reader support. `Ui::access_tree` describes the UI as roles, names, values and states, and `Ui::perform_action` runs what assistive technology asks for (click, focus, increment, decrement).
  - Desktop: the tree goes to VoiceOver, Narrator, NVDA and Orca through AccessKit. It is built only while a screen reader is listening.
  - Web: the tree is mirrored as invisible ARIA elements over the canvas. Focus is announced through `aria-activedescendant`, toasts through a live region, and screen reader clicks reach the UI. `WebOptions::accessibility(false)` turns it off.
  - Every built-in component describes itself, such as check boxes with their state, sliders with their range, the DataGrid with its rows and keyboard row, and dialogs as modal (the page behind them is hidden).
  - Custom components use `BuildCx::semantics` with `Semantics`.
  - New accessible names: `IconButton::label`, `Button::accessible_name` and `Image::alt`. `label` was added to `Slider`, `Stepper`, `Select`, `Combobox`, `DatePicker`, `RadioGroup` and `SegmentedControl`. `FormField` and `Tooltip` name the control inside them.
- Text selection and the clipboard in `TextField`:
  - Shift with arrows, Home and End selects; Alt or Ctrl moves and deletes by word; Cmd moves and deletes to the line ends.
  - Cmd/Ctrl+A selects all. A press places the caret, a drag selects, Shift-click extends and a double-click selects a word.
  - Cmd/Ctrl+C, X and V copy, cut and paste. Password fields accept a paste but never copy.
  - The clipboard is a runtime service, `BuildCx::clipboard`: the system clipboard on desktop and the browser's on the web.
- DataGrid: Cmd/Ctrl+C copies the selected rows (or the keyboard row) as tab-separated text that pastes into spreadsheets; Cmd/Ctrl+A selects every row.
- `Menubar` follows the WAI-ARIA menubar pattern. Left and Right move between titles, Down, Enter or Space opens a menu, Left and Right inside a menu open the neighbor menu, and Escape returns focus to the title.
- Full keyboard operation following the WAI-ARIA patterns:
  - `RadioGroup`, `SegmentedControl` and `Tabs`: arrow keys move selection and focus together, wrapping; Home and End jump to the ends.
  - `ToggleGroup`: arrow keys move focus; Space toggles.
  - `Stepper`: arrow keys step; Home and End jump to the limits.
  - Menus (`DropdownMenu`, `ContextMenu`, `Menubar`, the `Ribbon` menus) and the `Select` list: the first (or current) item takes focus on open, arrows move, Enter chooses, Escape closes, and focus returns to the trigger. Arrow keys also open a `Select`.
  - `Calendar`: the grid is one Tab stop; arrows move by day and week, Home and End to the ends of the week, Page Up and Page Down by month.
  - `Dialog`, `Sheet` and other full-window layers keep Tab inside them and return focus to where it was when they close.
- `Key::PageUp` and `Key::PageDown`.
- Documentation on every public item, a README per crate, and runnable examples.

### Fixed
- DataGrid: Page Up and Page Down move by 15 rows again; they stopped working when those keys got their own `Key` variants.
- On the web, the canvas now passes keyboard focus to the hidden text input when it is focused with Tab; the listener was never registered.
- DataGrid queries over 100,000 rows now fit in a frame: search 47 → 3.2 ms, range filter 31 → 0.9 ms, text sort 23 → 12 ms, grouping 15 → 11 ms.
- Focus moved by code keeps the focus ring while the user navigates by keyboard.
- `AspectRatio` content with modifiers (for example a styled `Spacer`) no longer collapses to zero height.
- `winit` no longer pulls in `sctk-adwaita`, removing a dependency with a security advisory (RUSTSEC-2026-0192) on Linux.

### Quality
- Accessibility tests for every component family, and an audit that opens every gallery page and fails on any unnamed control.
- GPU golden-image tests (6 scenes in dark and light), property tests, layout and keyboard tests, criterion benchmarks with budgets, and `cargo deny`, MSRV, feature and benchmark checks in `scripts/check.sh`. See [docs/QUALITY.md](docs/QUALITY.md).

## [0.1.0] - 2026-10-31

The first public release.

### Platform
- One declarative UI for native windows (Windows, macOS, Linux via winit) and the browser (WebAssembly + WebGPU on a `<canvas>`).
- GPU renderer on `wgpu` 30: SDF rounded rectangles, borders, gradients, soft shadows, images with mipmaps, glyph atlas text, rectangular clipping.
- Text with Parley and the embedded Inter font; 200 Lucide icons as an embedded icon font.
- Flexbox and grid layout on Taffy.
- Reactive state (`State`, `Binding`, `Derived`) with dependency tracking: only views that read a changed value rebuild, and idle apps draw nothing.
- Runtime services for components: widget-local state, overlays, virtualized scrolling, timers, keyboard shortcuts, drag, double and right click, focus management.
- Web: DPR-aware canvas, pointer, touch scrolling, wheel, keyboard and text input through a hidden textarea (IME commit, dead keys, paste), on-demand frames, device-loss recovery, cleanup.
- Themes: Raven Dark, Raven Light and Raven Glass from semantic design tokens, contrast-checked in tests.

### Components
- Inputs: Button, IconButton, ToggleButton, ToggleGroup, TextField, Select, Combobox, Checkbox, RadioGroup, Toggle, Slider, Stepper, SegmentedControl, InputOtp, Calendar, DatePicker, ThemeSwitcher.
- Forms: FormField with validators, Label.
- Navigation: Ribbon, Menubar, Tabs, Sidebar, Breadcrumb, Pagination, Disclosure, Accordion, CommandPalette.
- Overlays: Dialog, Sheet, Popover, HoverCard, Tooltip, DropdownMenu, ContextMenu, Toaster.
- Data: DataGrid (virtualized, multi-sort, filter row with operators, search, grouping with summaries, column resize and chooser, selection, keyboard navigation, in-place editing, cell templates, CSV export), Table, List, Stat, BarChart, AreaChart.
- Layout: VStack, HStack, Grid, ScrollView, Splitter, Card, GlassCard, Section, AspectRatio, Spacer, `responsive`, and view modifiers.
- Media and feedback: Image, Icon, Carousel, Avatar, Badge, Alert, ProgressBar, Spinner, Skeleton, Kbd, Divider, Canvas.

### Tooling
- `scripts/check.sh` runs formatting, clippy (native and wasm32), the tests and the web builds locally.
- `scripts/deploy-pages.sh` publishes the landing page, the live gallery and the API docs.

[Unreleased]: https://github.com/alexkads/RavenRS/compare/v27.1.10...HEAD
[27.1.10]: https://github.com/alexkads/RavenRS/compare/v27.1.9...v27.1.10
[27.1.3]: https://github.com/alexkads/RavenRS/compare/v27.1.2...v27.1.3
[27.1.2]: https://github.com/alexkads/RavenRS/compare/v27.1.1...v27.1.2
[27.1.1]: https://github.com/alexkads/RavenRS/compare/v27.1.0...v27.1.1
[27.1.0]: https://github.com/alexkads/RavenRS/compare/v26.2.1...v27.1.0
[26.2.1]: https://github.com/alexkads/RavenRS/compare/v26.2.0...v26.2.1
[26.2.0]: https://github.com/alexkads/RavenRS/compare/v0.1.0...v26.2.0
[0.1.0]: https://github.com/alexkads/RavenRS/releases/tag/v0.1.0
