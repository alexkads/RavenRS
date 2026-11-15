//! The components of `ravenrs-widgets-lists`, grouped by what they are for.

pub mod ano;
pub mod arvore;
pub mod documentos;
pub mod inspetor;
pub mod longas;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Listas e árvores";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: inspetor::ID,
        caption: inspetor::PAGE,
        group: GROUP,
        icon: IconName::Search,
        make: || Box::new(inspetor::InspectorPage),
    },
    Page {
        id: arvore::ID,
        caption: arvore::PAGE,
        group: GROUP,
        icon: IconName::FolderOpen,
        make: || Box::new(arvore::TreeViewPage),
    },
    Page {
        id: longas::ID,
        caption: longas::PAGE,
        group: GROUP,
        icon: IconName::List,
        make: || Box::new(longas::VirtualListPage),
    },
    Page {
        id: documentos::ID,
        caption: documentos::PAGE,
        group: GROUP,
        icon: IconName::FileText,
        make: || Box::new(documentos::DocumentTabsPage),
    },
    Page {
        id: ano::ID,
        caption: ano::PAGE,
        group: GROUP,
        icon: IconName::CalendarDays,
        make: || Box::new(ano::YearViewPage),
    },
];
