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

/// The system's dialog for choosing a file to open.
///
/// It blocks until answered and has to run on the UI thread, which is where
/// event handlers such as `on_click` run.
///
/// ```no_run
/// use ravenrs_platform_native::OpenDialog;
///
/// let chosen = OpenDialog::new().title("Abrir").filter("Texto", &["txt"]).ask();
/// if let Some(path) = chosen {
///     let text = std::fs::read_to_string(path)?;
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct OpenDialog {
    dialog: rfd::FileDialog,
}

impl OpenDialog {
    /// A dialog that shows every file, until a [`filter`](Self::filter) narrows it.
    pub fn new() -> Self {
        Self::default()
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
        self.dialog.pick_file()
    }
}

/// The largest file a drop reads into memory: 1 GiB.
pub(crate) const MOST_DROPPED_BYTES: u64 = 1024 * 1024 * 1024;

/// Reads what the system's dialogs and drops bring. A file above `most_bytes` keeps its
/// name and size, with no bytes, so the place that asked can say why it refused it.
pub(crate) fn read_picked(path: &Path, most_bytes: Option<u64>) -> Option<ravenrs_widgets::PickedFile> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let size = std::fs::metadata(path).ok()?.len();
    if most_bytes.is_some_and(|most| size > most) {
        return Some(ravenrs_widgets::PickedFile { name, size, bytes: Vec::new().into() });
    }
    match std::fs::read(path) {
        Ok(bytes) => Some(ravenrs_widgets::PickedFile::new(name, bytes)),
        Err(e) => {
            tracing::warn!(error = %e, "could not read the chosen file");
            None
        }
    }
}

/// The system's dialog, as what views ask for files through.
#[derive(Debug, Default)]
pub(crate) struct SystemFilePicker;

impl ravenrs_widgets::FilePickerProvider for SystemFilePicker {
    fn pick(
        &self,
        request: &ravenrs_widgets::FileRequest,
        done: Box<dyn FnOnce(Vec<ravenrs_widgets::PickedFile>)>,
    ) {
        let mut dialog = rfd::FileDialog::new();
        if !request.extensions.is_empty() {
            let kinds: Vec<&str> = request.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(kinds.join(", "), &kinds);
        }
        let paths = if request.multiple {
            dialog.pick_files().unwrap_or_default()
        } else {
            dialog.pick_file().into_iter().collect()
        };
        done(paths.iter().filter_map(|p| read_picked(p, request.most_bytes)).collect());
    }

    fn save(&self, name: &str, bytes: &[u8]) {
        let Some(path) = SaveDialog::new(name).ask() else { return };
        if let Err(e) = std::fs::write(path, bytes) {
            tracing::warn!(error = %e, "could not save the file");
        }
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

/// Opens a web page in the browser. Only `http` and `https` are accepted, so a
/// bad URL cannot end up running a program or opening a file instead.
///
/// On Windows this does not go through `cmd`, as [`open_with_system`] does: `cmd`
/// would take the `&` between a query's parameters as the start of a new command.
pub fn open_url(url: &str) -> io::Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a web address"));
    }
    if cfg!(target_os = "windows") {
        Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn().map(|_| ())
    } else {
        open_with_system(Path::new(url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_addresses_are_opened() {
        for url in ["file:///etc/passwd", "/bin/sh", "--help", "javascript:alert(1)", ""] {
            assert_eq!(open_url(url).map_err(|e| e.kind()), Err(io::ErrorKind::InvalidInput), "{url}");
        }
    }
}
