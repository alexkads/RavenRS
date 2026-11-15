//! The family `ravenrs-widgets-chat`, group "Conversa e código": the conversation and what goes
//! with it, the code editor and the playground, and the text with marks and the floating panel.

pub mod codigo;
pub mod mensagens;
pub mod texto;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Conversa e código";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: mensagens::ID,
        caption: mensagens::PAGE,
        group: GROUP,
        icon: IconName::MessageSquare,
        make: || Box::new(mensagens::ChatPage),
    },
    Page {
        id: codigo::ID,
        caption: codigo::PAGE,
        group: GROUP,
        icon: IconName::Code,
        make: || Box::new(codigo::CodePage),
    },
    Page {
        id: texto::ID,
        caption: texto::PAGE,
        group: GROUP,
        icon: IconName::Type,
        make: || Box::new(texto::RichTextPage),
    },
];
