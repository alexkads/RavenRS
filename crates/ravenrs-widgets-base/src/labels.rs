//! The words the built-in components show or read aloud, in one place.

use std::fmt::Write as _;

use ravenrs_app_i18n::{Language, Texts, format_with};

use crate::date::Date;

/// The words the built-in components use. Start from [`Labels::default`]
/// (English) or [`Labels::pt_br`], change what you need, and give it to
/// [`Localized`](crate::Localized).
#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Labels {
    /// Closes a dialog or a sheet.
    pub close: String,
    /// The name of a [`HelpTip`](crate::HelpTip), before what it explains.
    pub help: String,
    /// A sidebar's name.
    pub navigation: String,
    /// A sidebar trigger's name.
    pub toggle_sidebar: String,
    /// Dismisses a toast.
    pub dismiss: String,
    /// A `Chat`'s send button.
    pub send: String,
    /// A `Chat`'s button to hear an answer.
    pub listen: String,
    /// A `TreeView`'s name.
    pub tree_name: String,
    /// Before the tag a `TagBox` button takes off.
    pub tag_remove: String,
    /// A [`ColorPicker`](crate::ColorPicker)'s panel.
    pub pick_color: String,
    /// A [`ColorPicker`](crate::ColorPicker) with no color.
    pub no_color: String,
    /// The shade square of a [`ColorPicker`](crate::ColorPicker).
    pub color_shade: String,
    /// The hue bar of a [`ColorPicker`](crate::ColorPicker).
    pub color_hue: String,
    /// The opacity bar of a [`ColorPicker`](crate::ColorPicker).
    pub color_alpha: String,
    /// The typed color of a [`ColorPicker`](crate::ColorPicker).
    pub color_hex: String,
    /// A [`DateRangePicker`](crate::DateRangePicker) with no period.
    pub pick_date_range: String,
    /// Where a candle of a `Chart` opened.
    pub candle_open: String,
    /// The highest a candle of a `Chart` went.
    pub candle_high: String,
    /// The lowest a candle of a `Chart` went.
    pub candle_low: String,
    /// Where a candle of a `Chart` closed.
    pub candle_close: String,
    /// What a `SankeyChart` says when its flows go in a circle,
    /// before the names of the nodes.
    pub sankey_cycle: String,
    /// A `VirtualList`'s name.
    pub list_name: String,
    /// A `Filmstrip`'s name.
    pub filmstrip_name: String,
    /// A `PagedDocumentView`'s name.
    pub document_name: String,
    /// A page of a `PagedDocumentView`, before its number.
    pub page_word: String,
    /// Between a page's number and the total, such as `" of "`.
    pub page_of: String,
    /// Goes to the previous page of a `PagedDocumentView`.
    pub previous_page: String,
    /// Goes to the next page of a `PagedDocumentView`.
    pub next_page: String,
    /// A `FileBrowser`'s name.
    pub file_browser_name: String,
    /// The column of names of a `FileBrowser`.
    pub file_name: String,
    /// The column of sizes of a `FileBrowser`.
    pub file_size: String,
    /// An `AttachmentStrip`'s name.
    pub attachments: String,
    /// Takes a file out of an `AttachmentStrip`, before its name.
    pub attachment_remove: String,
    /// How a file will be sent, with `{what}` for the way chosen.
    pub attachment_send_as: String,
    /// A `SelectionBar`'s name.
    pub selection_bar: String,
    /// Leaves a selection.
    pub selection_exit: String,
    /// What a `SelectionBar` says with nothing chosen.
    pub selection_none: String,
    /// What a `SelectionBar` says with one thing chosen.
    pub selection_one: String,
    /// What a `SelectionBar` says with more, with `{count}`.
    pub selection_many: String,
    /// The name of an `AudioPlayerButton`'s progress.
    pub audio_player: String,
    /// Plays a recording.
    pub audio_play: String,
    /// Stops a recording.
    pub audio_stop: String,
    /// A recording is on its way.
    pub audio_loading: String,
    /// Shows or hides what was said in a recording.
    pub audio_transcript: String,
    /// Asks for what was said in a recording.
    pub audio_transcribe: String,
    /// What was said in a recording is on its way.
    pub audio_transcribing: String,
    /// A recording with no one speaking.
    pub audio_no_speech: String,
    /// A photo in a `Chat` with no description.
    pub chat_photo: String,
    /// A photo in a `Chat` is on its way.
    pub chat_photo_loading: String,
    /// Opens a file in a `Chat`, with `{what}` for what it is.
    pub chat_open: String,
    /// The column of dates of a `FileBrowser`.
    pub file_modified: String,
    /// What a `FileBrowser` says of a folder.
    pub folder_word: String,
    /// What a `FileBrowser` shows in a folder with nothing in it.
    pub folder_empty: String,
    /// A `YearView`'s button to the year before.
    pub previous_year: String,
    /// A `YearView`'s button to the year after.
    pub next_year: String,
    /// What a `YearView` says of a marked day without words of its own.
    pub day_marked: String,
    /// A `SortableList`'s name.
    pub sortable_name: String,
    /// How to move a row of a `SortableList` from the keyboard.
    pub sortable_hint: String,
    /// What is announced after a reorder. `{position}` and `{total}` are filled in.
    pub moved_to: String,
    /// A `DocumentTabs`' name.
    pub document_tabs_name: String,
    /// A `DockArea`'s name.
    pub dock_area_name: String,
    /// The name of the tabs of a group of a `DockArea`.
    pub dock_tabs_name: String,
    /// What a `DockArea` says with nothing in its center.
    pub dock_empty: String,
    /// The left dock of a `DockArea`, in a menu.
    pub dock_left: String,
    /// The right dock of a `DockArea`, in a menu.
    pub dock_right: String,
    /// The bottom dock of a `DockArea`, in a menu.
    pub dock_bottom: String,
    /// The top dock of a `DockArea`, in a menu.
    pub dock_top: String,
    /// The button that hides a panel of a `DockArea` at the edge of its dock, with `{title}`.
    pub dock_auto_hide: String,
    /// The menu item that hides a panel of a `DockArea` at the edge of its dock.
    pub dock_auto_hide_item: String,
    /// The button that puts a hidden panel of a `DockArea` back in its dock, with `{title}`.
    pub dock_pin: String,
    /// A hidden panel of a `DockArea` on its rail, and what is said as it hides, with `{title}`
    /// and `{place}`.
    pub dock_hidden: String,
    /// What is said as a hidden panel of a `DockArea` goes back in its dock, with `{title}`.
    pub dock_pinned: String,
    /// The menu item that lifts a panel of a `DockArea` out to float over the area.
    pub dock_float_item: String,
    /// What is said as a panel of a `DockArea` floats, and what its box is called, with `{title}`.
    pub dock_floating: String,
    /// The button that puts a floating panel of a `DockArea` back, with `{title}`.
    pub dock_dock_back: String,
    /// The header of a floating panel of a `DockArea`, which moves it, with `{title}`.
    pub dock_move_floating: String,
    /// An edge or a corner that resizes a floating panel of a `DockArea`, with `{title}`.
    pub dock_resize_floating: String,
    /// Why a document of a `DockArea` cannot go to a dock.
    pub dock_documents_center: String,
    /// Why a tool of a `DockArea` cannot go to the center.
    pub dock_tools_docks: String,
    /// The menu item that closes the other documents of a group of a `DockArea`.
    pub dock_close_others: String,
    /// The list Ctrl+Tab shows over a `DockArea`, to switch panels.
    pub dock_switcher: String,
    /// The column of documents in that list.
    pub dock_documents: String,
    /// The column of tools in that list.
    pub dock_tools: String,
    /// Where a floating panel is, in that list.
    pub dock_where_floating: String,
    /// Where a hidden panel is, in that list, with `{place}`.
    pub dock_where_hidden: String,
    /// The center of a `DockArea`, in a menu.
    pub dock_center: String,
    /// The item of a `DockArea`'s menu that opens where a panel can go.
    pub dock_move_to: String,
    /// Joins a group of a `DockArea`, with `{title}` for what shows in it.
    pub dock_with: String,
    /// Opens a group to the right of a panel's own in a `DockArea`.
    pub dock_split_right: String,
    /// Opens a group below a panel's own in a `DockArea`.
    pub dock_split_below: String,
    /// Closes a panel of a `DockArea`, in a menu.
    pub dock_close: String,
    /// The item of a `DockArea`'s menu that lists closed panels.
    pub dock_reopen: String,
    /// The button that closes a panel, with `{title}`.
    pub dock_close_panel: String,
    /// The button with a panel's menu, with `{title}`.
    pub dock_options: String,
    /// Collapses a dock, with `{place}`.
    pub dock_hide: String,
    /// Brings a dock back, with `{place}`.
    pub dock_show: String,
    /// The bar that resizes a dock, with `{place}`.
    pub dock_resize: String,
    /// The bar between two parts of a split, with `{first}` and `{second}`.
    pub dock_resize_pair: String,
    /// What is announced after a panel moved, with `{place}`.
    pub dock_moved: String,
    /// What is announced after a panel closed, with `{title}`.
    pub dock_closed: String,
    /// Closes a tab of a `DocumentTabs`.
    pub close_tab: String,
    /// Opens a new tab of a `DocumentTabs`.
    pub new_tab: String,
    /// Closes the other tabs of a `DocumentTabs`.
    pub close_other_tabs: String,
    /// Closes the tabs to the right in a `DocumentTabs`.
    pub close_tabs_to_right: String,
    /// What a `DocumentTabs` says of a document with unsaved changes.
    pub unsaved: String,
    /// Goes to the previous step of a `Wizard`.
    pub wizard_back: String,
    /// Goes to the next step of a `Wizard`.
    pub wizard_next: String,
    /// Ends a `Wizard`.
    pub wizard_finish: String,
    /// Leaves a `Wizard`.
    pub wizard_cancel: String,
    /// A step of a `Wizard` already done.
    pub wizard_done: String,
    /// A step of a `Wizard` or a `StepIndicator` with a problem.
    pub wizard_error: String,
    /// The bar between two panes, for screen readers.
    pub split_resize: String,
    /// What dragging a `FloatingPanel` by its header does.
    pub panel_move: String,
    /// Reduces a `FloatingPanel` to its header.
    pub panel_minimize: String,
    /// Brings back a reduced `FloatingPanel`.
    pub panel_restore: String,
    /// The arrow of a [`ScrollEdgeArrow`](crate::ScrollEdgeArrow) toward the start.
    pub scroll_back: String,
    /// The arrow of a [`ScrollEdgeArrow`](crate::ScrollEdgeArrow) toward the end.
    pub scroll_forward: String,
    /// The minimize button of a [`TitleBar`](crate::TitleBar).
    pub window_minimize: String,
    /// The maximize button of a [`TitleBar`](crate::TitleBar).
    pub window_maximize: String,
    /// The same button of a [`TitleBar`](crate::TitleBar) when the window is maximized.
    pub window_restore: String,
    /// Names a `ValueSlider` that was not given a label.
    pub value_adjust: String,
    /// Names the field of an `InlineRename` that was not given a label.
    pub rename: String,
    /// How to start an `InlineRename`, for screen readers.
    pub rename_hint: String,
    /// What a `FileUpload` asks for.
    pub file_drop: String,
    /// Opens the chooser of a `FileUpload`.
    pub file_choose: String,
    /// Takes a file off.
    pub file_remove: String,
    /// Why a file was refused.
    pub file_wrong_kind: String,
    /// Why a file was refused.
    pub file_too_large: String,
    /// Why a file was refused.
    pub file_too_many: String,
    /// Opens the chooser of an `ImageField`.
    pub image_choose: String,
    /// Opens the chooser of an `ImageField` that has a picture.
    pub image_replace: String,
    /// An `ImageField` with no picture.
    pub image_none: String,
    /// A file an `ImageField` cannot show.
    pub image_invalid: String,
    /// The handle that moves a tile of a `TileLayout`.
    pub tile_move: String,
    /// The handle that resizes a tile of a `TileLayout`.
    pub tile_resize: String,
    /// A `CardView`'s name.
    pub cards_name: String,
    /// Opens the details of a row of a `DataGrid`.
    pub grid_expand_row: String,
    /// Closes the details of a row of a `DataGrid`.
    pub grid_collapse_row: String,
    /// What an option of a `TagBox` says when it adds what was typed, where `{0}`
    /// is what was typed. [`Labels::tag_add`] fills it in.
    pub tag_add: String,
    /// Where a `Wizard` stands, where `{0}` is its step and `{1}` its count.
    /// [`Labels::wizard_progress`] fills it in.
    pub wizard_progress: String,
    /// The language and region of numbers and dates in files, such as `pt-BR`.
    pub culture: String,
    /// A spinner's name.
    pub loading: String,
    /// A stepper's minus button.
    pub decrease: String,
    /// A stepper's plus button.
    pub increase: String,
    /// The error of an empty [`FormField`](crate::FormField) marked as required.
    pub required: String,
    /// A date picker with no date.
    pub pick_date: String,
    /// A combobox with no choice.
    pub select: String,
    /// A combobox's search field.
    pub search: String,
    /// A combobox's search that found nothing.
    pub no_results: String,
    /// A command palette's field.
    pub search_commands: String,
    /// A command palette's search that found nothing.
    pub no_commands: String,
    /// A command palette's name.
    pub command_palette: String,
    /// A theme switcher's name.
    pub theme: String,
    /// The dark theme.
    pub theme_dark: String,
    /// The light theme.
    pub theme_light: String,
    /// The glass theme.
    pub theme_glass: String,
    /// Following the system between light and dark.
    pub theme_system: String,
    /// A theme editor's group: backgrounds and borders.
    pub theme_surfaces: String,
    /// A theme editor's group: the colors of text.
    pub theme_text: String,
    /// A theme editor's group: the accent.
    pub theme_accent: String,
    /// A theme editor's group: colors that say how something went.
    pub theme_status: String,
    /// A theme editor's group: the series of a chart after the first.
    pub theme_charts: String,
    /// A theme editor's group: corners.
    pub theme_shape: String,
    /// A theme editor's group: the sizes of text.
    pub theme_sizes: String,
    /// Goes back to the theme as it came.
    pub theme_restore: String,
    /// Copies what changed in a theme.
    pub theme_copy: String,
    /// Takes tokens that were copied.
    pub theme_paste: String,
    /// What was pasted is not tokens of a theme.
    pub theme_invalid: String,
    /// How many tokens changed: none, one, and more, where `{0}` is how many.
    /// [`Labels::theme_changed`] picks and fills it in.
    pub theme_changed: [String; 3],
    /// A contrast, where `{0}` is the ratio and `{1}` the token it is measured over.
    /// [`Labels::theme_contrast`] fills it in.
    pub theme_contrast: String,
    /// A code block's copy button.
    pub copy: String,
    /// A code block's copy button, right after copying.
    pub copied: String,
    /// The list of versions in a `ReleaseNotes`.
    pub release_versions: String,
    /// Goes to the next newer version in a `ReleaseNotes`.
    pub release_newer: String,
    /// Goes to the next older version in a `ReleaseNotes`.
    pub release_older: String,
    /// Between the position and the total, as in "2 of 5".
    pub release_of: String,
    /// The diagnostics panel's title.
    pub diagnostics: String,
    /// The button that starts a performance test.
    pub diagnostics_test: String,
    /// A performance test is running.
    pub diagnostics_testing: String,
    /// What to do while a performance test runs.
    pub diagnostics_testing_detail: String,
    /// A performance test found the app fluid.
    pub diagnostics_test_good: String,
    /// A performance test found visible stutter.
    pub diagnostics_test_fair: String,
    /// A performance test found the app slow.
    pub diagnostics_test_poor: String,
    /// After the rate a performance test measured.
    pub diagnostics_test_fps: String,
    /// A performance test found nothing to blame.
    pub diagnostics_test_cause_none: String,
    /// Diagnostics panel: title of the picture of the latest frames.
    pub diagnostics_timeline: String,
    /// Diagnostics panel: under it, what the colors and the line mean.
    pub diagnostics_timeline_detail: String,
    /// Diagnostics panel: why the views ran again.
    pub diagnostics_why: String,
    /// Diagnostics panel: nothing ran again.
    pub diagnostics_why_none: String,
    /// Diagnostics panel: the names of the causes, in the order of `BuildCause::ALL`.
    pub diagnostics_causes: [String; 10],
    /// Diagnostics panel: the views that cost the most.
    pub diagnostics_views: String,
    /// Diagnostics panel: what splits the draw calls.
    pub diagnostics_batches: String,
    /// Diagnostics panel: the GPU's resources and memory.
    pub diagnostics_memory: String,
    /// Diagnostics panel: the button that copies a capture of the frames.
    pub diagnostics_copy_trace: String,
    /// A performance test blames drawing.
    pub diagnostics_test_cause_gpu: String,
    /// A performance test blames building the screen.
    pub diagnostics_test_cause_interface: String,
    /// Diagnostics: frames are drawn on the CPU.
    pub diagnostics_software: String,
    /// What drawing on the CPU means, and what to do about it.
    pub diagnostics_software_detail: String,
    /// Diagnostics: the app was built without optimizations.
    pub diagnostics_unoptimized: String,
    /// What a build without optimizations means, and what to do about it.
    pub diagnostics_unoptimized_detail: String,
    /// Diagnostics: GPUs left out because their drivers are not conformant.
    pub diagnostics_hidden: String,
    /// Diagnostics: no GPU could be used.
    pub diagnostics_no_gpu: String,
    /// Diagnostics: the time of a whole frame.
    pub diagnostics_frame: String,
    /// Diagnostics: the time spent rebuilding and laying out the UI.
    pub diagnostics_update: String,
    /// Diagnostics: the time spent waiting for a texture to draw on.
    pub diagnostics_acquire: String,
    /// Diagnostics: the time spent recording and submitting GPU work.
    pub diagnostics_render: String,
    /// Diagnostics: the button that opens a new bug report in the browser.
    pub diagnostics_report_bug: String,
    /// Diagnostics: the title of what a bug report would share.
    pub diagnostics_report_shares: String,
    /// Diagnostics: what a bug report would share and that nothing is sent yet.
    pub diagnostics_report_shares_detail: String,
    /// Diagnostics: the browser could not be opened for a bug report.
    pub diagnostics_report_failed: String,
    /// What to do about the browser not opening.
    pub diagnostics_report_failed_detail: String,
    /// The start of a new bug report: the questions for the person to answer.
    pub diagnostics_report_questions: String,
    /// After a report cut short to fit in a URL.
    pub diagnostics_report_truncated: String,
    /// A calendar's back button.
    pub previous_month: String,
    /// A calendar's forward button.
    pub next_month: String,
    /// Marks today in a calendar.
    pub today: String,
    /// Sunday to Saturday, short, as a calendar's columns.
    pub weekdays: [String; 7],
    /// January to December.
    pub months: [String; 12],
    /// Between a month and its year, such as `" "` or `" de "`.
    pub month_year: String,
    /// How people here write a date, for a date picker, where `{0}` is the day and `{1}` the month
    /// with two digits, `{2}` the year, `{3}` the day as it is and `{4}` the month in three letters.
    /// [`Labels::short_date`] fills it in.
    pub short_date: String,
    /// Above the errors that keep a form from being saved.
    pub form_errors: String,
    /// A `LookupField`'s button that empties it.
    pub clear: String,
    /// A `FilterBuilder`'s word: all of these.
    pub filter_all: String,
    /// A `FilterBuilder`'s word: any of these.
    pub filter_any: String,
    /// A `FilterBuilder`'s word: how lines combine.
    pub filter_match: String,
    /// A `FilterBuilder`'s word: condition.
    pub filter_add_condition: String,
    /// A `FilterBuilder`'s word: group.
    pub filter_add_group: String,
    /// A `FilterBuilder`'s word: remove.
    pub filter_remove: String,
    /// A `FilterBuilder`'s word: field.
    pub filter_field: String,
    /// A `FilterBuilder`'s word: comparison.
    pub filter_comparison: String,
    /// A `FilterBuilder`'s word: value.
    pub filter_value: String,
    /// A `FilterBuilder`'s word: expression.
    pub filter_expression: String,
    /// A `FilterBuilder`'s word: yes.
    pub filter_yes: String,
    /// A `FilterBuilder`'s word: no.
    pub filter_no: String,
    /// A `FilterBuilder`'s word: is.
    pub filter_equal: String,
    /// A `FilterBuilder`'s word: is not.
    pub filter_not_equal: String,
    /// A `FilterBuilder`'s word: greater than.
    pub filter_greater: String,
    /// A `FilterBuilder`'s word: at least.
    pub filter_greater_or_equal: String,
    /// A `FilterBuilder`'s word: less than.
    pub filter_less: String,
    /// A `FilterBuilder`'s word: at most.
    pub filter_less_or_equal: String,
    /// A `FilterBuilder`'s word: contains.
    pub filter_contains: String,
    /// A `FilterBuilder`'s word: starts with.
    pub filter_starts_with: String,
    /// A `FilterBuilder`'s word: is empty.
    pub filter_is_empty: String,
    /// A `FilterBuilder`'s word: is not empty.
    pub filter_is_not_empty: String,
    /// A `FilterBuilder`'s word: is any of.
    pub filter_any_of: String,
    /// A `FilterBuilder`'s word: is none of.
    pub filter_none_of: String,
    /// A `Ribbon`'s name.
    pub ribbon: String,
    /// A `Ribbon`'s search box.
    pub ribbon_search: String,
    /// Keeps a `Ribbon` open.
    pub ribbon_pin: String,
    /// Folds a `Ribbon`.
    pub ribbon_collapse: String,
    /// A [`Carousel`](crate::Carousel)'s button back.
    pub carousel_previous: String,
    /// A [`Carousel`](crate::Carousel)'s button forward.
    pub carousel_next: String,
    /// A [`Carousel`](crate::Carousel)'s dot, before its number.
    pub carousel_slide: String,
    /// An [`InputOtp`](crate::InputOtp)'s name.
    pub verification_code: String,
    /// A question's button that gives up on it.
    pub cancel: String,
    /// Before the word a `ConfirmDialog` asks to be typed.
    pub confirm_by_typing: String,
    /// The `Playground` console, when the code prints the same lines on every build.
    pub printed_every_build: String,
    /// The menu of commands that did not fit.
    pub more_commands: String,
    /// A number's decimal separator.
    pub decimal_separator: char,
    /// A number's thousands separator.
    pub group_separator: char,
    /// A `TimePicker`'s hint and button.
    pub pick_time: String,
    /// A number field's button that adds a step.
    pub step_up: String,
    /// A number field's button that takes a step away.
    pub step_down: String,
    /// After the count of events an `EventCalendar` day has no room for.
    pub calendar_more: String,
    /// Month, week and day, as an `EventCalendar` offers its views.
    pub calendar_views: [String; 3],
    /// The name of an `EventCalendar`'s choice of view.
    pub calendar_view_name: String,
    /// An `EventCalendar`'s back button, by week or by day.
    pub calendar_previous: String,
    /// An `EventCalendar`'s forward button, by week or by day.
    pub calendar_next: String,
    /// Over the events of a whole day, above the hours of a week or a day.
    pub calendar_all_day: String,
    /// What an empty hour of an `EventCalendar` creates, before its time.
    pub calendar_new_event: String,
    /// The header over a `Scheduler`'s resources.
    pub scheduler_resources: String,
    /// Zooms a scheduler's time axis in.
    pub zoom_in: String,
    /// Zooms a scheduler's time axis out.
    pub zoom_out: String,
    /// A `Histogram`'s name.
    pub histogram: String,
    /// What a `Histogram` says when a channel piles up at its low end.
    pub histogram_clipped_low: String,
    /// What a `Histogram` says when a channel piles up at its high end.
    pub histogram_clipped_high: String,
    /// A `Ruler`'s name.
    pub ruler: String,
    /// A `ColorWheel`'s name.
    pub color_wheel: String,
    /// The distance from gray on a `ColorWheel`.
    pub color_saturation: String,
    /// A `CurveEditor`'s name.
    pub curve_editor: String,
    /// One point of a `CurveEditor`, before its position in the list.
    pub curve_point: String,
    /// After the number of points of a `CurveEditor`.
    pub curve_points: String,
    /// The horizontal value of a point of a `CurveEditor`.
    pub curve_input: String,
    /// The vertical value of a point of a `CurveEditor`.
    pub curve_output: String,
    /// A [`ZoomControl`](crate::ZoomControl)'s name.
    pub zoom: String,
    /// Shows the whole content, in a [`ZoomControl`](crate::ZoomControl).
    pub zoom_fit: String,
    /// What the 1:1 button of a [`ZoomControl`](crate::ZoomControl) is called by screen readers.
    pub zoom_actual: String,
    /// A `ZoomPanView`'s name.
    pub zoom_view: String,
    /// A `Minimap`'s name.
    pub minimap: String,
    /// A `FlowCanvas`'s name.
    pub flow_canvas: String,
    /// What a `FlowCanvas` holds, with `{0}` nodes and `{1}` connections.
    pub flow_count: String,
    /// The zoom buttons of a `FlowCanvas`, as a group.
    pub flow_controls: String,
    /// Locks the editing of a `FlowCanvas`.
    pub flow_lock: String,
    /// Unlocks the editing of a `FlowCanvas`.
    pub flow_unlock: String,
    /// Why two ports of a `FlowCanvas` do not connect, when the app says nothing.
    pub flow_refused: String,
    /// A node of a `FlowCanvas` that is switched off.
    pub flow_disabled: String,
    /// Lays a `FlowCanvas` out by itself, in layers.
    pub flow_arrange: String,
    /// A group of a `FlowCanvas`, and what a new one is called.
    pub flow_group: String,
    /// How many nodes a group of a `FlowCanvas` holds, with `{0}` the number.
    pub flow_members: String,
    /// The field where a node's title of a `FlowCanvas` is typed.
    pub flow_title: String,
    /// A turned node of a `FlowCanvas`, with `{0}` its turn in degrees.
    pub flow_turned: String,
    /// A run waits for one or more ways out of `{0}`.
    pub flow_run_choose_many: String,
    /// Follows the ways out ticked in a `FlowCanvas` run.
    pub flow_run_follow: String,
    /// A `FlowCanvas` run asked to follow no way out.
    pub flow_run_none_chosen: String,
    /// The heading of a `FlowLayers`.
    pub flow_layers: String,
    /// Hides a node of a `FlowLayers`, with `{0}` its name.
    pub flow_layer_hide: String,
    /// Shows a hidden node of a `FlowLayers`, with `{0}` its name.
    pub flow_layer_show: String,
    /// Brings a node of a `FlowLayers` forward, with `{0}` its name.
    pub flow_layer_up: String,
    /// Sends a node of a `FlowLayers` back, with `{0}` its name.
    pub flow_layer_down: String,
    /// Puts a breakpoint on the nodes selected in a `FlowCanvas`, or takes it off (F9).
    pub flow_run_breakpoint: String,
    /// The Debug button and box of a `FlowCanvas`.
    pub flow_debug: String,
    /// Starts a `FlowCanvas` run over.
    pub flow_run_start: String,
    /// Moves a `FlowCanvas` run one step.
    pub flow_run_step: String,
    /// Moves a `FlowCanvas` run on to the next stop.
    pub flow_run_continue: String,
    /// Ends a `FlowCanvas` run.
    pub flow_run_stop: String,
    /// What the debug box of a `FlowCanvas` says before a run.
    pub flow_run_hint: String,
    /// A token went from `{0}` to `{1}`.
    pub flow_run_moved: String,
    /// A token reached the end `{0}`.
    pub flow_run_ended: String,
    /// A run waits for a way out of `{0}`.
    pub flow_run_choose: String,
    /// A run in which every token reached an end.
    pub flow_run_done: String,
    /// A run stopped at `{0}`, which leads nowhere.
    pub flow_run_dead_end: String,
    /// A run stuck at the join `{0}`.
    pub flow_run_stuck: String,
    /// A run that goes round without ending.
    pub flow_run_looping: String,
    /// A run with nowhere to start from.
    pub flow_run_no_start: String,
    /// The Check button of a `FlowCanvas`.
    pub flow_check: String,
    /// A `FlowCanvas` check that found nothing wrong.
    pub flow_check_ok: String,
    /// A `FlowCanvas` check that found problems, with `{0}` how many.
    pub flow_check_found: String,
    /// Closes the box of a `FlowCanvas` check.
    pub flow_check_close: String,
    /// A field of a `FlowInspector`: the color inside a node.
    pub flow_field_fill: String,
    /// A field of a `FlowInspector`: the color of an outline or a line.
    pub flow_field_stroke: String,
    /// A field of a `FlowInspector`: the color of a node's words.
    pub flow_field_text: String,
    /// A field of a `FlowInspector`: an outline's width.
    pub flow_field_stroke_width: String,
    /// A field of a `FlowInspector`: a line drawn in dashes.
    pub flow_field_dashed: String,
    /// The heading of a `FlowLibrary`.
    pub flow_library: String,
    /// Starts a new diagram in a `FlowLibrary`.
    pub flow_library_new: String,
    /// Keeps the open diagram of a `FlowLibrary`.
    pub flow_library_save: String,
    /// Opens a diagram from a JSON file in a `FlowLibrary`.
    pub flow_library_open_file: String,
    /// Writes the open diagram of a `FlowLibrary` to a JSON file.
    pub flow_library_save_file: String,
    /// Deletes a diagram of a `FlowLibrary`.
    pub flow_library_delete: String,
    /// Asks before a `FlowLibrary` deletes a diagram, with `{0}` its name.
    pub flow_library_delete_title: String,
    /// Says what deleting a diagram of a `FlowLibrary` means.
    pub flow_library_delete_message: String,
    /// The name field of a `FlowLibrary` while empty.
    pub flow_library_name: String,
    /// A diagram of a `FlowLibrary` not named yet.
    pub flow_library_untitled: String,
    /// A `FlowLibrary` diagram with changes not kept.
    pub flow_library_unsaved: String,
    /// A `FlowLibrary` diagram with every change kept.
    pub flow_library_saved: String,
    /// How many nodes a `FlowLibrary` diagram has, with `{0}` the number.
    pub flow_library_count: String,
    /// A `FlowCanvas` node running now, for screen readers.
    pub flow_running: String,
    /// A `FlowCanvas` node that ran and went well, for screen readers.
    pub flow_done: String,
    /// A `FlowCanvas` node that did not run in a run told from outside, for screen readers.
    pub flow_skipped: String,
    /// A `FlowCanvas` node stopped before it ended, for screen readers.
    pub flow_cancelled: String,
    /// A `FlowCanvas` node that gave the items fixed on it instead of running.
    pub flow_pinned: String,
    /// One item went along a `FlowCanvas` connection.
    pub flow_item_one: String,
    /// How many items went along a `FlowCanvas` connection, with `{count}`.
    pub flow_items: String,
    /// The search a `FlowCanvas` connection let go in the open leads to.
    pub flow_open_search: String,
    /// That search with no model the port accepts.
    pub flow_open_none: String,
    /// Closes that search without creating anything.
    pub flow_open_close: String,
    /// A `ParameterForm` field holding a value as it is.
    pub param_fixed: String,
    /// A `ParameterForm` field holding an expression.
    pub param_expression: String,
    /// What screen readers call the switch between value and expression, after the field's name.
    pub param_mode: String,
    /// The `ParameterForm` button that gives the values back.
    pub param_apply: String,
    /// The `ParameterForm` button that forgets what was edited.
    pub param_discard: String,
    /// Adds a row to a list of a `ParameterForm`.
    pub param_add: String,
    /// Removes a row of a list of a `ParameterForm`, with `{0}` its number.
    pub param_remove: String,
    /// While a `ParameterForm` field's options load.
    pub param_loading: String,
    /// The title of a `ParameterForm` field whose options failed to load.
    pub param_load_failed: String,
    /// Asks a `ParameterForm` field's options again.
    pub param_retry: String,
    /// A required `ParameterForm` field with no value, with `{0}` its name.
    pub param_required: String,
    /// A choice of a `ParameterForm` with nothing chosen.
    pub param_none: String,
    /// What screen readers call the switch between the ways a `DataInspector` shows its items.
    pub inspector_mode: String,
    /// The table of a `DataInspector`.
    pub inspector_table: String,
    /// The tree of a `DataInspector`.
    pub inspector_tree: String,
    /// The JSON of a `DataInspector`.
    pub inspector_json: String,
    /// The schema of a `DataInspector`.
    pub inspector_schema: String,
    /// A field a `DataInspector` item does not have, unlike one that is null.
    pub inspector_absent: String,
    /// A yes-or-no value in a `DataInspector` schema.
    pub inspector_boolean: String,
    /// A number in a `DataInspector` schema.
    pub inspector_number: String,
    /// A text in a `DataInspector` schema.
    pub inspector_text: String,
    /// A list in a `DataInspector` schema.
    pub inspector_list: String,
    /// An object in a `DataInspector` schema.
    pub inspector_object: String,
    /// Entries of a list a `DataInspector` left out, with `{0}`.
    pub inspector_more: String,
    /// Items a `DataInspector` shows of all, with `{0}` and `{1}`.
    pub inspector_sampled: String,
    /// A `DataInspector` with no items.
    pub inspector_empty: String,
    /// Goes to the item before in a `DataInspector`.
    pub inspector_previous: String,
    /// Goes to the next item in a `DataInspector`.
    pub inspector_next: String,
    /// The item a `DataInspector` shows, with `{0}` and `{1}`.
    pub inspector_item: String,
    /// The column of paths of a `DataInspector` schema.
    pub inspector_path: String,
    /// The column of kinds of a `DataInspector` schema.
    pub inspector_kind: String,
    /// The column of how many items have a field, in a `DataInspector` schema.
    pub inspector_in: String,
    /// How many items have a field, with `{0}` and `{1}`.
    pub inspector_present: String,
    /// The path of the field chosen in a `DataInspector`, with `{0}`.
    pub inspector_chosen: String,
    /// What an `ExpressionEditor` expression works out to over a sample, with `{0}`.
    pub expression_result: String,
    /// Why an `ExpressionEditor` expression could not be worked out, with `{0}`.
    pub expression_failed: String,
    /// The tag of the field chosen in a `DataInspector`, to carry to an expression, with `{0}`.
    pub inspector_drag: String,
    /// Copies the field chosen in a `DataInspector` as an expression.
    pub inspector_copy: String,
    /// A field of a `ParameterForm` that takes no field carried to it.
    pub param_no_field: String,
    /// The Export menu of a `FlowCanvas`.
    pub flow_export: String,
    /// Export a `FlowCanvas` as a PNG picture.
    pub flow_export_png: String,
    /// Export a `FlowCanvas` as an SVG drawing.
    pub flow_export_svg: String,
    /// Export a `FlowCanvas` as a PDF page.
    pub flow_export_pdf: String,
    /// Export only what is selected in a `FlowCanvas`.
    pub flow_export_selection: String,
    /// A bend point of a `FlowCanvas` line, with `{0}` its place, `{1}` how many and `{2}` the
    /// line's label (or id).
    pub flow_bend: String,
    /// Why a group of a `FlowCanvas` does not turn.
    pub flow_no_turn: String,
    /// The name of a `FlowInspector`.
    pub flow_inspector: String,
    /// What a `FlowInspector` says with nothing selected.
    pub flow_inspector_empty: String,
    /// The heading of a `FlowInspector` for one node.
    pub flow_inspector_node: String,
    /// The heading of a `FlowInspector` for one connection.
    pub flow_inspector_edge: String,
    /// The heading of a `FlowInspector` for several nodes, with `{0}` how many.
    pub flow_inspector_nodes: String,
    /// The heading of a `FlowInspector` with several connections selected, with `{0}` how many.
    pub flow_inspector_edges: String,
    /// A value of a `FlowInspector` that differs among the nodes selected.
    pub flow_inspector_mixed: String,
    /// What a read-only `FlowInspector` says.
    pub flow_inspector_read_only: String,
    /// The heading of a `FlowToolbox`.
    pub flow_toolbox: String,
    /// What the search field of a `FlowToolbox` says while empty.
    pub flow_toolbox_search: String,
    /// What a `FlowToolbox` says when no model matches the search.
    pub flow_toolbox_none: String,
    /// What a model of a `FlowToolbox` says how to use it, to screen readers.
    pub flow_toolbox_hint: String,
    /// A field of a `FlowInspector`: a node's title.
    pub flow_field_title: String,
    /// A field of a `FlowInspector`: the line under a node's title.
    pub flow_field_line: String,
    /// A field of a `FlowInspector`: a node's kind.
    pub flow_field_kind: String,
    /// A field of a `FlowInspector`: a node's or a connection's shape.
    pub flow_field_shape: String,
    /// A field of a `FlowInspector`.
    pub flow_field_width: String,
    /// A field of a `FlowInspector`.
    pub flow_field_height: String,
    /// A field of a `FlowInspector`: the left edge.
    pub flow_field_x: String,
    /// A field of a `FlowInspector`: the top edge.
    pub flow_field_y: String,
    /// A field of a `FlowInspector`: how a node is doing.
    pub flow_field_status: String,
    /// A field of a `FlowInspector`: a connection's label.
    pub flow_field_label: String,
    /// A field of a `FlowInspector`: the cap at a connection's start.
    pub flow_field_start: String,
    /// A field of a `FlowInspector`: the cap at a connection's end.
    pub flow_field_end: String,
    /// A field of a `FlowInspector`: a connection that shows something travelling.
    pub flow_field_animated: String,
    /// The statuses of a node in a `FlowInspector`.
    pub flow_status_names: [String; 4],
    /// The shapes of a node in a `FlowInspector`, in the order of `FlowShape`.
    pub flow_shape_names: [String; 10],
    /// The shapes of a connection in a `FlowInspector`.
    pub flow_line_names: [String; 3],
    /// The ends of a connection in a `FlowInspector` and a `LineStylePicker`, in the order of
    /// `EdgeCap::ALL`.
    pub flow_cap_names: [String; 13],
    /// The patterns of a line in a `LineStylePicker`, in the order of `LineDash::ALL`.
    pub line_dash_names: [String; 5],
    /// A `LineStylePicker`'s name.
    pub line_style: String,
    /// A `LineStylePicker`'s field: how wide the line is.
    pub line_style_width: String,
    /// A `LineStylePicker`'s field: the pattern the line is drawn in.
    pub line_style_dash: String,
    /// A `LineStylePicker`'s field: the cap at the line's start.
    pub line_style_start: String,
    /// A `LineStylePicker`'s field: the cap at the line's end.
    pub line_style_end: String,
    /// What a `LineStylePicker` shows for a value the lines it edits do not share.
    pub line_style_mixed: String,
    /// The items of a `FlowCanvas`'s context menu.
    pub flow_menu_cut: String,
    /// See `flow_menu_cut`.
    pub flow_menu_copy: String,
    /// See `flow_menu_cut`.
    pub flow_menu_paste: String,
    /// See `flow_menu_cut`.
    pub flow_menu_duplicate: String,
    /// See `flow_menu_cut`.
    pub flow_menu_delete: String,
    /// See `flow_menu_cut`.
    pub flow_menu_front: String,
    /// See `flow_menu_cut`.
    pub flow_menu_back: String,
    /// See `flow_menu_cut`.
    pub flow_menu_group: String,
    /// See `flow_menu_cut`.
    pub flow_menu_ungroup: String,
    /// See `flow_menu_cut`.
    pub flow_menu_select_all: String,
    /// A handle of a `FlowCanvas` node with no ports that starts a connection, with `{0}` its
    /// node's title.
    pub flow_connect_from: String,
    /// The field a connection's label is typed in on a `FlowCanvas`, with `{0}` the connection.
    pub flow_edge_label: String,
    /// A port of a `FlowCanvas` where connections end, with `{0}` its node's title.
    pub flow_input: String,
    /// A port of a `FlowCanvas` where connections start, with `{0}` its node's title.
    pub flow_output: String,
    /// What to do next while a connection is being made with the keys.
    pub flow_pick_port: String,
    /// A `RubberBand`'s name.
    pub rubber_band: String,
    /// A `Marquee`'s name.
    pub marquee: String,
    /// A `TransformHandles`'s name.
    pub transform: String,
    /// A `CropOverlay`'s name.
    pub crop: String,
    /// A `Protractor`'s name.
    pub protractor: String,
    /// An `Eyedropper`'s name.
    pub eyedropper: String,
    /// What an `Eyedropper`'s tooltip says.
    pub eyedropper_hint: String,
    /// An armed `EyedropperArea`'s name.
    pub eyedropper_area: String,
    /// A `CompareView`'s name.
    pub compare: String,
    /// The caption over the first picture of a `CompareView`.
    pub compare_first: String,
    /// The caption over the second picture of a `CompareView`.
    pub compare_second: String,
    /// An `ImageLightbox`'s name, for a picture with no caption.
    pub lightbox: String,
    /// Goes to the previous picture of an `ImageLightbox`.
    pub lightbox_previous: String,
    /// Goes to the next picture of an `ImageLightbox`.
    pub lightbox_next: String,
    /// Which picture an `ImageLightbox` is showing, where `{0}` is which
    /// and `{1}` how many. [`Labels::lightbox_counter`] fills it in.
    pub lightbox_counter: String,
    /// A recurrence editor's name.
    pub recurrence_name: String,
    /// Once, daily, weekly, monthly and yearly, as an editor offers them.
    pub recurrence_frequencies: [String; 5],
    /// Before the interval, such as "Every".
    pub recurrence_every: String,
    /// The singular and plural of day, week, month and year.
    pub recurrence_units: [(String, String); 4],
    /// A rule repeating every day, week, month and year, in words.
    pub recurrence_each: [String; 4],
    /// What says when a rule stops.
    pub recurrence_ends_name: String,
    /// Never, on a date and after a number of times, as an editor offers them.
    pub recurrence_ends: [String; 3],
    /// After the number of times.
    pub recurrence_times: String,
    /// Before the last day, in words.
    pub recurrence_until: String,
    /// Before the next days, in words.
    pub recurrence_next: String,
    /// What a grid's group panel says while nothing is grouped.
    pub grid_drag_hint: String,
    /// Before a column a grid is grouped by.
    pub grid_grouped_by: String,
    /// Stops grouping a grid by a column.
    pub grid_remove_group: String,
    /// Opens every group of a grid.
    pub grid_expand_all: String,
    /// Closes every group of a grid.
    pub grid_collapse_all: String,
    /// Sorts a grid's column from smallest to largest.
    pub grid_sort_ascending: String,
    /// Sorts a grid's column from largest to smallest.
    pub grid_sort_descending: String,
    /// Stops sorting by a grid's column.
    pub grid_clear_sort: String,
    /// Groups a grid by a column.
    pub grid_group_by_column: String,
    /// Stops grouping a grid by a column, in its menu.
    pub grid_ungroup_column: String,
    /// Keeps a grid's column in place.
    pub grid_pin_column: String,
    /// Lets a grid's column scroll again.
    pub grid_unpin_column: String,
    /// Hides a grid's column.
    pub grid_hide_column: String,
    /// Fits a grid's column to its content.
    pub grid_fit_column: String,
    /// Shows a grid's group panel.
    pub grid_show_panel: String,
    /// Hides a grid's group panel.
    pub grid_hide_panel: String,
    /// Opens a grid's column menu.
    pub grid_column_menu: String,
    /// Filters a grid's column by its values.
    pub grid_filter_values: String,
    /// Removes a grid's filters.
    pub grid_clear_filter: String,
    /// The text of an `ExpressionEditor`.
    pub expression_text: String,
    /// The search field of an `ExpressionEditor`.
    pub expression_search: String,
    /// Puts the chosen item into an `ExpressionEditor` at the caret.
    pub expression_insert: String,
    /// An `ExpressionEditor` whose text has no problem.
    pub expression_valid: String,
    /// Where a syntax error is, with `{line}` and `{column}` in it.
    pub expression_error_at: String,
    /// Before the names an `ExpressionEditor` does not know.
    pub expression_unknown: String,
    /// What an `ExpressionEditor` says when nothing matches the search.
    pub expression_no_match: String,
    /// The groups of an `ExpressionEditor`, in order: fields,
    /// parameters, logical, text, math, date and time, reporting and aggregate functions,
    /// operators and constants.
    pub expression_groups: [String; 10],
    /// The operators an `ExpressionEditor` lists: what it inserts
    /// and what it does.
    pub expression_operators: Vec<(String, String)>,
    /// The constants an `ExpressionEditor` lists: what it inserts
    /// and what it is.
    pub expression_constants: Vec<(String, String)>,
    /// What each function of the expression catalog does, by its name, such as
    /// `("Round", "…")`. A function missing here shows the catalog's own English text.
    pub expression_functions: Vec<(String, String)>,
    /// How each function of the expression catalog is called, by the same key as
    /// `expression_functions`, with its parameters named in this language.
    pub expression_signatures: Vec<(String, String)>,
    /// What each syntax error says, by its code, with `{0}`, `{1}`… where its arguments go.
    /// A code missing here shows the parser's English message.
    pub expression_errors: Vec<(String, String)>,
    /// Before what a grid is filtering by.
    pub grid_filter_label: String,
    /// Opens the editor of a grid's whole filter.
    pub grid_edit_filter: String,
    /// Placeholder of the search in a column's filter list.
    pub grid_filter_search: String,
    /// The tab of a grid's filter editor that builds the filter line by line.
    pub grid_filter_visual: String,
    /// The tab of a grid's filter editor that writes the filter as an expression.
    pub grid_filter_text: String,
    /// Every value of a grid's column, in its filter.
    pub grid_all_values: String,
    /// A grid's menu of saved arrangements.
    pub grid_scenario: String,
    /// Saves how a grid is arranged, under a new name.
    pub grid_scenario_save: String,
    /// Saves over the scenario in use.
    pub grid_scenario_update: String,
    /// Deletes the scenario in use.
    pub grid_scenario_delete: String,
    /// Puts a grid back as it first was.
    pub grid_scenario_original: String,
    /// What to call a scenario.
    pub grid_scenario_name: String,
    /// Confirms a new scenario.
    pub grid_save: String,
    /// Before how many rows a grid has per page.
    pub grid_rows_per_page: String,
    /// Asks a server for a grid's rows again.
    pub grid_try_again: String,
    /// Before a page's number in a grid's pager.
    pub grid_page: String,
    /// A grid's pager goes back.
    pub grid_previous: String,
    /// A grid's pager goes on.
    pub grid_next: String,
    /// Opens the editor of a column's summary.
    pub grid_edit_summary: String,
    /// Before the column whose summary is being edited.
    pub grid_summary_of: String,
    /// No summary for a column.
    pub grid_summary_none: String,
    /// A column's summary from a formula.
    pub grid_summary_formula: String,
    /// What a formula looks like.
    pub grid_formula_hint: String,
    /// Before what a formula comes to.
    pub grid_formula_result: String,
    /// Where a formula could not be worked out.
    pub grid_formula_error: String,
    /// Before the columns a formula can use.
    pub grid_formula_columns: String,
    /// Before the functions a formula can use.
    pub grid_formula_functions: String,
    /// Puts a formula to use.
    pub grid_apply: String,
    /// A grid's search field.
    pub grid_search: String,
    /// A grid's grouping menu.
    pub grid_group: String,
    /// Removes a grid's grouping.
    pub grid_clear_grouping: String,
    /// A grid's column menu.
    pub grid_columns: String,
    /// Puts a grid's sorting and filters back.
    pub grid_reset: String,
    /// A grid's export button.
    pub grid_export: String,
    /// A grid's menu of file formats.
    pub grid_export_file: String,
    /// A data grid's button that adds a row.
    pub grid_add_row: String,
    /// A data grid's button that removes the selected rows.
    pub grid_delete_rows: String,
    /// A grid whose filters leave nothing to show.
    pub grid_no_rows: String,
    /// A grid with no rows at all.
    pub grid_no_records: String,
    /// A grid's name.
    pub grid_name: String,
    /// A row's check box.
    pub grid_select_row: String,
    /// The verb before a row's name on its check box: "Select Ana".
    pub grid_select: String,
    /// A column sorted up.
    pub grid_sorted_ascending: String,
    /// A column sorted down.
    pub grid_sorted_descending: String,
    /// A text column's filter hint.
    pub grid_filter_hint: String,
    /// A number column's filter hint.
    pub grid_number_filter_hint: String,
    /// Before a column's name, in its filter's name.
    pub grid_filter_column: String,
    /// A group with no value.
    pub grid_empty: String,
    /// Between shown and total rows.
    pub grid_of: String,
    /// After the row count.
    pub grid_rows: String,
    /// After the count of selected rows.
    pub grid_selected: String,
    /// What a `Rating` is called when nothing says what is rated.
    pub rating: String,
    /// Where a `Rating` stands, where `{0}` is its score and `{1}` its maximum.
    /// [`Labels::rating_value`] fills it in.
    pub rating_value: String,
    /// A `DaySeparator` for the day before today.
    pub yesterday: String,
    /// The title of a `ShortcutSheet`.
    pub shortcut_sheet: String,
    /// What a [`FavoriteToggle`](crate::FavoriteToggle) is called when nothing says what it marks.
    pub favorite: String,
    /// A [`SwatchGrid`](crate::SwatchGrid)'s name.
    pub swatches: String,
    /// A [`NinePointAnchor`](crate::NinePointAnchor)'s name.
    pub anchor_point: String,
    /// The points of a [`NinePointAnchor`](crate::NinePointAnchor), row by row from the top left.
    pub anchor_names: [String; 9],
    /// The upper color of a [`ColorPair`](crate::ColorPair).
    pub color_foreground: String,
    /// The lower color of a [`ColorPair`](crate::ColorPair).
    pub color_background: String,
    /// Swaps the colors of a [`ColorPair`](crate::ColorPair).
    pub color_swap: String,
    /// Puts back the first colors of a [`ColorPair`](crate::ColorPair).
    pub color_reset: String,
    /// Count, sum, average, smallest and largest, as a grid's footer calls them.
    pub grid_summaries: [String; 5],
    /// Runs the code of a `Playground`.
    pub playground_run: String,
    /// Puts the chosen example of a `Playground` back.
    pub playground_restore: String,
    /// The examples of a `Playground`.
    pub playground_example: String,
    /// The code tab of a `Playground`.
    pub playground_code: String,
    /// What the code of a `Playground` makes.
    pub playground_result: String,
    /// The result a `Playground` keeps while the code has problems.
    pub playground_last_good: String,
    /// Before the line of a problem.
    pub playground_line: String,
    /// Before the column of a problem.
    pub playground_column: String,
    /// Saves what the code of a `Playground` makes as a file to keep.
    pub playground_download: String,
    /// The panel under the result of a `Playground`.
    pub playground_output: String,
    /// The field a `CodeEditor` finds text with, and its Cmd/Ctrl+F.
    pub code_find: String,
    /// The list of suggestions a `CodeEditor` shows under the caret.
    pub code_suggestions: String,
    /// The field and the button that replace what a `CodeEditor` found.
    pub code_replace: String,
    /// The button that replaces everything a `CodeEditor` found.
    pub code_replace_all: String,
    /// Goes to the match before, in a `CodeEditor`.
    pub code_find_previous: String,
    /// Goes to the next match, in a `CodeEditor`.
    pub code_find_next: String,
    /// Finds only with the same capitals, in a `CodeEditor`.
    pub code_match_case: String,
    /// Finds only whole words, in a `CodeEditor`.
    pub code_whole_word: String,
    /// Which match of how many, in a `CodeEditor`: `{current}` and `{total}` are replaced.
    pub code_find_count: String,
    /// When a `CodeEditor` finds nothing.
    pub code_no_results: String,
    /// Shows or hides the replacing in the find bar of a `CodeEditor`.
    pub code_show_replace: String,
    /// Closes the find bar of a `CodeEditor`.
    pub code_close_find: String,
    /// What the code of a `Playground` printed.
    pub playground_console: String,
    /// The problems of the code of a `Playground`.
    pub playground_problems: String,
    /// When the code of a `Playground` has none.
    pub playground_no_problems: String,
    /// The `Playground` status line, where the caret is: `{line}` and `{column}` are replaced.
    pub playground_position: String,
    /// The `Playground` status line, how many spaces a Tab puts: `{count}` is replaced.
    pub playground_spaces: String,
    /// The language of the code of a `Playground` that is not colored, beside its file name.
    pub playground_plain_text: String,
    /// What a problem of a `Playground` does when clicked: `{place}` is replaced.
    pub playground_go_to: String,
    /// When the code of a `Playground` printed nothing.
    pub playground_nothing_printed: String,
    /// Empties the output of a `Playground`.
    pub playground_clear_output: String,
    /// Folds the output of a `Playground` away.
    pub playground_hide_output: String,
    /// Opens the output of a `Playground` again.
    pub playground_show_output: String,
    /// The menu of a narrow `Playground` with what is not Run.
    pub playground_more: String,
    /// Puts a `Playground` in full screen.
    pub playground_full_screen: String,
    /// Takes a `Playground` out of full screen.
    pub playground_exit_full_screen: String,
}

/// The texts whose keys start with `prefix`, by the rest of the key, in this language
/// and, where it has none, in English.
fn keyed(texts: &Texts, prefix: &str) -> Vec<(String, String)> {
    let catalog = texts.catalog();
    let mut keys: Vec<&str> = catalog
        .keys(texts.language())
        .chain(catalog.keys(Language::English.code()))
        .filter(|k| k.starts_with(prefix))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().map(|k| (k[prefix.len()..].to_owned(), texts.get(k))).collect()
}

impl Default for Labels {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Labels {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            close: texts.get("widgets.close"),
            help: texts.get("widgets.help"),
            navigation: texts.get("widgets.navigation"),
            toggle_sidebar: texts.get("widgets.toggle_sidebar"),
            dismiss: texts.get("widgets.dismiss"),
            send: texts.get("widgets.send"),
            listen: texts.get("widgets.listen"),
            tree_name: texts.get("widgets.tree_name"),
            tag_remove: texts.get("widgets.tag_remove"),
            pick_color: texts.get("widgets.pick_color"),
            no_color: texts.get("widgets.no_color"),
            color_shade: texts.get("widgets.color_shade"),
            color_hue: texts.get("widgets.color_hue"),
            color_alpha: texts.get("widgets.color_alpha"),
            color_hex: texts.get("widgets.color_hex"),
            pick_date_range: texts.get("widgets.pick_date_range"),
            candle_open: texts.get("widgets.candle_open"),
            candle_high: texts.get("widgets.candle_high"),
            candle_low: texts.get("widgets.candle_low"),
            candle_close: texts.get("widgets.candle_close"),
            sankey_cycle: texts.get("widgets.sankey_cycle"),
            list_name: texts.get("widgets.list_name"),
            filmstrip_name: texts.get("widgets.filmstrip_name"),
            document_name: texts.get("widgets.document_name"),
            page_word: texts.get("widgets.page_word"),
            page_of: texts.get("widgets.page_of"),
            previous_page: texts.get("widgets.previous_page"),
            next_page: texts.get("widgets.next_page"),
            file_browser_name: texts.get("widgets.file_browser_name"),
            file_name: texts.get("widgets.file_name"),
            file_size: texts.get("widgets.file_size"),
            attachments: texts.get("widgets.attachments"),
            attachment_remove: texts.get("widgets.attachment_remove"),
            attachment_send_as: texts.get("widgets.attachment_send_as"),
            selection_bar: texts.get("widgets.selection_bar"),
            selection_exit: texts.get("widgets.selection_exit"),
            selection_none: texts.get("widgets.selection_none"),
            selection_one: texts.get("widgets.selection_one"),
            selection_many: texts.get("widgets.selection_many"),
            audio_player: texts.get("widgets.audio_player"),
            audio_play: texts.get("widgets.audio_play"),
            audio_stop: texts.get("widgets.audio_stop"),
            audio_loading: texts.get("widgets.audio_loading"),
            audio_transcript: texts.get("widgets.audio_transcript"),
            audio_transcribe: texts.get("widgets.audio_transcribe"),
            audio_transcribing: texts.get("widgets.audio_transcribing"),
            audio_no_speech: texts.get("widgets.audio_no_speech"),
            chat_photo: texts.get("widgets.chat_photo"),
            chat_photo_loading: texts.get("widgets.chat_photo_loading"),
            chat_open: texts.get("widgets.chat_open"),
            file_modified: texts.get("widgets.file_modified"),
            folder_word: texts.get("widgets.folder_word"),
            folder_empty: texts.get("widgets.folder_empty"),
            previous_year: texts.get("widgets.previous_year"),
            next_year: texts.get("widgets.next_year"),
            day_marked: texts.get("widgets.day_marked"),
            sortable_name: texts.get("widgets.sortable_name"),
            sortable_hint: texts.get("widgets.sortable_hint"),
            moved_to: texts.get("widgets.moved_to"),
            document_tabs_name: texts.get("widgets.document_tabs_name"),
            dock_area_name: texts.get("widgets.dock_area_name"),
            dock_tabs_name: texts.get("widgets.dock_tabs_name"),
            dock_empty: texts.get("widgets.dock_empty"),
            dock_left: texts.get("widgets.dock_left"),
            dock_right: texts.get("widgets.dock_right"),
            dock_bottom: texts.get("widgets.dock_bottom"),
            dock_top: texts.get("widgets.dock_top"),
            dock_auto_hide: texts.get("widgets.dock_auto_hide"),
            dock_auto_hide_item: texts.get("widgets.dock_auto_hide_item"),
            dock_pin: texts.get("widgets.dock_pin"),
            dock_hidden: texts.get("widgets.dock_hidden"),
            dock_pinned: texts.get("widgets.dock_pinned"),
            dock_float_item: texts.get("widgets.dock_float_item"),
            dock_floating: texts.get("widgets.dock_floating"),
            dock_dock_back: texts.get("widgets.dock_dock_back"),
            dock_move_floating: texts.get("widgets.dock_move_floating"),
            dock_resize_floating: texts.get("widgets.dock_resize_floating"),
            dock_documents_center: texts.get("widgets.dock_documents_center"),
            dock_tools_docks: texts.get("widgets.dock_tools_docks"),
            dock_close_others: texts.get("widgets.dock_close_others"),
            dock_switcher: texts.get("widgets.dock_switcher"),
            dock_documents: texts.get("widgets.dock_documents"),
            dock_tools: texts.get("widgets.dock_tools"),
            dock_where_floating: texts.get("widgets.dock_where_floating"),
            dock_where_hidden: texts.get("widgets.dock_where_hidden"),
            dock_center: texts.get("widgets.dock_center"),
            dock_move_to: texts.get("widgets.dock_move_to"),
            dock_with: texts.get("widgets.dock_with"),
            dock_split_right: texts.get("widgets.dock_split_right"),
            dock_split_below: texts.get("widgets.dock_split_below"),
            dock_close: texts.get("widgets.dock_close"),
            dock_reopen: texts.get("widgets.dock_reopen"),
            dock_close_panel: texts.get("widgets.dock_close_panel"),
            dock_options: texts.get("widgets.dock_options"),
            dock_hide: texts.get("widgets.dock_hide"),
            dock_show: texts.get("widgets.dock_show"),
            dock_resize: texts.get("widgets.dock_resize"),
            dock_resize_pair: texts.get("widgets.dock_resize_pair"),
            dock_moved: texts.get("widgets.dock_moved"),
            dock_closed: texts.get("widgets.dock_closed"),
            close_tab: texts.get("widgets.close_tab"),
            new_tab: texts.get("widgets.new_tab"),
            close_other_tabs: texts.get("widgets.close_other_tabs"),
            close_tabs_to_right: texts.get("widgets.close_tabs_to_right"),
            unsaved: texts.get("widgets.unsaved"),
            wizard_back: texts.get("widgets.wizard_back"),
            wizard_next: texts.get("widgets.wizard_next"),
            wizard_finish: texts.get("widgets.wizard_finish"),
            wizard_cancel: texts.get("widgets.wizard_cancel"),
            wizard_done: texts.get("widgets.wizard_done"),
            wizard_error: texts.get("widgets.wizard_error"),
            split_resize: texts.get("widgets.split_resize"),
            panel_move: texts.get("widgets.panel_move"),
            panel_minimize: texts.get("widgets.panel_minimize"),
            panel_restore: texts.get("widgets.panel_restore"),
            scroll_back: texts.get("widgets.scroll_back"),
            scroll_forward: texts.get("widgets.scroll_forward"),
            window_minimize: texts.get("widgets.window_minimize"),
            window_maximize: texts.get("widgets.window_maximize"),
            window_restore: texts.get("widgets.window_restore"),
            value_adjust: texts.get("widgets.value_adjust"),
            rename: texts.get("widgets.rename"),
            rename_hint: texts.get("widgets.rename_hint"),
            file_drop: texts.get("widgets.file_drop"),
            file_choose: texts.get("widgets.file_choose"),
            file_remove: texts.get("widgets.file_remove"),
            file_wrong_kind: texts.get("widgets.file_wrong_kind"),
            file_too_large: texts.get("widgets.file_too_large"),
            file_too_many: texts.get("widgets.file_too_many"),
            image_choose: texts.get("widgets.image_choose"),
            image_replace: texts.get("widgets.image_replace"),
            image_none: texts.get("widgets.image_none"),
            image_invalid: texts.get("widgets.image_invalid"),
            tile_move: texts.get("widgets.tile_move"),
            tile_resize: texts.get("widgets.tile_resize"),
            cards_name: texts.get("widgets.cards_name"),
            grid_expand_row: texts.get("widgets.grid_expand_row"),
            grid_collapse_row: texts.get("widgets.grid_collapse_row"),
            tag_add: texts.get("widgets.tag_add"),
            wizard_progress: texts.get("widgets.wizard_progress"),
            culture: texts.get("widgets.culture"),
            loading: texts.get("widgets.loading"),
            decrease: texts.get("widgets.decrease"),
            increase: texts.get("widgets.increase"),
            required: texts.get("widgets.required"),
            pick_date: texts.get("widgets.pick_date"),
            select: texts.get("widgets.select"),
            search: texts.get("widgets.search"),
            no_results: texts.get("widgets.no_results"),
            search_commands: texts.get("widgets.search_commands"),
            no_commands: texts.get("widgets.no_commands"),
            command_palette: texts.get("widgets.command_palette"),
            theme: texts.get("widgets.theme"),
            theme_dark: texts.get("widgets.theme_dark"),
            theme_light: texts.get("widgets.theme_light"),
            theme_glass: texts.get("widgets.theme_glass"),
            theme_system: texts.get("widgets.theme_system"),
            theme_surfaces: texts.get("widgets.theme_surfaces"),
            theme_text: texts.get("widgets.theme_text"),
            theme_accent: texts.get("widgets.theme_accent"),
            theme_status: texts.get("widgets.theme_status"),
            theme_charts: texts.get("widgets.theme_charts"),
            theme_shape: texts.get("widgets.theme_shape"),
            theme_sizes: texts.get("widgets.theme_sizes"),
            theme_restore: texts.get("widgets.theme_restore"),
            theme_copy: texts.get("widgets.theme_copy"),
            theme_paste: texts.get("widgets.theme_paste"),
            theme_invalid: texts.get("widgets.theme_invalid"),
            theme_changed: texts.array("widgets.theme_changed"),
            theme_contrast: texts.get("widgets.theme_contrast"),
            copy: texts.get("widgets.copy"),
            copied: texts.get("widgets.copied"),
            release_versions: texts.get("widgets.release_versions"),
            release_newer: texts.get("widgets.release_newer"),
            release_older: texts.get("widgets.release_older"),
            release_of: texts.get("widgets.release_of"),
            diagnostics: texts.get("widgets.diagnostics"),
            diagnostics_test: texts.get("widgets.diagnostics_test"),
            diagnostics_testing: texts.get("widgets.diagnostics_testing"),
            diagnostics_testing_detail: texts.get("widgets.diagnostics_testing_detail"),
            diagnostics_test_good: texts.get("widgets.diagnostics_test_good"),
            diagnostics_test_fair: texts.get("widgets.diagnostics_test_fair"),
            diagnostics_test_poor: texts.get("widgets.diagnostics_test_poor"),
            diagnostics_test_fps: texts.get("widgets.diagnostics_test_fps"),
            diagnostics_test_cause_none: texts.get("widgets.diagnostics_test_cause_none"),
            diagnostics_timeline: texts.get("widgets.diagnostics_timeline"),
            diagnostics_timeline_detail: texts.get("widgets.diagnostics_timeline_detail"),
            diagnostics_why: texts.get("widgets.diagnostics_why"),
            diagnostics_why_none: texts.get("widgets.diagnostics_why_none"),
            diagnostics_causes: texts.array("widgets.diagnostics_causes"),
            diagnostics_views: texts.get("widgets.diagnostics_views"),
            diagnostics_batches: texts.get("widgets.diagnostics_batches"),
            diagnostics_memory: texts.get("widgets.diagnostics_memory"),
            diagnostics_copy_trace: texts.get("widgets.diagnostics_copy_trace"),
            diagnostics_test_cause_gpu: texts.get("widgets.diagnostics_test_cause_gpu"),
            diagnostics_test_cause_interface: texts.get("widgets.diagnostics_test_cause_interface"),
            diagnostics_software: texts.get("widgets.diagnostics_software"),
            diagnostics_software_detail: texts.get("widgets.diagnostics_software_detail"),
            diagnostics_unoptimized: texts.get("widgets.diagnostics_unoptimized"),
            diagnostics_unoptimized_detail: texts.get("widgets.diagnostics_unoptimized_detail"),
            diagnostics_hidden: texts.get("widgets.diagnostics_hidden"),
            diagnostics_no_gpu: texts.get("widgets.diagnostics_no_gpu"),
            diagnostics_frame: texts.get("widgets.diagnostics_frame"),
            diagnostics_update: texts.get("widgets.diagnostics_update"),
            diagnostics_acquire: texts.get("widgets.diagnostics_acquire"),
            diagnostics_render: texts.get("widgets.diagnostics_render"),
            diagnostics_report_bug: texts.get("widgets.diagnostics_report_bug"),
            diagnostics_report_shares: texts.get("widgets.diagnostics_report_shares"),
            diagnostics_report_shares_detail: texts.get("widgets.diagnostics_report_shares_detail"),
            diagnostics_report_failed: texts.get("widgets.diagnostics_report_failed"),
            diagnostics_report_failed_detail: texts.get("widgets.diagnostics_report_failed_detail"),
            diagnostics_report_questions: texts.get("widgets.diagnostics_report_questions"),
            diagnostics_report_truncated: texts.get("widgets.diagnostics_report_truncated"),
            previous_month: texts.get("widgets.previous_month"),
            next_month: texts.get("widgets.next_month"),
            today: texts.get("widgets.today"),
            weekdays: texts.array("widgets.weekdays"),
            months: texts.array("widgets.months"),
            month_year: texts.get("widgets.month_year"),
            short_date: texts.get("widgets.short_date"),
            form_errors: texts.get("widgets.form_errors"),
            clear: texts.get("widgets.clear"),
            filter_all: texts.get("widgets.filter_all"),
            filter_any: texts.get("widgets.filter_any"),
            filter_match: texts.get("widgets.filter_match"),
            filter_add_condition: texts.get("widgets.filter_add_condition"),
            filter_add_group: texts.get("widgets.filter_add_group"),
            filter_remove: texts.get("widgets.filter_remove"),
            filter_field: texts.get("widgets.filter_field"),
            filter_comparison: texts.get("widgets.filter_comparison"),
            filter_value: texts.get("widgets.filter_value"),
            filter_expression: texts.get("widgets.filter_expression"),
            filter_yes: texts.get("widgets.filter_yes"),
            filter_no: texts.get("widgets.filter_no"),
            filter_equal: texts.get("widgets.filter_equal"),
            filter_not_equal: texts.get("widgets.filter_not_equal"),
            filter_greater: texts.get("widgets.filter_greater"),
            filter_greater_or_equal: texts.get("widgets.filter_greater_or_equal"),
            filter_less: texts.get("widgets.filter_less"),
            filter_less_or_equal: texts.get("widgets.filter_less_or_equal"),
            filter_contains: texts.get("widgets.filter_contains"),
            filter_starts_with: texts.get("widgets.filter_starts_with"),
            filter_is_empty: texts.get("widgets.filter_is_empty"),
            filter_is_not_empty: texts.get("widgets.filter_is_not_empty"),
            filter_any_of: texts.get("widgets.filter_any_of"),
            filter_none_of: texts.get("widgets.filter_none_of"),
            ribbon: texts.get("widgets.ribbon"),
            ribbon_search: texts.get("widgets.ribbon_search"),
            ribbon_pin: texts.get("widgets.ribbon_pin"),
            ribbon_collapse: texts.get("widgets.ribbon_collapse"),
            carousel_previous: texts.get("widgets.carousel_previous"),
            carousel_next: texts.get("widgets.carousel_next"),
            carousel_slide: texts.get("widgets.carousel_slide"),
            verification_code: texts.get("widgets.verification_code"),
            cancel: texts.get("widgets.cancel"),
            confirm_by_typing: texts.get("widgets.confirm_by_typing"),
            printed_every_build: texts.get("widgets.printed_every_build"),
            more_commands: texts.get("widgets.more_commands"),
            decimal_separator: texts.char("format.decimal", '.'),
            group_separator: texts.char("format.thousands", ','),
            pick_time: texts.get("widgets.pick_time"),
            step_up: texts.get("widgets.step_up"),
            step_down: texts.get("widgets.step_down"),
            calendar_more: texts.get("widgets.calendar_more"),
            calendar_views: texts.array("widgets.calendar_views"),
            calendar_view_name: texts.get("widgets.calendar_view_name"),
            calendar_previous: texts.get("widgets.calendar_previous"),
            calendar_next: texts.get("widgets.calendar_next"),
            calendar_all_day: texts.get("widgets.calendar_all_day"),
            calendar_new_event: texts.get("widgets.calendar_new_event"),
            scheduler_resources: texts.get("widgets.scheduler_resources"),
            zoom_in: texts.get("widgets.zoom_in"),
            zoom_out: texts.get("widgets.zoom_out"),
            histogram: texts.get("widgets.histogram"),
            histogram_clipped_low: texts.get("widgets.histogram_clipped_low"),
            histogram_clipped_high: texts.get("widgets.histogram_clipped_high"),
            ruler: texts.get("widgets.ruler"),
            color_wheel: texts.get("widgets.color_wheel"),
            color_saturation: texts.get("widgets.color_saturation"),
            curve_editor: texts.get("widgets.curve_editor"),
            curve_point: texts.get("widgets.curve_point"),
            curve_points: texts.get("widgets.curve_points"),
            curve_input: texts.get("widgets.curve_input"),
            curve_output: texts.get("widgets.curve_output"),
            zoom: texts.get("widgets.zoom"),
            zoom_fit: texts.get("widgets.zoom_fit"),
            zoom_actual: texts.get("widgets.zoom_actual"),
            zoom_view: texts.get("widgets.zoom_view"),
            minimap: texts.get("widgets.minimap"),
            flow_canvas: texts.get("widgets.flow_canvas"),
            flow_count: texts.get("widgets.flow_count"),
            flow_controls: texts.get("widgets.flow_controls"),
            flow_lock: texts.get("widgets.flow_lock"),
            flow_unlock: texts.get("widgets.flow_unlock"),
            flow_refused: texts.get("widgets.flow_refused"),
            flow_disabled: texts.get("widgets.flow_disabled"),
            flow_arrange: texts.get("widgets.flow_arrange"),
            flow_group: texts.get("widgets.flow_group"),
            flow_members: texts.get("widgets.flow_members"),
            flow_title: texts.get("widgets.flow_title"),
            flow_turned: texts.get("widgets.flow_turned"),
            flow_run_choose_many: texts.get("widgets.flow_run_choose_many"),
            flow_run_follow: texts.get("widgets.flow_run_follow"),
            flow_run_none_chosen: texts.get("widgets.flow_run_none_chosen"),
            flow_layers: texts.get("widgets.flow_layers"),
            flow_layer_hide: texts.get("widgets.flow_layer_hide"),
            flow_layer_show: texts.get("widgets.flow_layer_show"),
            flow_layer_up: texts.get("widgets.flow_layer_up"),
            flow_layer_down: texts.get("widgets.flow_layer_down"),
            flow_run_breakpoint: texts.get("widgets.flow_run_breakpoint"),
            flow_debug: texts.get("widgets.flow_debug"),
            flow_run_start: texts.get("widgets.flow_run_start"),
            flow_run_step: texts.get("widgets.flow_run_step"),
            flow_run_continue: texts.get("widgets.flow_run_continue"),
            flow_run_stop: texts.get("widgets.flow_run_stop"),
            flow_run_hint: texts.get("widgets.flow_run_hint"),
            flow_run_moved: texts.get("widgets.flow_run_moved"),
            flow_run_ended: texts.get("widgets.flow_run_ended"),
            flow_run_choose: texts.get("widgets.flow_run_choose"),
            flow_run_done: texts.get("widgets.flow_run_done"),
            flow_run_dead_end: texts.get("widgets.flow_run_dead_end"),
            flow_run_stuck: texts.get("widgets.flow_run_stuck"),
            flow_run_looping: texts.get("widgets.flow_run_looping"),
            flow_run_no_start: texts.get("widgets.flow_run_no_start"),
            flow_check: texts.get("widgets.flow_check"),
            flow_check_ok: texts.get("widgets.flow_check_ok"),
            flow_check_found: texts.get("widgets.flow_check_found"),
            flow_check_close: texts.get("widgets.flow_check_close"),
            flow_field_fill: texts.get("widgets.flow_field_fill"),
            flow_field_stroke: texts.get("widgets.flow_field_stroke"),
            flow_field_text: texts.get("widgets.flow_field_text"),
            flow_field_stroke_width: texts.get("widgets.flow_field_stroke_width"),
            flow_field_dashed: texts.get("widgets.flow_field_dashed"),
            flow_library: texts.get("widgets.flow_library"),
            flow_library_new: texts.get("widgets.flow_library_new"),
            flow_library_save: texts.get("widgets.flow_library_save"),
            flow_library_open_file: texts.get("widgets.flow_library_open_file"),
            flow_library_save_file: texts.get("widgets.flow_library_save_file"),
            flow_library_delete: texts.get("widgets.flow_library_delete"),
            flow_library_delete_title: texts.get("widgets.flow_library_delete_title"),
            flow_library_delete_message: texts.get("widgets.flow_library_delete_message"),
            flow_library_name: texts.get("widgets.flow_library_name"),
            flow_library_untitled: texts.get("widgets.flow_library_untitled"),
            flow_library_unsaved: texts.get("widgets.flow_library_unsaved"),
            flow_library_saved: texts.get("widgets.flow_library_saved"),
            flow_library_count: texts.get("widgets.flow_library_count"),
            flow_running: texts.get("widgets.flow_running"),
            flow_done: texts.get("widgets.flow_done"),
            flow_skipped: texts.get("widgets.flow_skipped"),
            flow_cancelled: texts.get("widgets.flow_cancelled"),
            flow_pinned: texts.get("widgets.flow_pinned"),
            flow_item_one: texts.get("widgets.flow_item_one"),
            flow_items: texts.get("widgets.flow_items"),
            flow_open_search: texts.get("widgets.flow_open_search"),
            flow_open_none: texts.get("widgets.flow_open_none"),
            flow_open_close: texts.get("widgets.flow_open_close"),
            param_fixed: texts.get("widgets.param_fixed"),
            param_expression: texts.get("widgets.param_expression"),
            param_mode: texts.get("widgets.param_mode"),
            param_apply: texts.get("widgets.param_apply"),
            param_discard: texts.get("widgets.param_discard"),
            param_add: texts.get("widgets.param_add"),
            param_remove: texts.get("widgets.param_remove"),
            param_loading: texts.get("widgets.param_loading"),
            param_load_failed: texts.get("widgets.param_load_failed"),
            param_retry: texts.get("widgets.param_retry"),
            param_required: texts.get("widgets.param_required"),
            param_none: texts.get("widgets.param_none"),
            inspector_mode: texts.get("widgets.inspector_mode"),
            inspector_table: texts.get("widgets.inspector_table"),
            inspector_tree: texts.get("widgets.inspector_tree"),
            inspector_json: texts.get("widgets.inspector_json"),
            inspector_schema: texts.get("widgets.inspector_schema"),
            inspector_absent: texts.get("widgets.inspector_absent"),
            inspector_boolean: texts.get("widgets.inspector_boolean"),
            inspector_number: texts.get("widgets.inspector_number"),
            inspector_text: texts.get("widgets.inspector_text"),
            inspector_list: texts.get("widgets.inspector_list"),
            inspector_object: texts.get("widgets.inspector_object"),
            inspector_more: texts.get("widgets.inspector_more"),
            inspector_sampled: texts.get("widgets.inspector_sampled"),
            inspector_empty: texts.get("widgets.inspector_empty"),
            inspector_previous: texts.get("widgets.inspector_previous"),
            inspector_next: texts.get("widgets.inspector_next"),
            inspector_item: texts.get("widgets.inspector_item"),
            inspector_path: texts.get("widgets.inspector_path"),
            inspector_kind: texts.get("widgets.inspector_kind"),
            inspector_in: texts.get("widgets.inspector_in"),
            inspector_present: texts.get("widgets.inspector_present"),
            inspector_chosen: texts.get("widgets.inspector_chosen"),
            expression_result: texts.get("widgets.expression_result"),
            expression_failed: texts.get("widgets.expression_failed"),
            inspector_drag: texts.get("widgets.inspector_drag"),
            inspector_copy: texts.get("widgets.inspector_copy"),
            param_no_field: texts.get("widgets.param_no_field"),
            flow_export: texts.get("widgets.flow_export"),
            flow_export_png: texts.get("widgets.flow_export_png"),
            flow_export_svg: texts.get("widgets.flow_export_svg"),
            flow_export_pdf: texts.get("widgets.flow_export_pdf"),
            flow_export_selection: texts.get("widgets.flow_export_selection"),
            flow_bend: texts.get("widgets.flow_bend"),
            flow_no_turn: texts.get("widgets.flow_no_turn"),
            flow_inspector: texts.get("widgets.flow_inspector"),
            flow_inspector_empty: texts.get("widgets.flow_inspector_empty"),
            flow_inspector_node: texts.get("widgets.flow_inspector_node"),
            flow_inspector_edge: texts.get("widgets.flow_inspector_edge"),
            flow_inspector_nodes: texts.get("widgets.flow_inspector_nodes"),
            flow_inspector_edges: texts.get("widgets.flow_inspector_edges"),
            flow_inspector_mixed: texts.get("widgets.flow_inspector_mixed"),
            flow_inspector_read_only: texts.get("widgets.flow_inspector_read_only"),
            flow_toolbox: texts.get("widgets.flow_toolbox"),
            flow_toolbox_search: texts.get("widgets.flow_toolbox_search"),
            flow_toolbox_none: texts.get("widgets.flow_toolbox_none"),
            flow_toolbox_hint: texts.get("widgets.flow_toolbox_hint"),
            flow_field_title: texts.get("widgets.flow_field_title"),
            flow_field_line: texts.get("widgets.flow_field_line"),
            flow_field_kind: texts.get("widgets.flow_field_kind"),
            flow_field_shape: texts.get("widgets.flow_field_shape"),
            flow_field_width: texts.get("widgets.flow_field_width"),
            flow_field_height: texts.get("widgets.flow_field_height"),
            flow_field_x: texts.get("widgets.flow_field_x"),
            flow_field_y: texts.get("widgets.flow_field_y"),
            flow_field_status: texts.get("widgets.flow_field_status"),
            flow_field_label: texts.get("widgets.flow_field_label"),
            flow_field_start: texts.get("widgets.flow_field_start"),
            flow_field_end: texts.get("widgets.flow_field_end"),
            flow_field_animated: texts.get("widgets.flow_field_animated"),
            flow_status_names: texts.array("widgets.flow_status_names"),
            flow_shape_names: texts.array("widgets.flow_shape_names"),
            flow_line_names: texts.array("widgets.flow_line_names"),
            flow_cap_names: texts.array("widgets.flow_cap_names"),
            line_dash_names: texts.array("widgets.line_dash_names"),
            line_style: texts.get("widgets.line_style"),
            line_style_width: texts.get("widgets.line_style_width"),
            line_style_dash: texts.get("widgets.line_style_dash"),
            line_style_start: texts.get("widgets.line_style_start"),
            line_style_end: texts.get("widgets.line_style_end"),
            line_style_mixed: texts.get("widgets.line_style_mixed"),
            flow_menu_cut: texts.get("widgets.flow_menu_cut"),
            flow_menu_copy: texts.get("widgets.flow_menu_copy"),
            flow_menu_paste: texts.get("widgets.flow_menu_paste"),
            flow_menu_duplicate: texts.get("widgets.flow_menu_duplicate"),
            flow_menu_delete: texts.get("widgets.flow_menu_delete"),
            flow_menu_front: texts.get("widgets.flow_menu_front"),
            flow_menu_back: texts.get("widgets.flow_menu_back"),
            flow_menu_group: texts.get("widgets.flow_menu_group"),
            flow_menu_ungroup: texts.get("widgets.flow_menu_ungroup"),
            flow_menu_select_all: texts.get("widgets.flow_menu_select_all"),
            flow_connect_from: texts.get("widgets.flow_connect_from"),
            flow_edge_label: texts.get("widgets.flow_edge_label"),
            flow_input: texts.get("widgets.flow_input"),
            flow_output: texts.get("widgets.flow_output"),
            flow_pick_port: texts.get("widgets.flow_pick_port"),
            rubber_band: texts.get("widgets.rubber_band"),
            marquee: texts.get("widgets.marquee"),
            transform: texts.get("widgets.transform"),
            crop: texts.get("widgets.crop"),
            protractor: texts.get("widgets.protractor"),
            eyedropper: texts.get("widgets.eyedropper"),
            eyedropper_hint: texts.get("widgets.eyedropper_hint"),
            eyedropper_area: texts.get("widgets.eyedropper_area"),
            compare: texts.get("widgets.compare"),
            compare_first: texts.get("widgets.compare_first"),
            compare_second: texts.get("widgets.compare_second"),
            lightbox: texts.get("widgets.lightbox"),
            lightbox_previous: texts.get("widgets.lightbox_previous"),
            lightbox_next: texts.get("widgets.lightbox_next"),
            lightbox_counter: texts.get("widgets.lightbox_counter"),
            recurrence_name: texts.get("widgets.recurrence_name"),
            recurrence_frequencies: texts.array("widgets.recurrence_frequencies"),
            recurrence_every: texts.get("widgets.recurrence_every"),
            recurrence_units: texts.pairs("widgets.recurrence_units"),
            recurrence_each: texts.array("widgets.recurrence_each"),
            recurrence_ends_name: texts.get("widgets.recurrence_ends_name"),
            recurrence_ends: texts.array("widgets.recurrence_ends"),
            recurrence_times: texts.get("widgets.recurrence_times"),
            recurrence_until: texts.get("widgets.recurrence_until"),
            recurrence_next: texts.get("widgets.recurrence_next"),
            grid_drag_hint: texts.get("widgets.grid_drag_hint"),
            grid_grouped_by: texts.get("widgets.grid_grouped_by"),
            grid_remove_group: texts.get("widgets.grid_remove_group"),
            grid_expand_all: texts.get("widgets.grid_expand_all"),
            grid_collapse_all: texts.get("widgets.grid_collapse_all"),
            grid_sort_ascending: texts.get("widgets.grid_sort_ascending"),
            grid_sort_descending: texts.get("widgets.grid_sort_descending"),
            grid_clear_sort: texts.get("widgets.grid_clear_sort"),
            grid_group_by_column: texts.get("widgets.grid_group_by_column"),
            grid_ungroup_column: texts.get("widgets.grid_ungroup_column"),
            grid_pin_column: texts.get("widgets.grid_pin_column"),
            grid_unpin_column: texts.get("widgets.grid_unpin_column"),
            grid_hide_column: texts.get("widgets.grid_hide_column"),
            grid_fit_column: texts.get("widgets.grid_fit_column"),
            grid_show_panel: texts.get("widgets.grid_show_panel"),
            grid_hide_panel: texts.get("widgets.grid_hide_panel"),
            grid_column_menu: texts.get("widgets.grid_column_menu"),
            grid_filter_values: texts.get("widgets.grid_filter_values"),
            grid_clear_filter: texts.get("widgets.grid_clear_filter"),
            expression_text: texts.get("widgets.expression_text"),
            expression_search: texts.get("widgets.expression_search"),
            expression_insert: texts.get("widgets.expression_insert"),
            expression_valid: texts.get("widgets.expression_valid"),
            expression_error_at: texts.get("widgets.expression_error_at"),
            expression_unknown: texts.get("widgets.expression_unknown"),
            expression_no_match: texts.get("widgets.expression_no_match"),
            expression_groups: texts.array("widgets.expression_groups"),
            expression_operators: texts.pair_list("widgets.expression_operators"),
            expression_constants: texts.pair_list("widgets.expression_constants"),
            expression_functions: keyed(texts, "widgets.expression_function."),
            expression_signatures: keyed(texts, "widgets.expression_signature."),
            expression_errors: keyed(texts, "widgets.expression_error."),
            grid_filter_label: texts.get("widgets.grid_filter_label"),
            grid_edit_filter: texts.get("widgets.grid_edit_filter"),
            grid_filter_search: texts.get("widgets.grid_filter_search"),
            grid_filter_visual: texts.get("widgets.grid_filter_visual"),
            grid_filter_text: texts.get("widgets.grid_filter_text"),
            grid_all_values: texts.get("widgets.grid_all_values"),
            grid_scenario: texts.get("widgets.grid_scenario"),
            grid_scenario_save: texts.get("widgets.grid_scenario_save"),
            grid_scenario_update: texts.get("widgets.grid_scenario_update"),
            grid_scenario_delete: texts.get("widgets.grid_scenario_delete"),
            grid_scenario_original: texts.get("widgets.grid_scenario_original"),
            grid_scenario_name: texts.get("widgets.grid_scenario_name"),
            grid_save: texts.get("widgets.grid_save"),
            grid_rows_per_page: texts.get("widgets.grid_rows_per_page"),
            grid_try_again: texts.get("widgets.grid_try_again"),
            grid_page: texts.get("widgets.grid_page"),
            grid_previous: texts.get("widgets.grid_previous"),
            grid_next: texts.get("widgets.grid_next"),
            grid_edit_summary: texts.get("widgets.grid_edit_summary"),
            grid_summary_of: texts.get("widgets.grid_summary_of"),
            grid_summary_none: texts.get("widgets.grid_summary_none"),
            grid_summary_formula: texts.get("widgets.grid_summary_formula"),
            grid_formula_hint: texts.get("widgets.grid_formula_hint"),
            grid_formula_result: texts.get("widgets.grid_formula_result"),
            grid_formula_error: texts.get("widgets.grid_formula_error"),
            grid_formula_columns: texts.get("widgets.grid_formula_columns"),
            grid_formula_functions: texts.get("widgets.grid_formula_functions"),
            grid_apply: texts.get("widgets.grid_apply"),
            grid_search: texts.get("widgets.grid_search"),
            grid_group: texts.get("widgets.grid_group"),
            grid_clear_grouping: texts.get("widgets.grid_clear_grouping"),
            grid_columns: texts.get("widgets.grid_columns"),
            grid_reset: texts.get("widgets.grid_reset"),
            grid_export: texts.get("widgets.grid_export"),
            grid_export_file: texts.get("widgets.grid_export_file"),
            grid_add_row: texts.get("widgets.grid_add_row"),
            grid_delete_rows: texts.get("widgets.grid_delete_rows"),
            grid_no_rows: texts.get("widgets.grid_no_rows"),
            grid_no_records: texts.get("widgets.grid_no_records"),
            grid_name: texts.get("widgets.grid_name"),
            grid_select_row: texts.get("widgets.grid_select_row"),
            grid_select: texts.get("widgets.grid_select"),
            grid_sorted_ascending: texts.get("widgets.grid_sorted_ascending"),
            grid_sorted_descending: texts.get("widgets.grid_sorted_descending"),
            grid_filter_hint: texts.get("widgets.grid_filter_hint"),
            grid_number_filter_hint: texts.get("widgets.grid_number_filter_hint"),
            grid_filter_column: texts.get("widgets.grid_filter_column"),
            grid_empty: texts.get("widgets.grid_empty"),
            grid_of: texts.get("widgets.grid_of"),
            grid_rows: texts.get("widgets.grid_rows"),
            grid_selected: texts.get("widgets.grid_selected"),
            rating: texts.get("widgets.rating"),
            rating_value: texts.get("widgets.rating_value"),
            yesterday: texts.get("widgets.yesterday"),
            shortcut_sheet: texts.get("widgets.shortcut_sheet"),
            favorite: texts.get("widgets.favorite"),
            swatches: texts.get("widgets.swatches"),
            anchor_point: texts.get("widgets.anchor_point"),
            anchor_names: texts.array("widgets.anchor_names"),
            color_foreground: texts.get("widgets.color_foreground"),
            color_background: texts.get("widgets.color_background"),
            color_swap: texts.get("widgets.color_swap"),
            color_reset: texts.get("widgets.color_reset"),
            grid_summaries: texts.array("widgets.grid_summaries"),
            playground_run: texts.get("widgets.playground_run"),
            playground_restore: texts.get("widgets.playground_restore"),
            playground_example: texts.get("widgets.playground_example"),
            playground_code: texts.get("widgets.playground_code"),
            playground_result: texts.get("widgets.playground_result"),
            playground_last_good: texts.get("widgets.playground_last_good"),
            playground_line: texts.get("widgets.playground_line"),
            playground_column: texts.get("widgets.playground_column"),
            playground_download: texts.get("widgets.playground_download"),
            playground_output: texts.get("widgets.playground_output"),
            code_find: texts.get("widgets.code_find"),
            code_suggestions: texts.get("widgets.code_suggestions"),
            code_replace: texts.get("widgets.code_replace"),
            code_replace_all: texts.get("widgets.code_replace_all"),
            code_find_previous: texts.get("widgets.code_find_previous"),
            code_find_next: texts.get("widgets.code_find_next"),
            code_match_case: texts.get("widgets.code_match_case"),
            code_whole_word: texts.get("widgets.code_whole_word"),
            code_find_count: texts.get("widgets.code_find_count"),
            code_no_results: texts.get("widgets.code_no_results"),
            code_show_replace: texts.get("widgets.code_show_replace"),
            code_close_find: texts.get("widgets.code_close_find"),
            playground_console: texts.get("widgets.playground_console"),
            playground_problems: texts.get("widgets.playground_problems"),
            playground_no_problems: texts.get("widgets.playground_no_problems"),
            playground_position: texts.get("widgets.playground_position"),
            playground_spaces: texts.get("widgets.playground_spaces"),
            playground_plain_text: texts.get("widgets.playground_plain_text"),
            playground_go_to: texts.get("widgets.playground_go_to"),
            playground_nothing_printed: texts.get("widgets.playground_nothing_printed"),
            playground_clear_output: texts.get("widgets.playground_clear_output"),
            playground_hide_output: texts.get("widgets.playground_hide_output"),
            playground_show_output: texts.get("widgets.playground_show_output"),
            playground_more: texts.get("widgets.playground_more"),
            playground_full_screen: texts.get("widgets.playground_full_screen"),
            playground_exit_full_screen: texts.get("widgets.playground_exit_full_screen"),
        }
    }

    /// What the expression function `name` does, if these labels say it.
    pub fn expression_function(&self, name: &str) -> Option<&str> {
        self.expression_functions.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, d)| d.as_str())
    }

    /// How the expression function `name` is called, if these labels say it.
    pub fn expression_signature(&self, name: &str) -> Option<&str> {
        self.expression_signatures.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, s)| s.as_str())
    }

    /// The template of the syntax error `code`, if these labels say it.
    pub fn expression_error(&self, code: &str) -> Option<&str> {
        self.expression_errors.iter().find(|(c, _)| c == code).map(|(_, t)| t.as_str())
    }

    /// Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }

    /// The item that joins the group where `title` shows.
    pub fn dock_with_name(&self, title: &str) -> String {
        self.dock_with.replace("{title}", title)
    }

    /// The name of the button that closes the panel `title`.
    pub fn dock_close_name(&self, title: &str) -> String {
        self.dock_close_panel.replace("{title}", title)
    }

    /// The name of the button with the menu of the panel `title`.
    pub fn dock_options_name(&self, title: &str) -> String {
        self.dock_options.replace("{title}", title)
    }

    /// The name of the bar between two parts of a split.
    pub fn dock_resize_between(&self, first: &str, second: &str) -> String {
        self.dock_resize_pair.replace("{first}", first).replace("{second}", second)
    }

    /// What is announced after the panel `title` closed.
    pub fn dock_closed_name(&self, title: &str) -> String {
        self.dock_closed.replace("{title}", title)
    }

    /// What is announced after a reorder, such as "Moved to position 2 of 5".
    pub fn moved_to_position(&self, position: usize, total: usize) -> String {
        self.moved_to.replace("{position}", &position.to_string()).replace("{total}", &total.to_string())
    }

    /// The name of a page, such as "Page 2 of 9".
    pub fn page_name(&self, number: usize, total: usize) -> String {
        format!("{} {number}{}{total}", self.page_word, self.page_of)
    }

    /// A date the way people here write it, for a date picker.
    pub fn short_date(&self, date: Date) -> String {
        let month = &self.months[usize::from(date.month().clamp(1, 12)) - 1];
        format_with(&self.short_date, |index, out| {
            match index {
                0 => write!(out, "{:02}", date.day()),
                1 => write!(out, "{:02}", date.month()),
                2 => write!(out, "{:04}", date.year()),
                3 => write!(out, "{}", date.day()),
                4 => {
                    out.extend(month.chars().take(3));
                    Ok(())
                }
                _ => return false,
            }
            .is_ok()
        })
    }

    /// What a `TagBox` option says when it adds `typed`.
    pub fn tag_add(&self, typed: &str) -> String {
        ravenrs_app_i18n::format(&self.tag_add, &[typed])
    }

    /// Where a `Wizard` stands, at `step` of `count`.
    pub fn wizard_progress(&self, step: usize, count: usize) -> String {
        two_numbers(&self.wizard_progress, step, count)
    }

    /// How many tokens changed.
    pub fn theme_changed(&self, count: usize) -> String {
        two_numbers(&self.theme_changed[count.min(2)], count, 0)
    }

    /// A contrast, and the token it is measured over.
    pub fn theme_contrast(&self, ratio: f32, ground: &str) -> String {
        format_with(&self.theme_contrast, |index, out| match index {
            0 => {
                let at = out.len();
                let written = write!(out, "{ratio:.1}").is_ok();
                if let Some(point) = out[at..].find('.') {
                    let mut separator = [0; 4];
                    out.replace_range(
                        at + point..=at + point,
                        self.decimal_separator.encode_utf8(&mut separator),
                    );
                }
                written
            }
            1 => {
                out.push_str(ground);
                true
            }
            _ => false,
        })
    }

    /// What a `FlowCanvas` holds, for screen readers.
    pub fn flow_count(&self, nodes: usize, edges: usize) -> String {
        two_numbers(&self.flow_count, nodes, edges)
    }

    /// Which picture of how many an `ImageLightbox` is showing.
    pub fn lightbox_counter(&self, at: usize, of: usize) -> String {
        two_numbers(&self.lightbox_counter, at, of)
    }

    /// Where a `Rating` stands, from its score and its maximum.
    pub fn rating_value(&self, score: u32, max: u32) -> String {
        two_numbers(&self.rating_value, score as usize, max as usize)
    }

    /// A month and its year: `November 2026`, `novembro de 2026`.
    pub fn month_title(&self, date: Date) -> String {
        let month = &self.months[usize::from(date.month().clamp(1, 12)) - 1];
        format!("{month}{}{}", self.month_year, date.year())
    }

    /// A full date, for screen readers: `15 November 2026`, `15 novembro de 2026`.
    pub fn long_date(&self, date: Date) -> String {
        format!("{} {}", date.day(), self.month_title(date))
    }
}

/// A template with `{0}` and `{1}` filled in with these, each written where it goes.
fn two_numbers(template: &str, first: usize, second: usize) -> String {
    format_with(template, |index, out| match index {
        0 => write!(out, "{first}").is_ok(),
        1 => write!(out, "{second}").is_ok(),
        _ => false,
    })
}
