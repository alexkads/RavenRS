//! Part of the family `ravenrs-widgets-base`, group "Básicos": the controls a person operates, one
//! page per subject, every public feature of each component in it shown.

pub mod botoes;
pub mod campos_texto;
pub mod cores;
pub mod datas;
pub mod escolhas;
pub mod ligacoes;
pub mod listas_escolha;
pub mod numeros;

use ravenrs::prelude::IconName;

use super::super::Page;

/// The group of the navigation.
pub const GROUP: &str = super::GROUP;

/// The pages of this part, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: botoes::ID,
        caption: botoes::PAGE,
        group: GROUP,
        icon: IconName::MousePointer,
        make: || Box::new(botoes::ButtonsPage),
    },
    Page {
        id: escolhas::ID,
        caption: escolhas::PAGE,
        group: GROUP,
        icon: IconName::CircleCheck,
        make: || Box::new(escolhas::ChoicesPage),
    },
    Page {
        id: listas_escolha::ID,
        caption: listas_escolha::PAGE,
        group: GROUP,
        icon: IconName::ChevronDown,
        make: || Box::new(listas_escolha::PickListsPage),
    },
    Page {
        id: campos_texto::ID,
        caption: campos_texto::PAGE,
        group: GROUP,
        icon: IconName::TextCursor,
        make: || Box::new(campos_texto::TextFieldsPage),
    },
    Page {
        id: numeros::ID,
        caption: numeros::PAGE,
        group: GROUP,
        icon: IconName::SlidersHorizontal,
        make: || Box::new(numeros::NumbersPage),
    },
    Page {
        id: datas::ID,
        caption: datas::PAGE,
        group: GROUP,
        icon: IconName::CalendarDays,
        make: || Box::new(datas::DatesPage),
    },
    Page {
        id: cores::ID,
        caption: cores::PAGE,
        group: GROUP,
        icon: IconName::Palette,
        make: || Box::new(cores::ColorsPage),
    },
    Page {
        id: ligacoes::ID,
        caption: ligacoes::PAGE,
        group: GROUP,
        icon: IconName::Link,
        make: || Box::new(ligacoes::LinksPage),
    },
];
