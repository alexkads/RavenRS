//! Part of the family `ravenrs-widgets-kit`, group "Kit": one page per subject, every public feature of
//! each component in it shown.

pub mod arquivos;
pub mod busca;
pub mod datas;
pub mod filtro;
pub mod formulario;
pub mod passos;
pub mod temas;

use ravenrs::prelude::IconName;

use super::super::Page;

/// The group of the navigation.
pub const GROUP: &str = super::GROUP;

/// The pages of this part, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: formulario::ID,
        caption: formulario::PAGE,
        group: GROUP,
        icon: IconName::Pencil,
        make: || Box::new(formulario::FormFieldsPage),
    },
    Page {
        id: datas::ID,
        caption: datas::PAGE,
        group: GROUP,
        icon: IconName::Calendar,
        make: || Box::new(datas::DatesPage),
    },
    Page {
        id: busca::ID,
        caption: busca::PAGE,
        group: GROUP,
        icon: IconName::Search,
        make: || Box::new(busca::SearchPage),
    },
    Page {
        id: arquivos::ID,
        caption: arquivos::PAGE,
        group: GROUP,
        icon: IconName::Upload,
        make: || Box::new(arquivos::FilesPage),
    },
    Page {
        id: filtro::ID,
        caption: filtro::PAGE,
        group: GROUP,
        icon: IconName::Filter,
        make: || Box::new(filtro::FilterPage),
    },
    Page {
        id: passos::ID,
        caption: passos::PAGE,
        group: GROUP,
        icon: IconName::ListOrdered,
        make: || Box::new(passos::StepsPage),
    },
    Page {
        id: temas::ID,
        caption: temas::PAGE,
        group: GROUP,
        icon: IconName::Palette,
        make: || Box::new(temas::ThemesPage),
    },
];
