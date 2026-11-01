//! The UI shared by the native and the web build of the first RavenRS milestone.

use std::rc::Rc;

use ravenrs::prelude::*;

/// A reusable component: a labelled headline number in a card, with a note
/// under it. It shows how apps build their own components from RavenRS views.
pub struct MetricCard {
    label: &'static str,
    value: &'static str,
    /// Recomputed on every build, so it can read state.
    note: Rc<dyn Fn() -> String>,
}

impl std::fmt::Debug for MetricCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricCard").field("label", &self.label).field("value", &self.value).finish()
    }
}

impl MetricCard {
    pub fn new(label: &'static str, value: &'static str, note: impl Fn() -> String + 'static) -> Self {
        Self { label, value, note: Rc::new(note) }
    }
}

impl Component for MetricCard {
    fn body(&self) -> impl View {
        let note = self.note.clone();
        Card::new(
            VStack::new((
                Text::new(self.label.to_uppercase()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                Text::new(self.value).style(TextStyle::Metric),
                Text::dynamic(move || note()).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0),
        )
        .corner_radius(24.0)
        .shadow(Shadow::soft())
    }
}

/// The app: a title, a card bound to a click counter, and two buttons.
pub fn app() -> impl View {
    let clicks = State::new(0_u32);
    let (increment, reset, read) = (clicks.clone(), clicks.clone(), clicks);

    VStack::new((
        Text::new("Welcome to RavenRS").style(TextStyle::Display),
        Text::new("One interface in Rust, drawn on the GPU in a native window and in the browser.")
            .foreground(Semantic::InkMuted),
        MetricCard::new("Revenue", "R$ 128.450", move || match read.get() {
            0 => "Press Get Started to count clicks.".to_owned(),
            1 => "Clicked once.".to_owned(),
            n => format!("Clicked {n} times."),
        }),
        HStack::new((
            Button::new("Get Started").on_click(move || increment.update(|n| *n += 1)),
            Button::new("Reset").style(ButtonStyle::Secondary).on_click(move || reset.set(0)),
        ))
        .spacing(12.0),
    ))
    .spacing(24.0)
    .padding(32.0)
    .max_width(640.0)
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match ravenrs::web::start(super::app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    /// Unmounts the app and releases its listeners and GPU resources.
    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}

#[cfg(test)]
mod tests {
    use ravenrs::core::{Event, Point, PointerButton, Primitive, Size};

    use super::*;

    fn texts(ui: &Ui) -> usize {
        ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
    }

    #[test]
    fn app_renders_and_counts_clicks() {
        let mut ui = Ui::new(app(), Theme::raven_dark());
        ui.set_viewport(Size::new(900.0, 600.0), 1.0);
        ui.update();
        assert_eq!(texts(&ui), 7, "title, subtitle, three card lines, two button labels");
        let accent = Theme::raven_dark().color(Semantic::Accent);
        let button = ui
            .scene()
            .primitives()
            .iter()
            .find_map(|p| match p {
                Primitive::Quad(q) if q.fill == accent.into() => Some(q.rect),
                _ => None,
            })
            .expect("primary button");
        let at: Point = button.center();
        for _ in 0..3 {
            ui.handle_event(&Event::PointerMove { position: at });
            ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
            ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
        }
        assert!(ui.needs_redraw());
        ui.update();
    }
}
