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
    }
}
