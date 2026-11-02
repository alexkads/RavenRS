//! How an application is put together: modules bring entities, behavior and model changes.

use std::cell::RefCell;
use std::rc::Rc;

use crate::frame::FrameContext;
use crate::model::{Diff, MergedModel, ModelLayer, ModelLayers, check_model, generate_model};
use crate::{EntityInfo, Frame, ModelError, ModelNode, ObjectSpace, ViewController, WindowController};

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
            let needs = module.dependencies();
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
pub struct Application {
    title: String,
    modules: Vec<Box<dyn Module>>,
    entities: Vec<EntityInfo>,
    layers: ModelLayers,
    model: Rc<ModelNode>,
    unusable: Vec<crate::UnusableNode>,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application").field("title", &self.title).finish_non_exhaustive()
    }
}

impl Application {
    /// Starts an application with this title.
    pub fn builder(title: impl Into<String>) -> ApplicationBuilder {
        ApplicationBuilder { title: title.into(), modules: Vec::new(), entities: Vec::new() }
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

    /// Applies the layers again. Frames made after this see the result; views already showing
    /// have to be shown again.
    pub fn refresh_model(&mut self) -> Result<(), ModelError> {
        let MergedModel { root, unusable } = self.layers.merged();
        let problems = check_model(&root);
        if !problems.is_empty() {
            return Err(ModelError(problems.join("; ")));
        }
        self.model = Rc::new(root);
        self.unusable = unusable;
        Ok(())
    }

    /// A frame with new controllers from every module, in order, over this unit of work.
    pub fn new_frame(&self, context: FrameContext, space: Rc<RefCell<dyn ObjectSpace>>) -> Frame {
        let view = self.modules.iter().flat_map(|m| m.view_controllers()).collect();
        let window = self.modules.iter().flat_map(|m| m.window_controllers()).collect();
        let frame = Frame::new(context, space, Some(self.model.clone()), view, window);
        frame.set_entities(Rc::new(self.entities.clone()));
        frame
    }
}

/// Collects what goes in an [`Application`].
pub struct ApplicationBuilder {
    title: String,
    modules: Vec<Box<dyn Module>>,
    entities: Vec<EntityInfo>,
}

impl std::fmt::Debug for ApplicationBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationBuilder").field("title", &self.title).finish_non_exhaustive()
    }
}

impl ApplicationBuilder {
    /// Adds a module.
    pub fn module(mut self, module: impl Module + 'static) -> Self {
        self.modules.push(Box::new(module));
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

    /// Puts it together: orders the modules, generates the model, lets the modules change it,
    /// applies their layers and checks the result. What is wrong comes out here, and not when
    /// someone reaches that screen.
    pub fn build(self) -> Result<Application, ModelError> {
        let modules = in_dependency_order(self.modules)?;
        let mut entities = self.entities;
        for module in &modules {
            entities.extend(module.entities());
        }
        let mut zero = generate_model(&entities)?;
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
        let mut application = Application {
            title: self.title,
            modules,
            entities,
            layers,
            model: Rc::new(ModelNode::default()),
            unusable: Vec::new(),
        };
        application.refresh_model()?;
        Ok(application)
    }
}
