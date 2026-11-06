//! A simple calculator. Only components of the library and its tokens, in about a hundred
//! lines, so it doubles as a small and steady screen to compare performance with: the
//! scene `calculator` in `crates/ravenrs-perf` measures this very view.

use std::rc::Rc;

use ravenrs::core::Key;
use ravenrs::prelude::*;

pub mod engine;

use engine::{Engine, Op, Press};

/// The buttons, row by row: what they show, what they do and how they look.
const KEYS: [(&str, &str, Press, ButtonStyle); 20] = [
    ("C", "Clear", Press::Clear, ButtonStyle::Ghost),
    ("⌫", "Erase", Press::Backspace, ButtonStyle::Ghost),
    ("%", "Percent", Press::Percent, ButtonStyle::Ghost),
    ("÷", "Divide", Press::Op(Op::Divide), ButtonStyle::Secondary),
    ("7", "7", Press::Digit(7), ButtonStyle::Secondary),
    ("8", "8", Press::Digit(8), ButtonStyle::Secondary),
    ("9", "9", Press::Digit(9), ButtonStyle::Secondary),
    ("×", "Multiply", Press::Op(Op::Multiply), ButtonStyle::Secondary),
    ("4", "4", Press::Digit(4), ButtonStyle::Secondary),
    ("5", "5", Press::Digit(5), ButtonStyle::Secondary),
    ("6", "6", Press::Digit(6), ButtonStyle::Secondary),
    ("−", "Subtract", Press::Op(Op::Subtract), ButtonStyle::Secondary),
    ("1", "1", Press::Digit(1), ButtonStyle::Secondary),
    ("2", "2", Press::Digit(2), ButtonStyle::Secondary),
    ("3", "3", Press::Digit(3), ButtonStyle::Secondary),
    ("+", "Add", Press::Op(Op::Add), ButtonStyle::Secondary),
    ("±", "Change sign", Press::Negate, ButtonStyle::Secondary),
    ("0", "0", Press::Digit(0), ButtonStyle::Secondary),
    (".", "Point", Press::Point, ButtonStyle::Secondary),
    ("=", "Equals", Press::Equals, ButtonStyle::Primary),
];

/// The keyboard: what each key on it presses.
fn keyboard() -> Vec<(Key, Press)> {
    let character = |text: &str| Key::Character(text.into());
    let mut keys: Vec<(Key, Press)> =
        (0..=9).map(|digit| (character(&digit.to_string()), Press::Digit(digit))).collect();
    keys.extend([
        (character("."), Press::Point),
        (character(","), Press::Point),
        (character("+"), Press::Op(Op::Add)),
        (character("-"), Press::Op(Op::Subtract)),
        (character("*"), Press::Op(Op::Multiply)),
        (character("/"), Press::Op(Op::Divide)),
        (character("%"), Press::Percent),
        (character("="), Press::Equals),
        (Key::Enter, Press::Equals),
        (Key::Backspace, Press::Backspace),
        (Key::Escape, Press::Clear),
    ]);
    keys
}

/// The calculator.
#[derive(Debug, Default)]
pub struct Calculator {
    engine: State<Engine>,
}

impl View for Calculator {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        for (key, press) in keyboard() {
            let engine = self.engine.clone();
            cx.shortcut(key, false, Rc::new(move || engine.update(|engine| engine.press(press))));
        }
        let keys: Vec<AnyView> = KEYS
            .iter()
            .map(|&(label, name, press, style)| -> AnyView {
                let engine = self.engine.clone();
                Box::new(
                    Button::new(label)
                        .accessible_name(name)
                        .style(style)
                        .size(ButtonSize::Large)
                        .full_width()
                        .on_click(move || engine.update(|engine| engine.press(press))),
                )
            })
            .collect();
        cx.child(
            &VStack::new((
                VStack::new((Text::bound(&self.engine, |engine| engine.display().to_owned())
                    .style(TextStyle::Metric)
                    .truncate(),))
                .align(Align::End),
                Grid::new(4, keys).spacing(8.0),
            ))
            .align(Align::Stretch)
            .spacing(16.0)
            .padding(16.0)
            .grow(1.0),
        )
    }
}

/// The app.
pub fn app() -> Calculator {
    Calculator::default()
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
}
