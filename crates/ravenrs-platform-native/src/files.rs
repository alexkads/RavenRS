//! Files and the desktop: asking where to save, and opening with the system's program.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The system's dialog for choosing where to save a file. It asks before
/// replacing a file that exists, so the path it returns can be written.
///
/// It blocks until answered and has to run on the UI thread, which is where
/// event handlers such as `on_click` run.
///
/// ```no_run
/// use ravenrs_platform_native::SaveDialog;
///
/// let chosen = SaveDialog::new("Contas.pdf").title("Exportar").filter("PDF", &["pdf"]).ask();
/// if let Some(path) = chosen {
///     std::fs::write(path, b"...")?;
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Debug)]
pub struct SaveDialog {
    dialog: rfd::FileDialog,
}

impl SaveDialog {
    /// A dialog suggesting this file name.
    pub fn new(file_name: &str) -> Self {
        Self { dialog: rfd::FileDialog::new().set_file_name(file_name) }
    }

    /// Sets the dialog's title.
    pub fn title(mut self, title: &str) -> Self {
        self.dialog = self.dialog.set_title(title);
        self
    }

    /// Offers a kind of file, such as `("PDF", &["pdf"])`.
    pub fn filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.dialog = self.dialog.add_filter(name, extensions);
        self
    }

    /// Opens in this folder, when the system allows it.
    pub fn folder(mut self, folder: &Path) -> Self {
        self.dialog = self.dialog.set_directory(folder);
        self
    }

    /// Shows the dialog; `None` when the user cancels.
    pub fn ask(self) -> Option<PathBuf> {
        self.dialog.save_file()
    }
}

/// Opens a file with the program the system uses for it: a PDF in its viewer, a
/// web page in the browser. The program runs on its own, so this does not wait for it.
pub fn open_with_system(path: &Path) -> io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(target_os = "windows") {
        // The empty argument is the window title `start` takes first, when a path has quotes.
        let mut start = Command::new("cmd");
        start.args(["/C", "start", ""]);
        start
    } else {
        Command::new("xdg-open")
    };
    command.arg(path).spawn().map(|_| ())
}
