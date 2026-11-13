//! The file chooser service and the files it brings, with no view.

use std::fmt;
use std::rc::Rc;

/// A file brought into the app, whole, in memory.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PickedFile {
    /// Its name, with no folder. Never trust it as a path.
    pub name: String,
    /// Its size on disk. A file bigger than the request allowed comes with no bytes.
    pub size: u64,
    /// Its content.
    pub bytes: Rc<[u8]>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PickedFile {
    /// A file with this name and content.
    pub fn new(name: impl Into<String>, bytes: impl Into<Rc<[u8]>>) -> Self {
        let bytes = bytes.into();
        Self { name: name.into(), size: bytes.len() as u64, bytes }
    }

    /// The part of the name after the last dot, in lower case.
    pub fn extension(&self) -> Option<String> {
        self.name.rsplit_once('.').map(|(_, extension)| extension.to_lowercase())
    }
}

impl fmt::Debug for PickedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PickedFile").field("name", &self.name).field("size", &self.size).finish()
    }
}

/// What to ask of a [`FilePicker`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FileRequest {
    /// The extensions offered, such as `pdf`; empty offers every file.
    pub extensions: Vec<String>,
    /// Whether more than one can be chosen.
    pub multiple: bool,
    /// Files above this size are not read, and come with no bytes.
    pub most_bytes: Option<u64>,
}

/// Asks the person for files. Platform layers install one with [`crate::Ui::set_file_picker`].
pub trait FilePickerProvider {
    /// Shows the system's chooser, and calls `done` with what was chosen, now or later
    /// on the UI thread. Canceling gives an empty list.
    fn pick(&self, request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>);

    /// Hands a file over to the person: the system's dialog on a desktop, a download in a
    /// browser. Nothing happens where there is no system to ask.
    fn save(&self, _name: &str, _bytes: &[u8]) {}
}

/// A picker that chooses nothing, for where there is no system to ask (tests, headless use).
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NoFilePicker;

impl FilePickerProvider for NoFilePicker {
    fn pick(&self, _request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>) {
        done(Vec::new());
    }
}

/// A shared handle to the file chooser, from [`crate::BuildCx::file_picker`].
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FilePicker(Rc<dyn FilePickerProvider>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FilePicker {
    /// Wraps a provider.
    pub fn new(provider: impl FilePickerProvider + 'static) -> Self {
        Self(Rc::new(provider))
    }

    /// Asks for files; `done` runs with what was chosen.
    pub fn pick(&self, request: &FileRequest, done: impl FnOnce(Vec<PickedFile>) + 'static) {
        self.0.pick(request, Box::new(done));
    }

    /// Hands a file over to the person.
    pub fn save(&self, name: &str, bytes: &[u8]) {
        self.0.save(name, bytes);
    }
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new(NoFilePicker)
    }
}

impl fmt::Debug for FilePicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilePicker")
    }
}
