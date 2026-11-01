# Changelog

All notable changes to RavenRS are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [semantic versioning](https://semver.org/); while the version is 0.x, minor versions may contain breaking changes.

## [Unreleased]

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

[Unreleased]: https://github.com/alexkads/RavenRS/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/alexkads/RavenRS/releases/tag/v0.1.0
