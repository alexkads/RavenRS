//! Records most business applications need, ready to use or to extend: every one is a plain
//! [`EntityInfo`], so adding a field or a rule is a matter of calling it before handing it over.
//!
//! ```
//! use ravenrs_app::{Application, FieldInfo, FieldKind};
//! use ravenrs_app_library::{LibraryModule, person};
//!
//! let application = Application::builder("Contacts")
//!     .module(LibraryModule::new().task().entity(person().field(FieldInfo::new("badge", FieldKind::Text))))
//!     .build()?;
//! // What they point to comes along.
//! assert!(application.entities().iter().any(|entity| entity.name == "Address"));
//! # Ok::<(), ravenrs_app::ModelError>(())
//! ```

use ravenrs_app::{
    Comparison, EntityInfo, EnumOption, FieldInfo, FieldKind, Module, Operand, Rule, RuleKind,
};

/// What an email address looks like, loosely, so it only catches slips.
const EMAIL: &str = r"^[^@\s]+@[^@\s]+\.[^@\s]+$";

/// Their words in Brazilian Portuguese, as a difference for `translation`.
pub const PT_BR: &str = include_str!("pt_br.json");

fn key() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Integer).key()
}

fn text(name: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Text)
}

fn email() -> FieldInfo {
    text("email").caption("Email")
}

fn reference(name: &str, target: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Reference(target.to_owned()))
}

/// A country, which addresses point to.
pub fn country() -> EntityInfo {
    EntityInfo::new("Country")
        .caption("Countries")
        .icon("globe")
        .field(key())
        .field(text("name").default_property())
        .field(text("code").hint("ISO 3166, such as BR"))
        .rule(Rule::required("name"))
        .rule(Rule::unique("name"))
}

/// A postal address.
pub fn address() -> EntityInfo {
    EntityInfo::new("Address")
        .caption("Addresses")
        .icon("map-pin")
        .field(key())
        .field(
            text("full_address")
                .caption("Address")
                .calculated("Concat([street], ', ', [number], ' - ', [city], ' ', [state])")
                .default_property()
                .in_detail(false),
        )
        .field(text("street"))
        .field(text("number"))
        .field(text("complement").in_list(false))
        .field(text("city"))
        .field(text("state"))
        .field(text("postal_code").caption("Postal code").in_list(false))
        .field(reference("country", "Country"))
        .rule(Rule::required("street"))
}

/// A person.
pub fn person() -> EntityInfo {
    EntityInfo::new("Person")
        .caption("People")
        .icon("user")
        .field(key())
        .field(
            text("full_name")
                .caption("Name")
                .calculated("Trim(Concat([first_name], ' ', [last_name]))")
                .default_property()
                .in_detail(false),
        )
        .field(text("first_name").caption("First name").in_list(false))
        .field(text("last_name").caption("Last name").in_list(false))
        .field(email())
        .field(text("phone"))
        .field(FieldInfo::new("birthday", FieldKind::Date).in_list(false))
        .field(reference("address", "Address").in_list(false))
        .rule(Rule::required("first_name"))
        .rule(Rule::matches("email", EMAIL))
}

/// A company or any other organization.
pub fn organization() -> EntityInfo {
    EntityInfo::new("Organization")
        .caption("Organizations")
        .icon("users")
        .field(key())
        .field(text("name").default_property())
        .field(email())
        .field(text("phone"))
        .field(text("website").in_list(false))
        .field(reference("address", "Address").in_list(false))
        .rule(Rule::required("name"))
        .rule(Rule::matches("email", EMAIL))
}

/// Something to do.
pub fn task() -> EntityInfo {
    let status = vec![
        EnumOption::new("not_started", "Not started"),
        EnumOption::new("in_progress", "In progress"),
        EnumOption::new("waiting", "Waiting"),
        EnumOption::new("done", "Done"),
        EnumOption::new("deferred", "Deferred"),
    ];
    EntityInfo::new("Task")
        .caption("Tasks")
        .icon("clipboard")
        .field(key())
        .field(text("subject").default_property())
        .field(FieldInfo::new("status", FieldKind::Enum(status)))
        .field(FieldInfo::new("assigned_to", FieldKind::Reference("Person".into())).caption("Assigned to"))
        .field(FieldInfo::new("due_date", FieldKind::Date).caption("Due date"))
        .field(
            FieldInfo::new("percent_done", FieldKind::Integer)
                .caption("Done")
                .display_format("{} %")
                .default_value(0.0),
        )
        .field(FieldInfo::new("description", FieldKind::LongText))
        .rule(Rule::required("subject"))
        .rule(Rule::range("percent_done", Some(0.0), Some(100.0)))
}

/// Something that happens at a time.
pub fn event() -> EntityInfo {
    EntityInfo::new("Event")
        .caption("Events")
        .icon("calendar")
        .field(key())
        .field(text("subject").default_property())
        .field(FieldInfo::new("starts", FieldKind::DateTime))
        .field(FieldInfo::new("ends", FieldKind::DateTime))
        .field(FieldInfo::new("all_day", FieldKind::Boolean).caption("All day"))
        .field(text("location"))
        .field(FieldInfo::new("description", FieldKind::LongText))
        .rule(Rule::required("subject"))
        .rule(Rule::required("starts"))
        .rule(
            Rule::on(
                "ends",
                RuleKind::Compare {
                    comparison: Comparison::GreaterOrEqual,
                    with: Operand::Field("starts".into()),
                },
            )
            .target_criteria("Not IsNull([starts])"),
        )
}

/// Something written down.
pub fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .caption("Notes")
        .icon("file-text")
        .field(key())
        .field(text("title").default_property())
        .field(FieldInfo::new("text", FieldKind::LongText))
        .rule(Rule::required("title"))
}

/// A file, with a name of its own.
pub fn file_record() -> EntityInfo {
    EntityInfo::new("FileRecord")
        .caption("Files")
        .icon("file")
        .field(key())
        .field(text("name").default_property())
        .field(FieldInfo::new("file", FieldKind::File))
        .rule(Rule::required("file"))
}

/// Brings these records into an application, each with a place in the navigation if given one.
/// What they point to in this library comes along by itself, out of the navigation.
#[derive(Debug, Default)]
pub struct LibraryModule {
    entities: Vec<EntityInfo>,
    navigation: Option<String>,
}

impl LibraryModule {
    /// A module with no records yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts the records named here in this group of the navigation.
    pub fn navigation(mut self, group: impl Into<String>) -> Self {
        self.navigation = Some(group.into());
        self
    }

    /// Brings in this record, as given, which may be one of this library with more on it. A second
    /// by the same name takes the place of the first.
    pub fn entity(mut self, entity: EntityInfo) -> Self {
        self.entities.retain(|known| known.name != entity.name);
        self.entities.push(entity);
        self
    }

    /// Brings in [`person`].
    pub fn person(self) -> Self {
        self.entity(person())
    }

    /// Brings in [`organization`].
    pub fn organization(self) -> Self {
        self.entity(organization())
    }

    /// Brings in [`address`].
    pub fn address(self) -> Self {
        self.entity(address())
    }

    /// Brings in [`country`].
    pub fn country(self) -> Self {
        self.entity(country())
    }

    /// Brings in [`task`].
    pub fn task(self) -> Self {
        self.entity(task())
    }

    /// Brings in [`event`].
    pub fn event(self) -> Self {
        self.entity(event())
    }

    /// Brings in [`note`].
    pub fn note(self) -> Self {
        self.entity(note())
    }

    /// Brings in [`file_record`].
    pub fn file_record(self) -> Self {
        self.entity(file_record())
    }
}

/// A record of this library by its name.
fn known(name: &str) -> Option<EntityInfo> {
    match name {
        "Person" => Some(person()),
        "Organization" => Some(organization()),
        "Address" => Some(address()),
        "Country" => Some(country()),
        _ => None,
    }
}

impl Module for LibraryModule {
    fn name(&self) -> &str {
        "Library"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        let mut all: Vec<EntityInfo> = self
            .entities
            .iter()
            .map(|entity| match (&self.navigation, entity.navigation.is_none()) {
                (Some(group), true) => entity.clone().navigation(group.clone()),
                _ => entity.clone(),
            })
            .collect();
        let mut at = 0;
        while at < all.len() {
            let needed: Vec<String> = all[at]
                .fields
                .iter()
                .filter_map(|field| match &field.kind {
                    FieldKind::Reference(target) => Some(target.clone()),
                    _ => None,
                })
                .collect();
            for name in needed {
                if !all.iter().any(|entity| entity.name == name)
                    && let Some(entity) = known(&name)
                {
                    all.push(entity);
                }
            }
            at += 1;
        }
        all
    }
}
