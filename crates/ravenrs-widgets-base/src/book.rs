//! The stories of a component, as Storybook keeps them: what [RavenComponentBook] lists, draws,
//! measures and exports.
//!
//! A component's [`Meta`] says where it sits in the book (`"Actions/Button"`), what it is, the
//! settings a person can change on it ([`ArgType`], each with its control and its default) and
//! how it is drawn from them. Its [`Story`]s are named states of it, each a set of [`Args`] over
//! the defaults: Primary, Disabled, With icon. The book draws a story, lets the settings be
//! changed while it is on screen, and measures what RavenRS paints (sizes, corners, colors,
//! fonts), so a design made from it can be built with the library as it is.
//!
//! Every crate of components keeps its stories next to its components, in a `book` module of
//! its own, behind the feature `book`, and lists them with a `stories()` function:
//!
//! ```
//! use ravenrs_widgets_base::book::{ArgType, Args, Meta};
//! use ravenrs_widgets_base::{AnyView, Button, ButtonStyle};
//!
//! let meta = Meta::new("Actions/Button", |args: &Args| -> AnyView {
//!     let style = match args.choice("style") {
//!         "Secondary" => ButtonStyle::Secondary,
//!         _ => ButtonStyle::Primary,
//!     };
//!     Box::new(Button::new(args.text("label")).style(style).disabled(args.bool("disabled")))
//! })
//! .summary("The action a person takes.")
//! .arg(ArgType::text("label", "Save"))
//! .arg(ArgType::choice("style", &["Primary", "Secondary"], "Primary"))
//! .arg(ArgType::bool("disabled", false))
//! .story("Primary", Args::new())
//! .story("Disabled", Args::new().with("disabled", true));
//! assert_eq!(meta.component(), "Button");
//! assert_eq!(meta.args_of(&meta.stories[1]).text("label"), "Save");
//! ```
//!
//! [RavenComponentBook]: https://github.com/alexkads/RavenRS/tree/master/tools/raven-component-book

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::{AnyView, View};

/// What every component of RavenRS gives the book: its [`Meta`], with its settings and its
/// stories, as a Storybook `*.stories` file does. The book lists, draws, measures and exports
/// the components through it, so a component with no `ComponentBook` is missing from the book
/// and from the design system made from it.
///
/// ```
/// use ravenrs_widgets_base::book::{Args, ComponentBook, Meta};
/// use ravenrs_widgets_base::{AnyView, Text};
///
/// struct Greeting;
///
/// impl ravenrs_widgets_base::View for Greeting {
///     fn build(&self, cx: &mut ravenrs_widgets_base::context::BuildCx<'_>) -> ravenrs_widgets_base::context::NodeId {
///         cx.child(&Text::new("Hello"))
///     }
/// }
///
/// impl ComponentBook for Greeting {
///     fn book() -> Meta {
///         Meta::new("Display and feedback/Greeting", |_: &Args| -> AnyView { Box::new(Greeting) })
///             .story("Default", Args::new())
///     }
/// }
/// assert_eq!(Greeting::book().component(), "Greeting");
/// ```
pub trait ComponentBook: View {
    /// Where the component sits in the book, its settings and its stories.
    fn book() -> Meta
    where
        Self: Sized;
}

#[cfg(feature = "book")]
mod button;
#[cfg(feature = "book")]
mod card;
#[cfg(feature = "book")]
mod display;
#[cfg(feature = "book")]
mod inputs;
#[cfg(feature = "book")]
mod select;
#[cfg(feature = "book")]
mod text_field;

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
#[cfg(feature = "book")]
pub fn stories() -> Vec<Meta> {
    use crate::{
        Alert, Avatar, Badge, Button, Card, Checkbox, IconButton, ProgressBar, SegmentedControl, Select,
        Slider, TextField, Toggle,
    };
    vec![
        Button::book(),
        IconButton::book(),
        SegmentedControl::book(),
        TextField::book(),
        Checkbox::book(),
        Toggle::book(),
        Slider::book(),
        Select::book(),
        Card::<AnyView>::book(),
        Badge::book(),
        Alert::book(),
        Avatar::book(),
        ProgressBar::book(),
    ]
}

/// The value of one setting of a story.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgValue {
    /// Words, as a label or a placeholder.
    Text(String),
    /// On or off.
    Bool(bool),
    /// A number, as a value or a size in points.
    Number(f64),
    /// One of the options of [`Control::Choice`], by its name.
    Choice(String),
}

impl ArgValue {
    /// One of the options of a [`Control::Choice`].
    pub fn choice(name: impl Into<String>) -> Self {
        Self::Choice(name.into())
    }

    /// The value as it is written in the book and in its JSON: `"Save"`, `true`, `0.4`.
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) | Self::Choice(s) => s.clone(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.to_string(),
        }
    }
}

impl From<&str> for ArgValue {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for ArgValue {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<bool> for ArgValue {
    fn from(on: bool) -> Self {
        Self::Bool(on)
    }
}

impl From<f64> for ArgValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

/// How the book lets a person change a setting.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// A text field.
    Text,
    /// A switch.
    Bool,
    /// A slider between `min` and `max`, by `step`.
    Number {
        /// The smallest value.
        min: f64,
        /// The largest value.
        max: f64,
        /// How far one step moves.
        step: f64,
    },
    /// A choice among these names, as the variants of an enum.
    Choice(Vec<&'static str>),
}

/// A setting of a component that a story can change: its name, what it does, how it is changed
/// and its value when a story does not say.
#[derive(Clone, Debug, PartialEq)]
pub struct ArgType {
    /// The name, as the builder method it maps to: `label`, `style`, `disabled`.
    pub name: &'static str,
    /// What it changes, in a sentence.
    pub description: &'static str,
    /// How the book changes it.
    pub control: Control,
    /// Its value when a story does not set it.
    pub default: ArgValue,
}

impl ArgType {
    /// Words, changed in a text field.
    pub fn text(name: &'static str, default: &str) -> Self {
        Self { name, description: "", control: Control::Text, default: ArgValue::from(default) }
    }

    /// On or off, changed with a switch.
    pub fn bool(name: &'static str, default: bool) -> Self {
        Self { name, description: "", control: Control::Bool, default: ArgValue::Bool(default) }
    }

    /// A number between `min` and `max`, changed with a slider.
    pub fn number(name: &'static str, default: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            name,
            description: "",
            control: Control::Number { min, max, step },
            default: ArgValue::Number(default),
        }
    }

    /// One of `options`, changed with a select.
    pub fn choice(name: &'static str, options: &[&'static str], default: &str) -> Self {
        Self {
            name,
            description: "",
            control: Control::Choice(options.to_vec()),
            default: ArgValue::choice(default),
        }
    }

    /// Says what the setting changes.
    pub fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }
}

/// The values of the settings of a story, by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args(BTreeMap<String, ArgValue>);

impl Args {
    /// No setting changed.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same with `name` set to `value`.
    pub fn with(mut self, name: &str, value: impl Into<ArgValue>) -> Self {
        self.set(name, value);
        self
    }

    /// Sets `name` to `value`.
    pub fn set(&mut self, name: &str, value: impl Into<ArgValue>) {
        self.0.insert(name.to_owned(), value.into());
    }

    /// The value of `name`, if set.
    pub fn get(&self, name: &str) -> Option<&ArgValue> {
        self.0.get(name)
    }

    /// Every setting, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ArgValue)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// The words of `name`; empty when it is not text.
    pub fn text(&self, name: &str) -> String {
        match self.get(name) {
            Some(ArgValue::Text(s) | ArgValue::Choice(s)) => s.clone(),
            _ => String::new(),
        }
    }

    /// Whether `name` is on; off when it is not a switch.
    pub fn bool(&self, name: &str) -> bool {
        matches!(self.get(name), Some(ArgValue::Bool(true)))
    }

    /// The number of `name`; zero when it is not a number.
    pub fn number(&self, name: &str) -> f64 {
        match self.get(name) {
            Some(ArgValue::Number(n)) => *n,
            _ => 0.0,
        }
    }

    /// The option chosen for `name`; empty when it is not a choice.
    pub fn choice(&self, name: &str) -> &str {
        match self.get(name) {
            Some(ArgValue::Choice(s) | ArgValue::Text(s)) => s,
            _ => "",
        }
    }
}

/// Draws a component from the values of its settings.
pub type Render = Rc<dyn Fn(&Args) -> AnyView>;

/// A named state of a component: the settings it changes over the defaults.
#[derive(Clone)]
pub struct Story {
    /// The name, as the book lists it: Primary, Disabled.
    pub name: &'static str,
    /// What it changes over the defaults of the [`Meta`].
    pub args: Args,
    render: Option<Render>,
}

impl std::fmt::Debug for Story {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Story").field("name", &self.name).field("args", &self.args).finish_non_exhaustive()
    }
}

/// A component in the book: where it sits, what it is, its settings and its stories.
#[derive(Clone)]
pub struct Meta {
    /// Where it sits, family first: `"Actions/Button"`.
    pub title: &'static str,
    /// What it is, in a sentence.
    pub summary: &'static str,
    /// The settings a person can change.
    pub arg_types: Vec<ArgType>,
    /// Its named states.
    pub stories: Vec<Story>,
    render: Render,
}

impl std::fmt::Debug for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Meta")
            .field("title", &self.title)
            .field("arg_types", &self.arg_types)
            .field("stories", &self.stories)
            .finish_non_exhaustive()
    }
}

impl Meta {
    /// A component at `title` (`"Actions/Button"`), drawn by `render` from its settings.
    pub fn new(title: &'static str, render: impl Fn(&Args) -> AnyView + 'static) -> Self {
        Self { title, summary: "", arg_types: Vec::new(), stories: Vec::new(), render: Rc::new(render) }
    }

    /// Says what it is.
    pub fn summary(mut self, summary: &'static str) -> Self {
        self.summary = summary;
        self
    }

    /// Adds a setting.
    pub fn arg(mut self, arg: ArgType) -> Self {
        self.arg_types.push(arg);
        self
    }

    /// Adds a story that changes `args` over the defaults.
    pub fn story(mut self, name: &'static str, args: Args) -> Self {
        self.stories.push(Story { name, args, render: None });
        self
    }

    /// Adds a story drawn its own way, for a state the settings do not reach, as a group of
    /// buttons side by side.
    pub fn story_with(
        mut self,
        name: &'static str,
        args: Args,
        render: impl Fn(&Args) -> AnyView + 'static,
    ) -> Self {
        self.stories.push(Story { name, args, render: Some(Rc::new(render)) });
        self
    }

    /// The component's name: the last part of the title.
    pub fn component(&self) -> &'static str {
        self.title.rsplit('/').next().unwrap_or(self.title)
    }

    /// The family: the title without the component.
    pub fn family(&self) -> &'static str {
        self.title.rsplit_once('/').map_or("", |(family, _)| family)
    }

    /// The settings of `story`: the defaults, with what the story changes.
    pub fn args_of(&self, story: &Story) -> Args {
        let mut args = Args::new();
        for arg in &self.arg_types {
            args.set(arg.name, arg.default.clone());
        }
        for (name, value) in story.args.iter() {
            args.set(name, value.clone());
        }
        args
    }

    /// Draws `story` with `args`.
    pub fn render(&self, story: &Story, args: &Args) -> AnyView {
        (story.render.as_ref().unwrap_or(&self.render))(args)
    }
}
