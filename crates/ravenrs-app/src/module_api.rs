//! What a module is: the trait every module of an application implements, and what it hands the
//! application (notices, dashboards). The application that puts modules together lives in
//! `module`; modules that implement the trait stand on this, not on it.

use crate::manifest::ModuleManifest;
use crate::{EntityInfo, ModelNode, ObjectSpace, ViewController, WindowController};

/// A part of an application: what it adds, in one place. Modules stack, each over those it
/// depends on.
pub trait Module {
    /// What it is known by.
    fn name(&self) -> &str;

    /// The modules that have to come before it, by name.
    fn dependencies(&self) -> Vec<String> {
        Vec::new()
    }

    /// The entities it declares.
    fn entities(&self) -> Vec<EntityInfo> {
        Vec::new()
    }

    /// New controllers for a frame; each frame has its own.
    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        Vec::new()
    }

    /// New window controllers for a frame.
    fn window_controllers(&self) -> Vec<Box<dyn WindowController>> {
        Vec::new()
    }

    /// Adds nodes and settings to the generated model. What comes from code is not a difference,
    /// so a person's own settings still win over it.
    fn extend_model(&self, _root: &mut ModelNode) {}

    /// Changes what was generated, once every module added its part.
    fn update_generated(&self, _root: &mut ModelNode) {}

    /// Its layer of the model, as JSON.
    fn model_difference(&self) -> Option<&'static str> {
        None
    }

    /// Puts in place the records it needs, when an application opens on its storage. It runs every
    /// time, so it looks before adding. The commit is up to whoever calls it.
    fn initial_records(&self, _space: &mut dyn ObjectSpace) -> Result<(), String> {
        Ok(())
    }

    /// What everyone using the application has to know while this module is in it, such as that it
    /// is experimental. It stays on top of every window and cannot be dismissed.
    fn notices(&self) -> Vec<Notice> {
        Vec::new()
    }

    /// What it says about itself: its version, the versions it accepts of what it depends on,
    /// where it runs and what it needs from the host. Saying nothing is accepted everywhere.
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::default()
    }
}

/// How serious a [`Notice`] is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum NoticeKind {
    /// Good to know.
    Info,
    /// Take care.
    #[default]
    Warning,
    /// Something can go wrong for real.
    Danger,
}

/// Something a module keeps in sight in every window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Notice {
    /// How serious it is.
    pub kind: NoticeKind,
    /// The short version.
    pub title: String,
    /// The rest.
    pub message: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Notice {
    /// A warning.
    pub fn warning(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Warning, title: title.into(), message: message.into() }
    }

    /// Something good to know.
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Info, title: title.into(), message: message.into() }
    }

    /// Something that can go wrong for real.
    pub fn danger(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Danger, title: title.into(), message: message.into() }
    }
}

/// What a piece of a dashboard shows.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum DashboardContent {
    /// A view of the model, by its id.
    View(String),
    /// A control the application registered, by its name.
    Control(String),
}

/// A piece of a dashboard.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DashboardItem {
    /// What it is known by.
    pub id: String,
    /// What people see over it.
    pub caption: String,
    /// What it shows.
    pub content: DashboardContent,
}

/// Other views together. It is bound to no records: each piece keeps its own frame.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DashboardInfo {
    /// Its name, which its view's id starts with.
    pub name: String,
    /// What people see.
    pub caption: String,
    /// The group it goes under in the navigation.
    pub navigation: Option<String>,
    /// The name of its icon.
    pub icon: Option<String>,
    /// Its pieces, in order.
    pub items: Vec<DashboardItem>,
    /// Other dashboards people can change it to: an id, what they see, and the other's name.
    pub variants: Vec<(String, String, String)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DashboardInfo {
    /// A dashboard with nothing in it yet.
    pub fn new(name: impl Into<String>, caption: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            caption: caption.into(),
            navigation: None,
            icon: None,
            items: Vec::new(),
            variants: Vec::new(),
        }
    }

    /// The group it goes under in the navigation.
    pub fn navigation(mut self, group: impl Into<String>) -> Self {
        self.navigation = Some(group.into());
        self
    }

    /// The name of its icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Lets people change it to another dashboard, by that one's name. It takes two for the menu
    /// to show, so a dashboard lists itself as well.
    pub fn variant(
        mut self,
        id: impl Into<String>,
        caption: impl Into<String>,
        dashboard: impl Into<String>,
    ) -> Self {
        self.variants.push((id.into(), caption.into(), dashboard.into()));
        self
    }

    /// Shows a view of the model, by its id.
    pub fn view(mut self, caption: impl Into<String>, view: impl Into<String>) -> Self {
        let view = view.into();
        self.items.push(DashboardItem {
            id: view.clone(),
            caption: caption.into(),
            content: DashboardContent::View(view),
        });
        self
    }

    /// Shows a control the application registered, by its name.
    pub fn control(mut self, caption: impl Into<String>, control: impl Into<String>) -> Self {
        let control = control.into();
        self.items.push(DashboardItem {
            id: control.clone(),
            caption: caption.into(),
            content: DashboardContent::Control(control),
        });
        self
    }
}
