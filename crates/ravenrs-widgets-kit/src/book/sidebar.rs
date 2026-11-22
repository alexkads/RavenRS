//! The stories of `Sidebar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #757: it is no longer written over.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `Sidebar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_collapsible = match args.choice("collapsible") {
            "Fixed" => SidebarCollapse::Fixed,
            "Hidden" => SidebarCollapse::Hidden,
            _ => SidebarCollapse::Icons,
        };
        let arg_label = args.text("label");
        let arg_side = match args.choice("side") {
            "Right" => Side::Right,
            _ => Side::Left,
        };
        let arg_variant = match args.choice("variant") {
            "Floating" => SidebarVariant::Floating,
            "Inset" => SidebarVariant::Inset,
            _ => SidebarVariant::Standard,
        };

        let state = SidebarState::new(args.bool("open"));
        HStack::new((
            {
                let mut it = Sidebar::new((
                    SidebarGroup::new(
                        "Workspace",
                        (
                            SidebarItem::new(IconName::LayoutDashboard, "Overview").selected(true),
                            SidebarItem::new(IconName::Users, "Customers").badge(Badge::new("12")),
                            SidebarItem::new(IconName::Settings, "Settings")
                                .item(SidebarItem::text("General").compact())
                                .item(SidebarItem::text("Billing").compact()),
                        ),
                    ),
                    SidebarGroup::new("Projects", (SidebarSkeleton::new(2),)).collapsible(),
                ))
                .state(state.clone())
                .footer(
                    HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                        .spacing(12.0)
                        .align(Align::Center),
                );
                it = it.collapsible(arg_collapsible.clone());
                if !arg_label.is_empty() {
                    it = it.label(arg_label.clone());
                }
                it = it.side(arg_side.clone());
                it = it.variant(arg_variant.clone());
                it
            },
            VStack::new((SidebarTrigger::new(state), Text::new("Or press Cmd/Ctrl+B.")))
                .spacing(12.0)
                .padding(16.0)
                .grow(1.0),
        ))
        .height(420.0)
        .align(Align::Stretch)
    })
}

impl ComponentBook for Sidebar<(AnyView,)> {
    fn book() -> Meta {
        Meta::new("Navigation/Sidebar", render)
            .summary("Navigation column with sections, items and counts.")
            .arg(ArgType::bool("open", true).description(
                "Open with names; closed, as the collapsible mode leaves it (icons only, or hidden).",
            ))
            .arg(ArgType::choice("collapsible", &["Fixed", "Icons", "Hidden"], "Icons"))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::choice("side", &["Left", "Right"], "Left"))
            .arg(ArgType::choice("variant", &["Standard", "Floating", "Inset"], "Standard"))
            .story("Example", Args::new())
            .story("Collapsed", Args::new().with("open", false))
            .story("Collapsible: Fixed", Args::new().with("collapsible", ArgValue::choice("Fixed")))
            .story("Collapsible: Hidden", Args::new().with("collapsible", ArgValue::choice("Hidden")))
            .story("Side: Right", Args::new().with("side", ArgValue::choice("Right")))
            .story("Variant: Floating", Args::new().with("variant", ArgValue::choice("Floating")))
            .story("Variant: Inset", Args::new().with("variant", ArgValue::choice("Inset")))
            .story_with("Compact, with sections", Args::new(), compact_with_sections)
            .story_with("Items with a state and an action", Args::new(), items_with_state_and_action)
    }
}

/// The sidebar of an application with many pages, as Raven Finanças has it: a brand with its
/// subtitle, a command search, sections with a title, compact items (32 pt), a count and the
/// items kept at the foot.
fn compact_with_sections(_: &Args) -> AnyView {
    let item = |icon: IconName, name: &str| SidebarItem::new(icon, name).compact();
    let content: Vec<AnyView> = vec![
        Box::new(item(IconName::Search, "Search commands").shortcut("⌘K")),
        Box::new(SidebarSection::new("Day to day")),
        Box::new(item(IconName::LayoutDashboard, "Overview")),
        Box::new(item(IconName::List, "Transactions").selected(true)),
        Box::new(item(IconName::CreditCard, "Bills").badge(Badge::new("2").tone(Tone::Danger))),
        Box::new(SidebarSection::new("Reports")),
        Box::new(item(IconName::ChartPie, "Reports")),
    ];
    Box::new(
        Sidebar::new(content)
            .header(SidebarBrand::new(Icon::new(IconName::Wallet), "Raven").subtitle("Family finances"))
            .footer(
                VStack::new((
                    item(IconName::Settings, "Settings"),
                    item(IconName::PanelLeft, "Fold menu").shortcut("⌘B"),
                ))
                .spacing(4.0)
                .align(Align::Stretch),
            )
            .height(460.0),
    )
}

/// Items that say a state with a badge in its tone and icon, and carry an action of their own, as
/// the list of agents of a workspace: a count on the chosen one, a delete button, a group action
/// (#831).
fn items_with_state_and_action(_: &Args) -> AnyView {
    let delete = |name: &str| {
        IconButton::new(IconName::Trash2).label(format!("Delete {name}")).size(ButtonSize::Small)
    };
    Box::new(
        Sidebar::new((
            SidebarGroup::new(
                "Workspaces",
                (
                    SidebarItem::new(IconName::Layers, "shop-api")
                        .selected(true)
                        .count("5")
                        .action(delete("shop-api")),
                    SidebarItem::new(IconName::Layers, "website").action(delete("website")),
                ),
            )
            .action(IconButton::new(IconName::Plus).label("New workspace").size(ButtonSize::Small)),
            SidebarGroup::new(
                "Agents",
                (
                    SidebarItem::new(IconName::Sparkles, "Planner")
                        .badge(Badge::new("running").tone(Tone::Warning).icon(IconName::LoaderCircle)),
                    SidebarItem::new(IconName::Code, "Builder")
                        .badge(Badge::new("ready").tone(Tone::Success).icon(IconName::Check)),
                    SidebarItem::new(IconName::Terminal, "Tests")
                        .badge(Badge::new("failed").tone(Tone::Danger).icon(IconName::X)),
                ),
            ),
        ))
        .height(420.0),
    )
}
