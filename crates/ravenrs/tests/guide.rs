//! The snippets from docs/GETTING_STARTED.md and the README, compiled and run
//! headless so the documentation cannot drift from the API.

use ravenrs::prelude::*;
use ravenrs::widgets::grid::Value;

fn readme_app() -> impl View {
    let count = State::new(0);
    let increment = count.clone();
    VStack::new((
        Text::new("Welcome to RavenRS").style(TextStyle::Display),
        Card::new(VStack::new((
            Text::new("REVENUE").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            Text::new("R$ 128.450").style(TextStyle::Metric),
            Text::bound(&count, |n| format!("Clicked {n} times")).foreground(Semantic::InkMuted),
        ))),
        Button::new("Get Started").icon(IconName::Sparkles).on_click(move || increment.update(|n| *n += 1)),
    ))
    .spacing(24.0)
    .padding(32.0)
}

struct Counter {
    count: State<i32>,
}

impl Component for Counter {
    fn body(&self) -> impl View {
        let c = self.count.clone();
        HStack::new((
            Text::bound(&self.count, |c| format!("{c}")).style(TextStyle::Title),
            Button::new("+1").on_click(move || c.update(|n| *n += 1)),
        ))
    }
}

struct Pill(&'static str);

impl View for Pill {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let fill =
            if cx.is_hovered() { theme.color(Semantic::Accent) } else { theme.color(Semantic::Control) };
        let label = cx.child(&Text::new(self.0));
        let node = cx.container(
            ravenrs::layout::LayoutStyle { padding: (6.0, 12.0).into(), ..Default::default() },
            &[label],
            Paint::Quad(QuadPaint { corner_radius: 999.0, ..QuadPaint::new(fill) }),
        );
        cx.on_click(node, std::rc::Rc::new(|| {}));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

fn grid() -> impl View {
    let data = GridData::new(vec![
        vec![Value::from("Northwind"), Value::from("Brazil"), Value::from(240)],
        vec![Value::from("Aurora"), Value::from("Chile"), Value::from(86)],
    ]);
    DataGrid::new(
        data,
        [
            GridColumn::text("Customer").editable(),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
        ],
    )
    .selection_mode(SelectionMode::Multiple)
    .height(480.0)
}

#[test]
fn guide_snippets_build_and_render() {
    let enabled = State::new(true);
    let name = State::new(String::new());
    let volume = State::new(0.5_f32);
    let mut ui = Ui::new(
        VStack::new((
            readme_app(),
            Counter { count: State::new(0) },
            Pill("Beta"),
            Toggle::new(enabled.binding()),
            TextField::new(name.binding()),
            Slider::new(volume.binding()),
            ThemeSwitcher,
            grid(),
        )),
        Theme::raven_light(),
    );
    ui.set_viewport(Size::new(1024.0, 1600.0), 1.0);
    ui.update();
    assert!(ui.element_count() > 50);
    // The window options from the guide type-check too.
    let _ = ravenrs::native::WindowOptions::new("Hello").size(800.0, 500.0).theme(Theme::raven_light());
}
