//! The channel with the real window (`--headed`): the commands land on the interface the window
//! owns, not on a copy, and the window is told apart from the session with none. The platform is
//! played here by a loop that lends its `Ui` to the driver, as `run_driven` does on the main
//! thread, so no window or GPU is needed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ravenrs_e2e_wire::{Command, Headed, Hello, Reply, codes, serve_on};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{
    BuildCx, Button, Driving, NodeId, Text, TextField, Ui, UiDriver, VStack, View, ViewExt,
};

/// A screen with a field and a button that adds a line, like creating an item.
struct Screen {
    name: State<String>,
    items: State<Vec<String>>,
}

impl View for Screen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (name, items) = (self.name.clone(), self.items.clone());
        let listed: Vec<String> = self.items.get();
        cx.child(&VStack::new((
            TextField::new(self.name.binding()).test_id("nome"),
            Button::new("Criar")
                .on_click(move || {
                    let new = name.get();
                    items.update(|list| list.push(new));
                })
                .test_id("criar"),
            Text::new(format!("{} itens: {}", listed.len(), listed.join(", "))).test_id("lista"),
        )))
    }
}

/// What the driver writes, kept to read back.
#[derive(Clone, Default)]
struct Out(Arc<Mutex<Vec<u8>>>);

impl Write for Out {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Out {
    fn lines(&self) -> Vec<String> {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap().lines().map(str::to_owned).collect()
    }
}

fn line(command: &Command) -> String {
    serde_json::to_string(command).unwrap() + "\n"
}

/// The window's interface, as the platform owns it.
fn window() -> (Ui, State<Vec<String>>) {
    let items = State::new(Vec::new());
    let mut ui =
        Ui::new(Screen { name: State::new(String::new()), items: items.clone() }, Theme::raven_light());
    ui.set_viewport(ravenrs_core::Size::new(640.0, 400.0), 1.0);
    ui.update();
    (ui, items)
}

/// Plays the platform: turns until the driver says who is in control other than itself, or the
/// replies reach `replies`.
fn turns(driver: &mut Headed, ui: &mut Ui, out: &Out, replies: usize) -> Driving {
    let started = Instant::now();
    loop {
        let driving = driver.drive(ui, started.elapsed().as_secs_f64());
        if driving != Driving::Driven || out.lines().len() > replies {
            return driving;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the driver never answered: {:?}", out.lines());
        ui.update();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_steps_act_on_the_interface_the_window_owns() {
    let (mut ui, items) = window();
    let input = [
        line(&Command::Fill { target: "#nome".into(), text: "Agente Norte".into() }),
        line(&Command::Click { target: "#criar".into() }),
        line(&Command::Mention { target: "#lista".into(), text: "1 itens: Agente Norte".into() }),
    ]
    .concat();
    let out = Out::default();
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(input), out.clone());
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&woken);
    driver.start(Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));
    let hello: Hello = serde_json::from_str(&out.lines()[0]).unwrap();
    assert!(hello.window, "the hello says the window is on screen");
    // The input ends after the three commands, as a tool that leaves: the window closes then.
    turns(&mut driver, &mut ui, &out, 3);
    assert_eq!(out.lines().len(), 4, "hello and one reply per command: {:?}", out.lines());
    let replies: Vec<Reply> = out.lines()[1..].iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(replies.iter().all(|r| r.ok), "{replies:?}");
    // The window's own state changed: the test drove it, not a copy of it.
    assert_eq!(items.get(), vec!["Agente Norte".to_owned()]);
    assert!(woken.load(Ordering::SeqCst) >= 3, "each command asks the platform for a turn");
}

#[test]
fn a_wrong_expectation_fails_with_the_same_code_as_with_no_window() {
    let (mut ui, _) = window();
    let wrong = Command::Check { target: "#lista".into(), text: "9 itens".into() };
    let out = Out::default();
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(line(&wrong)), out.clone());
    driver.start(Box::new(|| {}));
    turns(&mut driver, &mut ui, &out, 1);
    let reply: Reply = serde_json::from_str(&out.lines()[1]).unwrap();

    let (headless_ui_items, headless_out) = (State::new(Vec::new()), Out::default());
    let session = ravenrs_e2e::E2e::new(
        Screen { name: State::new(String::new()), items: headless_ui_items },
        Theme::raven_light(),
    );
    serve_on(Hello::of("Tela"), session, Cursor::new(line(&wrong)), headless_out.clone()).unwrap();
    let headless: Reply = serde_json::from_str(&headless_out.lines()[1]).unwrap();
    assert_eq!(reply.failure.as_ref().map(|f| f.code.clone()), headless.failure.map(|f| f.code));
    assert_eq!(reply.failure.unwrap().code, "E2E_EXPECTATION");
}

#[test]
fn bye_closes_the_window_and_release_leaves_it_to_the_person() {
    let (mut ui, _) = window();
    let out = Out::default();
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(line(&Command::Bye)), out.clone());
    driver.start(Box::new(|| {}));
    assert_eq!(turns(&mut driver, &mut ui, &out, 9), Driving::Done);

    let out = Out::default();
    // The tool releases the window and closes its end of the channel, as `Client::release` does.
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(line(&Command::Release)), out.clone());
    driver.start(Box::new(|| {}));
    assert_eq!(turns(&mut driver, &mut ui, &out, 9), Driving::Released);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(driver.drive(&mut ui, 1.0), Driving::Released, "the end of the input does not close it");
}

#[test]
fn a_tool_gone_without_bye_ends_the_window() {
    let (mut ui, _) = window();
    let out = Out::default();
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(String::new()), out.clone());
    driver.start(Box::new(|| {}));
    assert_eq!(turns(&mut driver, &mut ui, &out, 9), Driving::Done);
}

#[test]
fn closing_the_window_during_a_test_answers_with_its_code() {
    let out = Out::default();
    let mut driver = Headed::over(Hello::of("Tela"), Cursor::new(String::new()), out.clone());
    driver.window_closed();
    let reply: Reply = serde_json::from_str(&out.lines()[0]).unwrap();
    assert_eq!(reply.failure.unwrap().code, codes::WINDOW_CLOSED);
}

#[test]
fn release_with_no_window_ends_like_bye() {
    let session = ravenrs_e2e::E2e::new(
        Screen { name: State::new(String::new()), items: State::new(Vec::new()) },
        Theme::raven_light(),
    );
    let out = Out::default();
    let input = [line(&Command::Release), line(&Command::Snapshot)].concat();
    serve_on(Hello::of("Tela"), session, Cursor::new(input), out.clone()).unwrap();
    assert_eq!(out.lines().len(), 2, "hello and the release, nothing after it: {:?}", out.lines());
    let hello: Hello = serde_json::from_str(&out.lines()[0]).unwrap();
    assert!(!hello.window, "a session with no window says so");
    assert!(!out.lines()[0].contains("window"), "and keeps the hello of older apps");
}

#[test]
fn a_pause_between_steps_does_not_move_the_apps_clock_past_the_settle() {
    let (mut ui, _) = window();
    let out = Out::default();
    // The input stays open: a tool between two steps, slowed down or paused.
    let (reader, mut writer) = std::io::pipe().unwrap();
    let mut driver = Headed::over(Hello::of("Tela"), std::io::BufReader::new(reader), out.clone());
    driver.start(Box::new(|| {}));
    writer.write_all(line(&Command::Click { target: "#criar".into() }).as_bytes()).unwrap();
    turns(&mut driver, &mut ui, &out, 1);
    // Ten and twenty seconds of watching later, the app has only had the settle time.
    let (ten, twenty) = (driver.time(10.0), driver.time(20.0));
    assert_eq!(ten, twenty, "the app's clock stands still while the tool waits");
    assert!(ten < ravenrs_e2e_wire::SETTLE + 0.5, "only the settle time ran: {ten}");
    drop(writer);
}

/// A button that shows a notice for four seconds.
struct Notice {
    toasts: ravenrs_widgets::Toasts,
}

impl View for Notice {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let toasts = self.toasts.clone();
        cx.child(&ravenrs_widgets::ZStack::new((
            Button::new("Salvar")
                .on_click(move || toasts.show("Salvo", "", ravenrs_widgets::Tone::Success))
                .test_id("salvar"),
            ravenrs_widgets::Toaster::new(self.toasts.clone()),
        )))
    }
}

#[test]
fn a_notice_is_still_there_after_a_long_pause_on_screen() {
    let mut ui = Ui::new(Notice { toasts: ravenrs_widgets::Toasts::default() }, Theme::raven_light());
    ui.set_viewport(ravenrs_core::Size::new(640.0, 400.0), 1.0);
    ui.update();
    let out = Out::default();
    let (reader, mut writer) = std::io::pipe().unwrap();
    let mut driver = Headed::over(Hello::of("Tela"), std::io::BufReader::new(reader), out.clone());
    driver.start(Box::new(|| {}));
    writer.write_all(line(&Command::Click { target: "#salvar".into() }).as_bytes()).unwrap();
    turns(&mut driver, &mut ui, &out, 1);
    // The platform draws for thirty seconds of a pause, on the clock the driver gives it.
    for real in [1.0, 5.0, 12.0, 30.0] {
        let clock = driver.time(real);
        ui.set_time(clock);
        ui.update();
    }
    writer
        .write_all(line(&Command::Mention { target: "Salvo".into(), text: "Salvo".into() }).as_bytes())
        .unwrap();
    let started = Instant::now();
    while out.lines().len() < 3 {
        assert!(started.elapsed() < Duration::from_secs(10));
        let now = driver_now(&mut driver);
        driver.drive(&mut ui, now);
        std::thread::sleep(Duration::from_millis(5));
    }
    let reply: Reply = serde_json::from_str(&out.lines()[2]).unwrap();
    assert!(reply.ok, "{reply:?}");
    drop(writer);
}

fn driver_now(driver: &mut Headed) -> f64 {
    driver.time(31.0)
}
