//! How an application is put together: modules bring entities, behavior and model changes.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::frame::FrameContext;
use crate::manifest::{GuardedObjectSpace, ModuleManifest, Target};
use crate::model::{Diff, MergedModel, ModelLayer, ModelLayers, check_model, generate_model};
use crate::{EntityInfo, Frame, ModelError, ModelNode, ObjectSpace, ViewController, WindowController};

/// The layer that holds the model's words in the language picked.
const LANGUAGE_LAYER: &str = "Language";

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

/// A module switched off: its entities stay, to be read and exported, with every field read only;
/// its controllers, actions and initial records go.
struct SwitchedOff(Box<dyn Module>);

impl Module for SwitchedOff {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn dependencies(&self) -> Vec<String> {
        self.0.dependencies()
    }

    fn entities(&self) -> Vec<EntityInfo> {
        self.0
            .entities()
            .into_iter()
            .map(|mut entity| {
                for field in &mut entity.fields {
                    field.read_only = true;
                }
                entity
            })
            .collect()
    }

    fn extend_model(&self, root: &mut ModelNode) {
        self.0.extend_model(root);
    }

    fn update_generated(&self, root: &mut ModelNode) {
        self.0.update_generated(root);
    }

    fn model_difference(&self) -> Option<&'static str> {
        self.0.model_difference()
    }

    fn manifest(&self) -> ModuleManifest {
        self.0.manifest()
    }
}

/// Refuses what the modules say they cannot work with, naming the module.
fn check_manifests(modules: &[Box<dyn Module>], host: &Host) -> Result<(), ModelError> {
    let mut migrations: Vec<(String, String)> = Vec::new();
    let mut jobs: Vec<(String, String)> = Vec::new();
    for module in modules {
        let name = module.name();
        let manifest = module.manifest();
        for (other, range) in &manifest.requires {
            let found = modules
                .iter()
                .find(|m| m.name() == other)
                .ok_or_else(|| ModelError(format!("the module {name} needs {other}, which was not added")))?;
            match found.manifest().version {
                Some(version) if range.accepts(version) => {}
                Some(version) => {
                    return Err(ModelError(format!(
                        "the module {name} needs {other} {range}, and {other} is {version}"
                    )));
                }
                None => {
                    return Err(ModelError(format!(
                        "the module {name} needs {other} {range}, and {other} says no version"
                    )));
                }
            }
        }
        if let Some(target) = host.target
            && !manifest.targets.is_empty()
            && !manifest.targets.contains(&target)
        {
            return Err(ModelError(format!("the module {name} does not run on {target:?}")));
        }
        if let Some(database) = &host.database
            && !manifest.databases.is_empty()
            && !manifest.databases.contains(database)
        {
            return Err(ModelError(format!("the module {name} does not work on {database}")));
        }
        if let Some(missing) = manifest.capabilities.iter().find(|c| !host.capabilities.contains(c)) {
            return Err(ModelError(format!(
                "the module {name} needs the capability {missing}, which the host does not offer"
            )));
        }
        for (kind, ids, seen) in
            [("migration", &manifest.migrations, &mut migrations), ("job", &manifest.jobs, &mut jobs)]
        {
            for id in ids {
                if let Some((_, first)) = seen.iter().find(|(other, _)| other == id) {
                    return Err(ModelError(format!("the {kind} {id} comes from both {first} and {name}")));
                }
                seen.push((id.clone(), name.to_owned()));
            }
        }
    }
    Ok(())
}

/// What the host says about where the application runs.
#[derive(Debug, Default)]
struct Host {
    target: Option<Target>,
    database: Option<String>,
    capabilities: Vec<String>,
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

/// Puts modules in an order where each comes after what it depends on, keeping the order they
/// were added in as far as possible.
fn in_dependency_order(modules: Vec<Box<dyn Module>>) -> Result<Vec<Box<dyn Module>>, ModelError> {
    let names: Vec<String> = modules.iter().map(|m| m.name().to_owned()).collect();
    for (i, name) in names.iter().enumerate() {
        if names[..i].contains(name) {
            return Err(ModelError(format!("the module {name} was added twice")));
        }
    }
    let mut waiting: Vec<Option<Box<dyn Module>>> = modules.into_iter().map(Some).collect();
    let mut placed: Vec<String> = Vec::new();
    let mut ordered = Vec::with_capacity(waiting.len());
    while ordered.len() < names.len() {
        let mut progressed = false;
        for slot in &mut waiting {
            let Some(module) = slot.as_ref() else { continue };
            let mut needs = module.dependencies();
            needs.extend(
                module
                    .manifest()
                    .requires
                    .into_iter()
                    .map(|(name, _)| name)
                    .filter(|n| !module.dependencies().contains(n)),
            );
            if let Some(missing) = needs.iter().find(|d| !names.contains(d)) {
                return Err(ModelError(format!(
                    "the module {} depends on {missing}, which was not added",
                    module.name()
                )));
            }
            if needs.iter().all(|d| placed.contains(d)) {
                placed.push(module.name().to_owned());
                if let Some(module) = slot.take() {
                    ordered.push(module);
                }
                progressed = true;
                break;
            }
        }
        if !progressed {
            let stuck: Vec<&str> = waiting.iter().flatten().map(|m| m.name()).collect();
            return Err(ModelError(format!("these modules depend on each other: {}", stuck.join(", "))));
        }
    }
    Ok(ordered)
}

/// An application put together: its entities, its model and what makes frames. Layers from
/// the administrator and the user go on with [`layers_mut`](Self::layers_mut).
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Application {
    title: String,
    modules: Vec<Box<dyn Module>>,
    /// As they were declared.
    declared: Vec<EntityInfo>,
    /// As the model has them now.
    entities: Vec<EntityInfo>,
    services: Vec<Rc<dyn Any>>,
    translations: Vec<(String, Diff)>,
    layers: ModelLayers,
    model: Rc<ModelNode>,
    unusable: Vec<crate::UnusableNode>,
    security: Option<Rc<crate::SecurityContext>>,
    switched_off: Vec<String>,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Application {
    /// What its modules want everyone to know, in the order of the modules.
    pub fn notices(&self) -> Vec<Notice> {
        self.modules.iter().flat_map(|module| module.notices()).collect()
    }

    /// Starts an application with this title.
    pub fn builder(title: impl Into<String>) -> ApplicationBuilder {
        ApplicationBuilder {
            title: title.into(),
            modules: Vec::new(),
            entities: Vec::new(),
            dashboards: Vec::new(),
            services: Vec::new(),
            translations: Vec::new(),
            host: Host::default(),
            off: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Application {
    /// What each module says about itself, by name, for whoever lists what an application has.
    pub fn manifests(&self) -> Vec<(String, ModuleManifest)> {
        self.modules.iter().map(|m| (m.name().to_owned(), m.manifest())).collect()
    }

    /// A unit of work where the entities of switched-off modules can be read and not changed.
    pub fn guard(&self, space: Box<dyn ObjectSpace>) -> GuardedObjectSpace {
        GuardedObjectSpace::new(space, self.switched_off.clone())
    }

    /// The entities of switched-off modules.
    pub fn switched_off_entities(&self) -> &[String] {
        &self.switched_off
    }

    /// Its title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Every entity it knows.
    pub fn entities(&self) -> &[EntityInfo] {
        &self.entities
    }

    /// The model as it stands.
    pub fn model(&self) -> Rc<ModelNode> {
        self.model.clone()
    }

    /// What in the layers has nowhere to go.
    pub fn unusable(&self) -> &[crate::UnusableNode] {
        &self.unusable
    }

    /// Its layers. Call [`refresh_model`](Self::refresh_model) after changing them.
    pub fn layers_mut(&mut self) -> &mut ModelLayers {
        &mut self.layers
    }

    /// Shows the model in this language, from the translation given for it; one that was not
    /// given brings back what was declared.
    pub fn set_language(&mut self, language: &str) -> Result<(), ModelError> {
        let mut difference = Diff::new();
        for (_, part) in self.translations.iter().filter(|(name, _)| name == language) {
            difference.merge(part);
        }
        self.layers.put(&ModelLayer::Module(LANGUAGE_LAYER.to_owned()), difference);
        self.refresh_model()
    }

    /// What people see in the model, for translating outside the code: a line per caption, as CSV
    /// with its path, the caption and an empty column for the translation.
    pub fn export_captions(&self) -> String {
        let mut lines = vec!["path,caption,translation".to_owned()];
        crate::model::captions_of(&self.model, &self.entities, &mut lines);
        lines.join("\n") + "\n"
    }

    /// Takes what [`export_captions`](Self::export_captions) gave, with its last column filled in,
    /// as the model in this language. Lines left empty stay as they were. What points to nothing
    /// shows in [`unusable`](Self::unusable) once the language is picked.
    pub fn import_captions(&mut self, language: &str, csv: &str) -> Result<(), ModelError> {
        let difference = crate::model::translation_of(csv)?;
        self.translations.retain(|(name, _)| name != language);
        self.translations.push((language.to_owned(), difference));
        Ok(())
    }

    /// Puts in place the records every module needs, and keeps them.
    pub fn run_initial_records(&self, space: &mut dyn ObjectSpace) -> Result<(), String> {
        for module in &self.modules {
            module.initial_records(space)?;
        }
        space.commit().map_err(|e| e.to_string())
    }

    /// Keeps a setting typed as text in the person's own layer, and applies it. It stays what it
    /// was: a number has to be typed as a number, and yes or no as `true` or `false`.
    pub fn set_typed_setting(&mut self, path: &[&str], name: &str, text: &str) -> Result<(), ModelError> {
        let current = self.model.at(path).and_then(|node| node.properties.get(name));
        let value = match current {
            Some(serde_json::Value::String(_)) | None => serde_json::Value::String(text.to_owned()),
            Some(kept) => match serde_json::from_str::<serde_json::Value>(text) {
                Ok(value) if std::mem::discriminant(&value) == std::mem::discriminant(kept) => value,
                _ => return Err(ModelError(format!("{name} cannot be {text}"))),
            },
        };
        self.layers.set(&ModelLayer::User, path, name, value);
        self.refresh_model()
    }

    /// Throws away the person's own layer of the model.
    pub fn clear_user_settings(&mut self) -> Result<(), ModelError> {
        self.layers.put(&ModelLayer::User, Diff::new());
        self.refresh_model()
    }

    /// Keeps a setting in the person's own layer of the model, and applies it.
    pub fn set_user_setting(&mut self, path: &[&str], name: &str, value: &str) -> Result<(), ModelError> {
        self.layers.set(&ModelLayer::User, path, name, serde_json::Value::String(value.to_owned()));
        self.refresh_model()
    }

    /// Applies the layers again. Frames made after this see the result; views already showing
    /// have to be shown again.
    pub fn refresh_model(&mut self) -> Result<(), ModelError> {
        let MergedModel { mut root, unusable } = self.layers.merged();
        let problems = check_model(&root);
        if !problems.is_empty() {
            return Err(ModelError(problems.join("; ")));
        }
        crate::model::add_custom_fields(&mut root, &self.declared);
        crate::model::follow_captions(&mut root, &self.declared);
        self.entities =
            self.declared.iter().map(|entity| entity.as_in_model(&root)).collect::<Result<_, _>>()?;
        self.model = Rc::new(root);
        self.unusable = unusable;
        Ok(())
    }

    /// What every frame made from now on goes by, for who is signed in.
    pub fn set_security(&mut self, security: Rc<crate::SecurityContext>) {
        self.security = Some(security);
    }

    /// A frame with new controllers from every module, in order, over this unit of work.
    pub fn new_frame(&self, context: FrameContext, space: Rc<RefCell<dyn ObjectSpace>>) -> Frame {
        let view = self.modules.iter().flat_map(|m| m.view_controllers()).collect();
        let window = self.modules.iter().flat_map(|m| m.window_controllers()).collect();
        let frame = Frame::new(context, space, Some(self.model.clone()), view, window);
        for service in &self.services {
            frame.keep_extension(service.clone());
        }
        frame.set_entities(Rc::new(self.entities.clone()));
        if let Some(security) = &self.security {
            frame.set_security(security.clone());
        }
        frame
    }
}

/// Collects what goes in an [`Application`].
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ApplicationBuilder {
    title: String,
    modules: Vec<Box<dyn Module>>,
    entities: Vec<EntityInfo>,
    dashboards: Vec<DashboardInfo>,
    services: Vec<Rc<dyn Any>>,
    translations: Vec<(String, String)>,
    host: Host,
    off: Vec<String>,
}

impl std::fmt::Debug for ApplicationBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationBuilder").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ApplicationBuilder {
    /// Adds a module.
    pub fn module(mut self, module: impl Module + 'static) -> Self {
        self.modules.push(Box::new(module));
        self
    }

    /// Says where the application runs, so that modules that do not run there are refused.
    pub fn target(mut self, target: Target) -> Self {
        self.host.target = Some(target);
        self
    }

    /// Says the storage it works on, such as `sqlite`, so that modules that do not work there are
    /// refused.
    pub fn database(mut self, database: impl Into<String>) -> Self {
        self.host.database = Some(database.into());
        self
    }

    /// Says the host offers this capability of a provider, such as `events.status`.
    pub fn capability(mut self, capability: impl Into<String>) -> Self {
        self.host.capabilities.push(capability.into());
        self
    }

    /// Switches a module off by its name: its records can still be read and exported, nothing of
    /// it can be changed through [`Application::guard`], and its controllers and actions are gone.
    /// It grants nothing and deletes nothing.
    pub fn switch_off(mut self, module: impl Into<String>) -> Self {
        self.off.push(module.into());
        self
    }

    /// Adds an entity outside any module.
    pub fn entity(mut self, entity: EntityInfo) -> Self {
        self.entities.push(entity);
        self
    }

    /// Adds an entity declared as a Rust struct.
    pub fn entity_of<T: crate::BusinessObject>(self) -> Self {
        self.entity(T::entity_info())
    }

    /// Adds a dashboard.
    pub fn dashboard(mut self, dashboard: DashboardInfo) -> Self {
        self.dashboards.push(dashboard);
        self
    }

    /// Gives actions and controllers something the application has, which they ask for by its
    /// type with [`ActionContext::service`](crate::ActionContext::service).
    pub fn service<T: 'static>(mut self, service: T) -> Self {
        self.services.push(Rc::new(service));
        self
    }

    /// The model in another language, as a difference in JSON.
    /// [`Application::set_language`] picks it.
    pub fn translation(mut self, language: impl Into<String>, difference: impl Into<String>) -> Self {
        self.translations.push((language.into(), difference.into()));
        self
    }

    /// Puts it together: orders the modules, generates the model, lets the modules change it,
    /// applies their layers and checks the result. What is wrong comes out here, and not when
    /// someone reaches that screen.
    pub fn build(self) -> Result<Application, ModelError> {
        if let Some(unknown) =
            self.off.iter().find(|name| !self.modules.iter().any(|m| m.name() == name.as_str()))
        {
            return Err(ModelError(format!("the module {unknown} cannot be switched off: it was not added")));
        }
        let modules: Vec<Box<dyn Module>> = self
            .modules
            .into_iter()
            .map(|module| {
                if self.off.iter().any(|name| name == module.name()) {
                    Box::new(SwitchedOff(module)) as Box<dyn Module>
                } else {
                    module
                }
            })
            .collect();
        let mut modules = in_dependency_order(modules)?;
        check_manifests(&modules, &self.host)?;
        let switched_off: Vec<String> = modules
            .iter()
            .filter(|m| self.off.iter().any(|name| name == m.name()))
            .flat_map(|m| m.entities())
            .map(|entity| entity.name)
            .collect();
        let mut entities = self.entities;
        for module in &modules {
            entities.extend(module.entities());
        }
        let with_states: Vec<EntityInfo> =
            entities.iter().filter(|entity| !entity.state_machines.is_empty()).cloned().collect();
        if !with_states.is_empty() {
            modules.push(Box::new(crate::workflow::StateMachineModule(with_states)));
        }
        let with_actions: Vec<EntityInfo> =
            entities.iter().filter(|entity| !entity.actions.is_empty()).cloned().collect();
        if !with_actions.is_empty() {
            modules.push(Box::new(crate::record_action::RecordActionModule(with_actions)));
        }
        let mut zero = generate_model(&entities)?;
        for dashboard in &self.dashboards {
            crate::model::add_dashboard(&mut zero, dashboard);
        }
        for module in &modules {
            module.extend_model(&mut zero);
        }
        for module in &modules {
            module.update_generated(&mut zero);
        }
        let mut seen: Vec<String> = Vec::new();
        for module in &modules {
            let mut actions: Vec<crate::Action> =
                module.view_controllers().iter().flat_map(|c| c.actions()).collect();
            actions.extend(module.window_controllers().iter().flat_map(|c| c.actions()));
            for action in actions {
                action.check()?;
                if seen.iter().any(|id| id == action.id()) {
                    return Err(ModelError(format!("two actions have the id {}", action.id())));
                }
                seen.push(action.id().to_owned());
            }
        }
        let mut layers = ModelLayers::new(zero);
        for module in &modules {
            if let Some(text) = module.model_difference() {
                let difference = Diff::parse(text)
                    .map_err(|e| ModelError(format!("the module {}: {e}", module.name())))?;
                layers.put(&ModelLayer::Module(module.name().to_owned()), difference);
            }
        }
        // Over the other modules, whatever comes in, and under the person's own settings.
        layers.put(&ModelLayer::Module(LANGUAGE_LAYER.to_owned()), Diff::new());
        let translations = self
            .translations
            .into_iter()
            .map(|(language, text)| {
                Diff::parse(&text)
                    .map(|difference| (language.clone(), difference))
                    .map_err(|e| ModelError(format!("the translation {language}: {e}")))
            })
            .collect::<Result<_, _>>()?;
        let mut application = Application {
            title: self.title,
            modules,
            declared: entities.clone(),
            entities,
            services: self.services,
            translations,
            layers,
            model: Rc::new(ModelNode::default()),
            unusable: Vec::new(),
            security: None,
            switched_off,
        };
        application.refresh_model()?;
        Ok(application)
    }
}
