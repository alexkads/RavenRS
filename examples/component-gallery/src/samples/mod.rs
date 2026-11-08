//! One small example per component, for the component pages of the site. The
//! pages show these functions as they are, so the code a reader copies compiles.
//!
//! Each file of this folder holds the examples of one kind of component.
//! `scripts/gen-component-pages.py` reads the markers in them: `// sample: Name, height`
//! before a function and `// end` after it.

use std::rc::Rc;

use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;

/// Gives a file of examples its `sample`, which builds the one with that name.
macro_rules! samples {
    ($($name:literal => $function:ident),* $(,)?) => {
        /// The names of the examples in this file.
        #[cfg(test)]
        pub(super) const NAMES: &[&str] = &[$($name),*];

        /// The example of a component of this file.
        pub(super) fn sample(name: &str) -> Option<AnyView> {
            match name {
                $($name => Some(Box::new($function())),)*
                _ => None,
            }
        }
    };
}

mod buttons;
mod color;
mod data;
mod display;
mod fields;
mod imaging;
mod layout;
mod media;
mod navigation;
mod overlays;
mod pickers;

/// Every component with an example, in the order the playground lists them, with the file
/// that holds it.
macro_rules! catalog {
    ($($name:literal => $file:ident),* $(,)?) => {
        /// The name of every component with an example.
        pub const NAMES: &[&str] = &[$($name),*];

        /// The example of a component.
        pub fn sample(name: &str) -> Option<AnyView> {
            match name {
                $($name => $file::sample(name),)*
                _ => None,
            }
        }

        /// The names in each file, for the test that they agree with this list.
        #[cfg(test)]
        const IN_FILES: &[&[&str]] = &[buttons::NAMES, color::NAMES, data::NAMES, display::NAMES, fields::NAMES, imaging::NAMES, layout::NAMES, media::NAMES, navigation::NAMES, overlays::NAMES, pickers::NAMES];
    };
}

catalog! {
    "Button" => buttons,
    "IconButton" => buttons,
    "ToggleButton" => buttons,
    "ToggleGroup" => buttons,
    "SegmentedControl" => buttons,
    "Toggle" => buttons,
    "Checkbox" => buttons,
    "RadioGroup" => buttons,
    "Slider" => buttons,
    "Stepper" => buttons,
    "ColorSwatch" => color,
    "ThemeSwitcher" => buttons,
    "ThemePicker" => buttons,
    "ThemeEditor" => buttons,
    "TextField" => fields,
    "Select" => fields,
    "Combobox" => fields,
    "DatePicker" => pickers,
    "Calendar" => pickers,
    "EventCalendar" => pickers,
    "Scheduler" => pickers,
    "RecurrenceEditor" => pickers,
    "InputOtp" => fields,
    "FormField" => fields,
    "Label" => fields,
    "TextArea" => fields,
    "NumberField" => fields,
    "MaskedField" => fields,
    "TimePicker" => pickers,
    "DateTimePicker" => pickers,
    "LookupField" => fields,
    "FormLayout" => fields,
    "FilterBuilder" => fields,
    "ExpressionEditor" => fields,
    "CodeEditor" => fields,
    "Playground" => fields,
    "Toolbar" => buttons,
    "SplitButton" => buttons,
    "ConfirmDialog" => overlays,
    "TreeView" => data,
    "TagBox" => fields,
    "ColorPicker" => color,
    "DateRangePicker" => pickers,
    "Wizard" => pickers,
    "StepIndicator" => pickers,
    "StepBadge" => pickers,
    "FileUpload" => pickers,
    "ImageField" => pickers,
    "TileLayout" => data,
    "CardView" => data,
    "AppShell" => navigation,
    "Ribbon" => navigation,
    "Menubar" => navigation,
    "Tabs" => navigation,
    "Sidebar" => navigation,
    "Breadcrumb" => navigation,
    "Pagination" => navigation,
    "Accordion" => navigation,
    "Disclosure" => navigation,
    "Dialog" => overlays,
    "Sheet" => overlays,
    "Popover" => overlays,
    "HoverCard" => overlays,
    "Tooltip" => overlays,
    "HelpTip" => overlays,
    "SidebarBrand" => navigation,
    "DropdownMenu" => overlays,
    "ContextMenu" => overlays,
    "SplashScreen" => overlays,
    "CommandPalette" => overlays,
    "Toaster" => overlays,
    "DataGrid" => data,
    "Table" => data,
    "List" => data,
    "Stat" => data,
    "Chart" => data,
    "ChartReference" => data,
    "SankeyChart" => data,
    "ChartCard" => data,
    "BarChart" => data,
    "AreaChart" => data,
    "Barcode" => data,
    "VStack" => layout,
    "HStack" => layout,
    "Grid" => layout,
    "Spacer" => layout,
    "ScrollView" => layout,
    "ScrollScene" => layout,
    "ScrollLinked" => layout,
    "Sticky" => layout,
    "Link" => navigation,
    "Ticker" => layout,
    "CountUp" => layout,
    "Anchor" => layout,
    "QuadField" => layout,
    "Splitter" => layout,
    "SplitPanes" => layout,
    "SplitHandle" => layout,
    "DockArea" => layout,
    "Card" => layout,
    "GlassCard" => layout,
    "Section" => layout,
    "Divider" => layout,
    "Footer" => navigation,
    "StatusBar" => navigation,
    "ToolRail" => navigation,
    "Timeline" => data,
    "AspectRatio" => layout,
    "Collapse" => layout,
    "MoviePlayer" => media,
    "MovieAssistant" => media,
    "Chat" => media,
    "ModelPicker" => media,
    "FloatingPanel" => overlays,
    "Text" => display,
    "CodeBlock" => display,
    "RichText" => display,
    "AttachmentStrip" => media,
    "AudioPlayerButton" => media,
    "SelectionBar" => overlays,
    "ReleaseNotes" => display,
    "Alert" => display,
    "Badge" => display,
    "Avatar" => display,
    "Kbd" => display,
    "ProgressBar" => display,
    "Spinner" => display,
    "Skeleton" => display,
    "Dot" => display,
    "OverlayBadge" => display,
    "Rating" => buttons,
    "IconTile" => display,
    "EmptyState" => display,
    "HeroBanner" => display,
    "MultiProgress" => display,
    "ShimmerText" => display,
    "DaySeparator" => display,
    "ShortcutSheet" => navigation,
    "ShortcutHintBar" => navigation,
    "FloatingToolbar" => buttons,
    "TitleBar" => navigation,
    "DropZone" => pickers,
    "ScrollEdgeArrow" => layout,
    "ValueSlider" => buttons,
    "InlineRename" => fields,
    "ZStack" => layout,
    "PropertyRow" => data,
    "PageHeader" => navigation,
    "ButtonGroup" => buttons,
    "FavoriteToggle" => buttons,
    "SwatchGrid" => color,
    "NinePointAnchor" => imaging,
    "ColorPair" => color,
    "Histogram" => color,
    "Ruler" => imaging,
    "ColorWheel" => color,
    "CurveEditor" => color,
    "ZoomControl" => imaging,
    "ZoomPanView" => imaging,
    "Minimap" => imaging,
    "RuleOfThirdsGrid" => imaging,
    "RubberBand" => imaging,
    "Marquee" => imaging,
    "TransformHandles" => imaging,
    "Protractor" => imaging,
    "CropOverlay" => imaging,
    "Eyedropper" => color,
    "CompareView" => media,
    "ImageLightbox" => media,
    "VirtualList" => data,
    "Filmstrip" => media,
    "PagedDocumentView" => data,
    "FileBrowser" => data,
    "YearView" => pickers,
    "SortableList" => data,
    "DocumentTabs" => navigation,
    "Icon" => display,
    "Image" => media,
    "Lottie" => media,
    "Carousel" => media,
    "Canvas" => media,
}

/// The files of examples, where the code of each one is read from.
const SOURCES: &[&str] = &[
    include_str!("buttons.rs"),
    include_str!("color.rs"),
    include_str!("data.rs"),
    include_str!("display.rs"),
    include_str!("fields.rs"),
    include_str!("imaging.rs"),
    include_str!("layout.rs"),
    include_str!("media.rs"),
    include_str!("navigation.rs"),
    include_str!("overlays.rs"),
    include_str!("pickers.rs"),
];

/// A colored picture to show in the image examples.
fn picture(hue: f32) -> ImageData {
    ImageData::from_fn(160, 90, move |x, y| {
        let (u, v) = (x as f32 / 160.0, y as f32 / 90.0);
        Color::rgb(0.2 + 0.6 * u * hue, 0.3 + 0.4 * v, 0.9 - 0.5 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

/// Shows one example alone, for the frame on a component's page.
pub struct SampleApp {
    content: AnyView,
}

impl std::fmt::Debug for SampleApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleApp").finish_non_exhaustive()
    }
}

impl SampleApp {
    /// The example of a component, if it has one.
    pub fn new(name: &str) -> Option<Self> {
        sample(name).map(|content| Self { content })
    }
}

impl View for SampleApp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = cx.child(self.content.as_ref());
        cx.container(
            LayoutStyle {
                grow: 1.0,
                padding: Insets::uniform(24.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        )
    }
}

/// The code of a component's example, as its page shows it: what a reader copies.
pub fn sample_code(name: &str) -> Option<String> {
    let marker = format!("\n// sample: {name}, ");
    let (source, at) = SOURCES.iter().find_map(|source| Some((*source, source.find(&marker)?)))?;
    let body = &source[at + marker.len()..];
    let body = &body[body.find('\n')? + 1..];
    let end = body.find("\n// end")?;
    let body = &body[..end];
    // What this file brings in for every example, said in the ones that use it, so what a reader
    // copies or downloads compiles.
    // A name written in full, as `std::collections::BTreeSet`, needs nothing.
    let unqualified = |name: &str| {
        body.match_indices(name).any(|(at, _)| {
            let before = body[..at].chars().next_back();
            let after = body[at + name.len()..].chars().next();
            let apart = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
            apart(before) && apart(after) && !body[..at].ends_with("::")
        })
    };
    let mut uses = String::new();
    for (module, names) in
        [("std::collections", &["BTreeSet"][..]), ("ravenrs::layout", &["Axis", "LayoutStyle"])]
    {
        let used: Vec<&str> = names.iter().copied().filter(|name| unqualified(name)).collect();
        match used.as_slice() {
            [] => {}
            [one] => uses.push_str(&format!("use {module}::{one};\n")),
            many => uses.push_str(&format!("use {module}::{{{}}};\n", many.join(", "))),
        }
    }
    Some(format!("{uses}use ravenrs::prelude::*;\n\n{body}\n"))
}

/// The Cargo project of an example as it is in the playground: the crates the gallery's
/// examples use, and the helpers it calls.
pub fn sample_project(name: &str, code: &str) -> ravenrs_project::Project {
    let mut project = ravenrs_project::Project::new(format!("{name} example"))
        .source(code)
        .dependency("ravenrs", &["code"]);
    if code.contains("ravenrs_app") {
        project = project.dependency("ravenrs-app", &[]).dependency("ravenrs-app-ui", &[]);
    }
    if code.contains("picture(") {
        let source = include_str!("mod.rs");
        let start = source.find("/// A colored picture").unwrap_or(0);
        let end = source[start..].find("\n}\n").map_or(source.len(), |at| start + at + 3);
        project = project.helper(&source[start..end]);
    }
    project
}

/// What the examples can call, with the helpers they share.
pub fn sample_runner() -> LiveRunner {
    let mut registry = live_registry();
    registry.free_function("picture", |args| {
        let hue = args.first().and_then(|v| match v.read() {
            ravenrs::live::Value::Float(n) => Some(n as f32),
            ravenrs::live::Value::Int(n) => Some(n as f32),
            _ => None,
        });
        let hue = hue.ok_or("`picture` takes a hue, such as 0.4")?;
        Ok(ravenrs::live::Value::Host(ravenrs::live::Host::cloneable("ImageData", picture(hue))))
    });
    ravenrs::movie::live_expose(&mut registry);
    ravenrs_app::live_expose(&mut registry);
    ravenrs_app_ui::live_expose(&mut registry);
    LiveRunner::with_registry(registry)
}

/// The example of a component in a [`Playground`], with its code beside it. Edits run through
/// the interpreter; an example it cannot read yet opens compiled, and says so.
pub struct SamplePlayground {
    /// Each example, by its component's name, with its code.
    examples: Rc<Vec<(String, String)>>,
    /// The one it opens on.
    start: usize,
    /// One component's page, or the playground of all of them.
    single: bool,
    runner: LiveRunner,
    /// Why the interpreter cannot read the example as it is, if it cannot.
    compiled: Option<String>,
}

impl std::fmt::Debug for SamplePlayground {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplePlayground").field("examples", &self.examples.len()).finish_non_exhaustive()
    }
}

impl SamplePlayground {
    /// The playground of a component, if it has an example.
    pub fn new(name: &str) -> Option<Self> {
        let code = sample_code(name)?;
        sample(name)?;
        let runner = sample_runner();
        let compiled = runner.run(&code).err().map(|problems| problems[0].message.clone());
        Some(Self {
            examples: Rc::new(vec![(name.to_owned(), code)]),
            start: 0,
            single: true,
            runner,
            compiled,
        })
    }

    /// The playground of every example, listed in its bar, opening on `start` when it has one.
    pub fn all(start: Option<&str>) -> Self {
        let examples: Vec<(String, String)> =
            NAMES.iter().filter_map(|name| Some(((*name).to_owned(), sample_code(name)?))).collect();
        let start =
            start.and_then(|wanted| examples.iter().position(|(name, _)| name == wanted)).unwrap_or(0);
        Self { examples: Rc::new(examples), start, single: false, runner: sample_runner(), compiled: None }
    }
}

/// Runs edited code through the interpreter. An example as written always shows, compiled,
/// until the interpreter reads it; what the code prints comes from the interpreter.
struct SampleRunner {
    examples: Rc<Vec<(String, String)>>,
    live: LiveRunner,
}

impl Runner for SampleRunner {
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
        match self.live.run(source) {
            Ok(view) => Ok(view),
            Err(problems) => match self.examples.iter().find(|(_, code)| code == source) {
                Some((name, _)) => Ok(sample(name).unwrap_or_else(|| Box::new(Spacer::new()))),
                None => Err(problems),
            },
        }
    }

    fn take_output(&self) -> RunOutput {
        self.live.take_output()
    }

    fn watch(&self, wake: Rc<dyn Fn()>) {
        self.live.watch(wake);
    }
}

impl View for SamplePlayground {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let runner = SampleRunner { examples: self.examples.clone(), live: self.runner.clone() };
        let first = self.examples.get(self.start).map(|(_, code)| code.clone()).unwrap_or_default();
        let (start, single) = (self.start, self.single);
        let source = cx.use_state(move || first);
        let chosen = cx.use_state(move || start);
        let title = if single { self.examples[0].0.clone() } else { "Playground".to_owned() };
        let mut playground =
            Playground::new(runner).title(title).source(source.binding()).chosen_example(chosen.binding());
        // On a component's page the example is the point: the output waits folded.
        playground = playground.output_open(!single);
        for (name, code) in self.examples.iter() {
            playground = playground.example(name.clone(), code.clone());
        }
        let examples = self.examples.clone();
        playground = playground.download(move |code| {
            let name = examples.get(chosen.get()).map_or("Playground", |(name, _)| name.as_str());
            let project = sample_project(name, code);
            let bytes = project.zip().map_err(|problem| vec![CodeDiagnostic::error(1, 1, 1, problem)])?;
            Ok((format!("{}.zip", project.crate_name()), bytes))
        });
        if let Some(reason) = &self.compiled {
            playground = playground.note(format!(
                "This example runs compiled: the playground cannot read it yet ({reason}). Edits show what it reads."
            ));
        }
        let content = cx.child(&playground);
        cx.container(
            LayoutStyle {
                grow: 1.0,
                // Held to the window, so an example taller than it scrolls in the result.
                min_width: Some(0.0),
                min_height: Some(0.0),
                // In full screen the playground is the whole screen, edge to edge.
                padding: Insets::uniform(if cx.full_screen().is_on() { 0.0 } else { 16.0 }),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        )
    }
}

/// Components without an example would make the site's list a promise it does not keep.
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The height `gen-component-pages.py` gives the playground of an example on its page,
    /// with the numbers it uses.
    fn stage_height(name: &str) -> f32 {
        let script = include_str!("../../../../scripts/gen-component-pages.py");
        let names = "PAGE, BAR, LINE, EDITOR, STATUS, AROUND_EXAMPLE = ";
        let line = script.lines().find_map(|l| l.strip_prefix(names)).expect("the numbers of the page");
        let n: Vec<f32> = line.split(", ").map(|v| v.trim().parse().expect("a number")).collect();
        let marker = format!("\n// sample: {name}, ");
        let source = SOURCES.iter().find(|source| source.contains(&marker)).expect("its marker");
        let rest = &source[source.find(&marker).expect("its marker") + marker.len()..];
        let example: f32 = rest[..rest.find('\n').expect("a line")].parse().expect("its height");
        let lines = (sample_code(name).expect("its code").matches('\n').count() + 1) as f32;
        n[0] + n[1] + (lines * n[2] + n[3] + n[4]).max(example + n[5])
    }

    /// On a component's page the playground fits its box: every line of the code shows, and
    /// the status under it and the folded output under the example, nothing cut at the bottom.
    #[test]
    fn the_playground_of_each_page_fits_its_box() {
        let mut cut = Vec::new();
        for name in NAMES {
            let Some(playground) = SamplePlayground::new(name) else { continue };
            let height = stage_height(name);
            let mut ui = Ui::new(playground, Theme::raven_dark());
            ui.set_viewport(Size::new(824.0, height), 1.0);
            ui.update();
            ui.update();
            let tree = ui.access_tree();
            let bottom = |label: &str| {
                tree.nodes
                    .iter()
                    .find(|n| n.name.as_deref().is_some_and(|s| s.starts_with(label)))
                    .map_or(f32::MAX, |n| n.bounds.max_y())
            };
            let lowest = bottom("No problems").max(bottom("Show the output"));
            let lines = sample_code(name).unwrap_or_default().matches('\n').count() + 1;
            let needed = lines as f32 * 22.0 + 12.0;
            let editor = tree
                .nodes
                .iter()
                .find(|n| n.role == ravenrs::core::Role::TextInput)
                .map_or(0.0, |n| n.bounds.height());
            if lowest > height - 16.0 || editor < needed {
                cut.push(format!("{name}: bottom {lowest} in {height}, code {editor} of {needed}"));
            }
        }
        assert!(cut.is_empty(), "cut at the bottom: {cut:#?}");
    }

    /// Each example is in one file, and the list of all of them names each one once.
    #[test]
    fn the_files_hold_the_examples_the_list_names() {
        let in_files: Vec<&str> = IN_FILES.iter().flat_map(|names| names.iter().copied()).collect();
        let listed: BTreeSet<&str> = NAMES.iter().copied().collect();
        assert_eq!(in_files.len(), NAMES.len());
        assert_eq!(in_files.iter().copied().collect::<BTreeSet<_>>(), listed);
        for source in SOURCES {
            assert_eq!(source.matches("\n// sample: ").count(), source.matches("\n// end\n").count());
        }
        let marked: usize = SOURCES.iter().map(|source| source.matches("\n// sample: ").count()).sum();
        assert_eq!(marked, NAMES.len());
    }

    #[test]
    fn names_are_unique_and_every_example_builds() {
        let unique: std::collections::BTreeSet<&&str> = NAMES.iter().collect();
        assert_eq!(unique.len(), NAMES.len());
        for name in NAMES {
            let app = SampleApp::new(name).unwrap_or_else(|| panic!("{name}"));
            let mut ui = Ui::new(app, Theme::raven_dark());
            ui.set_viewport(Size::new(800.0, 500.0), 1.0);
            ui.update();
        }
        assert!(SampleApp::new("Nope").is_none());
    }

    /// Every example runs from its code in the interpreter, except those in
    /// `live-pending.txt`, which says why each one does not yet.
    #[test]
    fn coverage_of_the_interpreter() {
        let runner = sample_runner();
        let pending: BTreeSet<&str> = include_str!("../../live-pending.txt")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| line.split(':').next())
            .collect();
        let mut failing = BTreeSet::new();
        let mut unexpected = Vec::new();
        for name in NAMES {
            let code = sample_code(name).unwrap_or_default();
            match runner.run(&code) {
                Ok(view) => {
                    let mut ui = Ui::new(view, Theme::raven_dark());
                    ui.set_viewport(Size::new(800.0, 500.0), 1.0);
                    ui.update();
                    // What fails later, in a paint or a closure the component calls, counts too.
                    let late = ravenrs::live::late_problems();
                    if let Some(problem) = late.first() {
                        failing.insert(*name);
                        if !pending.contains(name) {
                            unexpected.push(format!(
                                "{name}: {} ({}:{})",
                                problem.message, problem.line, problem.column
                            ));
                        }
                        continue;
                    }
                    // The interpreted example builds as many elements as the compiled one.
                    let mut compiled =
                        Ui::new(sample(name).unwrap_or_else(|| panic!("{name}")), Theme::raven_dark());
                    compiled.set_viewport(Size::new(800.0, 500.0), 1.0);
                    compiled.update();
                    if compiled.element_count() != ui.element_count() {
                        failing.insert(*name);
                        if !pending.contains(name) {
                            unexpected.push(format!(
                                "{name}: {} elements interpreted, {} compiled",
                                ui.element_count(),
                                compiled.element_count()
                            ));
                        }
                    }
                }
                Err(problems) => {
                    failing.insert(*name);
                    if !pending.contains(name) {
                        unexpected.push(format!("{name}: {}", problems[0].message));
                    }
                }
            }
        }
        assert!(unexpected.is_empty(), "these do not run in the interpreter:\n{}", unexpected.join("\n"));
        let now_running: Vec<&&str> = pending.difference(&failing).collect();
        assert!(now_running.is_empty(), "these run now, take them off live-pending.txt: {now_running:?}");
    }

    #[test]
    fn every_example_has_its_code_and_a_playground() {
        for name in NAMES {
            let code = sample_code(name).unwrap_or_else(|| panic!("no code for {name}"));
            assert!(
                code.starts_with("use ")
                    && code.contains("use ravenrs::prelude::*;\n\n")
                    && code.contains("fn "),
                "{name}: {code}"
            );
            let mut ui =
                Ui::new(SamplePlayground::new(name).unwrap_or_else(|| panic!("{name}")), Theme::raven_dark());
            ui.set_viewport(Size::new(1100.0, 600.0), 1.0);
            ui.update();
        }
        assert!(SamplePlayground::new("Nope").is_none());
    }

    #[test]
    fn the_playground_of_all_lists_every_example_and_opens_on_the_one_asked() {
        let all = SamplePlayground::all(Some("Slider"));
        assert_eq!(all.examples.len(), NAMES.len());
        assert_eq!(all.examples[all.start].0, "Slider");
        assert_eq!(SamplePlayground::all(Some("Nope")).start, 0);
        let mut ui = Ui::new(all, Theme::raven_dark());
        ui.set_viewport(Size::new(1100.0, 600.0), 1.0);
        ui.update();
        ui.update();
        let names: Vec<String> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
        assert!(names.iter().any(|n| n == "Playground"), "{names:?}");
        assert!(names.iter().any(|n| n == "Download project"));
    }
}
