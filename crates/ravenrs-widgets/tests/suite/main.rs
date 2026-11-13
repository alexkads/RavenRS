//! Every integration test of this package in one binary.
//!
//! Each file under `tests/` used to become an executable of its own, linked against the whole
//! dependency graph; one binary links once. Add a new test file here as a module.

mod lists_support;

mod accessibility;
mod app_components;
mod batch_parity;
mod charts;
mod chat;
mod code;
mod components;
mod conversation;
mod counter;
mod datagrid;
mod diagnostics;
mod dock;
mod editing_and_clipboard;
mod editors;
mod event_calendar;
mod expression_editor;
mod file_browser_year;
mod flow;
mod focus_scroll;
mod footer;
mod grid_filter_parity;
mod grid_records;
mod imaging;
mod imaging_overlays;
mod keyboard_and_basics;
mod layout_and_content;
mod layout_input_pieces;
mod lists;
mod live;
mod lottie;
mod motion;
mod page_motion;
mod performance_tools;
mod properties;
mod release_notes;
mod reorder;
mod ribbon;
mod sankey;
mod scheduler;
mod scroll_scene;
mod sidebar;
mod simple_pieces;
mod splash;
mod status_bar;
mod structural_extensions;
mod system_theme;
mod timeline;
mod ui;
mod widget_extensions;
