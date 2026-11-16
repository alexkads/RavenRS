//! The Studio's menu beside the clock (#643): how its own test went, a button that runs it
//! again with the window closed, and an update that is ready.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use ravenrs::core::{TrayEntry, TrayMenu};

use crate::{report, runner};

/// The id of "Rodar autoteste".
pub const RUN_SELF_TEST: &str = "rts:self-test";

/// What the window and the icon share: the menu the icon shows, the choices made on it, and
/// the self-test run from it.
#[derive(Clone, Debug, Default)]
pub struct TrayLink {
    menu: TrayMenu,
    chosen: Rc<RefCell<Vec<String>>>,
    run: Arc<Mutex<SelfTest>>,
}

#[derive(Clone, Debug, Default)]
struct SelfTest {
    running: bool,
    /// What the last run from the menu said.
    said: Option<(String, bool)>,
}

impl TrayLink {
    /// A link that sets `menu`; give the same menu to the `SystemTray`.
    pub fn new(menu: TrayMenu) -> Self {
        Self { menu, ..Self::default() }
    }

    /// The menu the icon shows.
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// Keeps a choice of the menu for the window's next build, which acts on it. Give it to
    /// `SystemTray::on_select`.
    pub fn choose(&self, id: &str) {
        self.chosen.borrow_mut().push(id.to_owned());
    }

    /// The choices since the last build.
    pub(crate) fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.chosen.borrow_mut())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SelfTest> {
        self.run.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs the self-test on a thread of its own and wakes the screen when it ends.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn run_self_test(&self, wake: impl Fn() + Send + 'static) {
        {
            let mut run = self.lock();
            if run.running {
                return;
            }
            run.running = true;
        }
        let shared = self.run.clone();
        std::thread::spawn(move || {
            let said = crate::run_bundled("autoteste", None).map_or_else(
                || ("o autoteste não veio com o Studio".to_owned(), false),
                |r| (runner::summary(&r.outcomes, r.total), r.passed()),
            );
            let mut run = shared.lock().unwrap_or_else(PoisonError::into_inner);
            run.running = false;
            run.said = Some(said);
            drop(run);
            wake();
        });
    }

    /// The menu for the state the window is in: the self-test at the top, the button that runs
    /// it, and the update's entries.
    pub(crate) fn entries(&self, health: Option<&report::Report>, update: Vec<TrayEntry>) -> Vec<TrayEntry> {
        let run = self.lock().clone();
        let (title, detail) = match (&run, health) {
            (SelfTest { running: true, .. }, _) => ("Rodando o autoteste…".to_owned(), None),
            (SelfTest { said: Some((said, passed)), .. }, _) => {
                (format!("Autoteste: {said}"), Some(if *passed { "passou" } else { "falhou" }.to_owned()))
            }
            (_, Some(h)) => (
                format!("Autoteste: {}", runner::summary(&h.outcomes, h.total)),
                Some(if h.passed() { "passou ao abrir" } else { "falhou ao abrir" }.to_owned()),
            ),
            (_, None) => ("Autoteste ainda não rodou".to_owned(), None),
        };
        let mut out = vec![
            TrayEntry::status(title, detail),
            TrayEntry::item(RUN_SELF_TEST, "Rodar autoteste").enabled(!run.running),
        ];
        if !update.is_empty() {
            out.push(TrayEntry::Separator);
            out.extend(update);
        }
        out
    }
}
