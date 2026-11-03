//! What the window shows from its first frame: the splash screen, while the
//! database opens on another thread, and then the app.

use std::cell::{Cell, OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use ravenrs::prelude::*;

use super::App;
use super::store::FillColumn;
use crate::db::Database;
use crate::update::Updater;
use crate::{backup, seed};

/// How long the splash stays at least, in seconds, so that a fast computer shows
/// a name to read and not a flash.
const SHORTEST_SECONDS: f64 = 1.2;
const LOGO_SIZE: f32 = 96.0;
const LOGO_CORNER: f32 = 22.0;

/// What the thread that opens the database has to say.
enum Step {
    Status(&'static str),
    Ready,
    Failed(String),
}

/// The window's root: the splash screen over the app that comes after it.
pub struct Opening {
    path: PathBuf,
    today: Date,
    updater: Updater,
    diagnostics: Diagnostics,
    logo: Option<ImageData>,
    visible: State<bool>,
    status: Cell<&'static str>,
    began: Cell<Option<f64>>,
    steps: RefCell<Option<Receiver<Step>>>,
    app: OnceCell<Rc<App>>,
    failure: RefCell<Option<String>>,
}

impl std::fmt::Debug for Opening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opening").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Opening {
    /// Opens the database at `path` once the window is up. `diagnostics` is the
    /// window's panel, which the app reaches from Ajustes.
    pub fn new(path: PathBuf, today: Date, updater: Updater, diagnostics: Diagnostics) -> Self {
        Self {
            path,
            today,
            updater,
            diagnostics,
            logo: crate::icon(),
            visible: State::new(true),
            status: Cell::new("Abrindo seus dados"),
            began: Cell::new(None),
            steps: RefCell::new(None),
            app: OnceCell::new(),
            failure: RefCell::new(None),
        }
    }

    fn start(&self, waker: UiWaker) {
        let (send, receive) = channel();
        *self.steps.borrow_mut() = Some(receive);
        let (path, today) = (self.path.clone(), self.today);
        std::thread::spawn(move || {
            // Declared first to drop last: the UI must find the channel closed
            // when it wakes, even after a panic.
            let _wake_when_done = WakeOnDrop(waker.clone());
            let send = send;
            let step = match prepare(&path, today, &send, &waker) {
                Ok(()) => Step::Ready,
                Err(reason) => Step::Failed(reason),
            };
            let _ = send.send(step);
        });
    }

    /// Reads what the thread said, and builds the app when it finished.
    fn read_steps(&self) {
        let mut steps = self.steps.borrow_mut();
        let Some(receive) = steps.as_ref() else {
            return;
        };
        let finished = loop {
            match receive.try_recv() {
                Ok(Step::Status(status)) => self.status.set(status),
                Ok(Step::Ready) => break Some(self.enter()),
                Ok(Step::Failed(reason)) => break Some(Err(reason)),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => {
                    break Some(Err("A abertura do programa parou antes de terminar.".to_owned()));
                }
            }
        };
        if let Some(finished) = finished {
            *steps = None;
            if let Err(reason) = finished {
                tracing::error!(error = %reason, path = %self.path.display(), "could not open the database");
                *self.failure.borrow_mut() = Some(reason);
            }
        }
    }

    /// The database is ready by now, so opening it here costs the UI no wait.
    fn enter(&self) -> Result<(), String> {
        self.status.set("Preparando as telas");
        let db = Database::open(&self.path).map_err(|e| e.to_string())?;
        self.updater.watch();
        let app = App::with_diagnostics(
            db,
            self.today,
            Some(self.path.clone()),
            self.updater.clone(),
            self.diagnostics.clone(),
        );
        let _ = self.app.set(Rc::new(app));
        Ok(())
    }
}

/// Everything slow about opening. The connection stays on this thread and closes with it.
fn prepare(path: &Path, today: Date, send: &Sender<Step>, waker: &UiWaker) -> Result<(), String> {
    let say = |status| {
        let _ = send.send(Step::Status(status));
        waker.wake();
    };
    say("Abrindo seus dados");
    let db = Database::open(path).map_err(|e| e.to_string())?;
    // The first run opens on a family's sample data, so there is something to
    // explore; Settings starts over for real use.
    if db.is_empty().map_err(|e| e.to_string())? {
        say("Preparando os dados de exemplo");
        seed::load_demo(&db, today).map_err(|e| e.to_string())?;
    }
    say("Guardando a cópia do dia");
    if let Err(e) = backup::keep_daily_copy(&db, &backup::copies_folder(path), today) {
        tracing::warn!(error = %e, "could not keep the copy of the day");
    }
    Ok(())
}

struct WakeOnDrop(UiWaker);

impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.wake();
    }
}

/// The app, once it exists, among the window's children.
struct Shown(Rc<App>);

impl View for Shown {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&*self.0)
    }
}

impl View for Opening {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let now = cx.now();
        let began = self.began.get().unwrap_or_else(|| {
            self.began.set(Some(now));
            self.start(cx.ui_waker());
            now
        });
        self.read_steps();

        let failure = self.failure.borrow().clone();
        if failure.is_some() || (self.app.get().is_some() && now - began >= SHORTEST_SECONDS) {
            self.visible.set_if_changed(false);
        } else if self.app.get().is_some() {
            cx.wake_at(began + SHORTEST_SECONDS);
        }

        let mut splash =
            SplashScreen::new(self.visible.binding(), "Raven Finanças").status(self.status.get());
        splash = match &self.logo {
            // The name right below it says the same to a screen reader.
            Some(logo) => splash
                .logo(Image::new(logo.clone()).size(LOGO_SIZE, LOGO_SIZE).corner_radius(LOGO_CORNER).alt("")),
            None => splash.icon(IconName::Wallet),
        };
        let view = FillColumn {
            children: (
                self.app.get().cloned().map(Shown),
                failure.map(|reason| {
                    VStack::new((
                        Alert::new("Não foi possível abrir o banco de dados", reason).tone(Tone::Danger),
                    ))
                    .padding(32.0)
                }),
                // Last, so that it covers whatever the app opens on its first frame.
                splash,
            ),
            spacing: 0.0,
            padding: 0.0.into(),
        };
        cx.child(&view)
    }
}
