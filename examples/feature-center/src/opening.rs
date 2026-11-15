//! How the center opens: the whole center, the demonstration of the site, only the components, the
//! playground of the examples, or a single page (#587). The same words come from the command line
//! (`--modo vitrine --pagina X`) and from the address of the page (`?modo=vitrine&pagina=X`).
//!
//! None of them takes an address: they only choose what is on the screen. Where the center reads
//! its records from comes from the build (`RAVEN_API_URL`), never from a link.

/// What the center shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Everything, after signing in, kept in SQLite on the desktop and in memory in the browser.
    #[default]
    Whole,
    /// Everything, in memory, with a person of the demonstration filled in at the door: the site.
    Showcase,
    /// Only the pages of the components, with no sign-in and no application framework.
    Components,
    /// The playground of the examples, to edit and run them.
    Playground,
}

/// The look, by the name the parameter `tema` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// `escuro`
    Dark,
    /// `claro`
    Light,
    /// `vidro`
    Glass,
}

impl Look {
    /// The theme it stands for.
    pub fn theme(self) -> ravenrs::prelude::Theme {
        match self {
            Self::Dark => ravenrs::prelude::Theme::raven_dark(),
            Self::Light => ravenrs::prelude::Theme::raven_light(),
            Self::Glass => ravenrs::prelude::Theme::raven_glass(),
        }
    }
}

/// The language of the screens, by the name the parameter `idioma` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speech {
    /// `pt-BR`
    Portuguese,
    /// `en`
    English,
}

/// Everything the center is told when it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opening {
    /// What it shows.
    pub mode: Mode,
    /// The page it opens on: a view or a dashboard of the center, or a page of a component.
    pub page: Option<String>,
    /// In the playground, the component whose example opens on its own.
    pub component: Option<String>,
    /// Only the page, with no navigation around it, to sit inside another page.
    pub embedded: bool,
    /// The look, instead of the center's own.
    pub look: Option<Look>,
    /// The language, instead of Brazilian Portuguese.
    pub speech: Option<Speech>,
    /// The counter window on its own, as `cargo run -p feature-center -- caixa` has always opened.
    pub counter: bool,
}

/// What each parameter takes, for the message that refuses a wrong one.
const MODES: &str = "completo, vitrine, componentes, playground";
const LOOKS: &str = "escuro, claro, vidro";
const SPEECHES: &str = "pt-BR, en";

impl Opening {
    /// From the command line: `--modo vitrine --pagina X --incorporado`, and the old `caixa`.
    /// Anything it does not know is refused, saying what it takes.
    pub fn from_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut opening = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "caixa" => opening.counter = true,
                "--incorporado" => opening.embedded = true,
                flag if flag.starts_with("--") => {
                    let name = &flag[2..];
                    if !KEYS.contains(&name) {
                        return Err(format!(
                            "{flag} não existe; use --modo, --pagina, --componente, --tema, --idioma ou --incorporado"
                        ));
                    }
                    let value = arguments.next().ok_or_else(|| format!("{flag} precisa de um valor"))?;
                    opening.take(name, &value)?;
                }
                other => return Err(format!("{other} não é um parâmetro; use --modo, --pagina e os demais")),
            }
        }
        Ok(opening)
    }

    /// From the address of a page, as pairs of name and value. Names it does not know are left
    /// alone, since a link can carry others of its own; a wrong value of one it knows is refused.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Self, String> {
        let mut opening = Self::default();
        for (name, value) in pairs {
            match name {
                "incorporado" => opening.embedded = !matches!(value, "0" | "false" | "nao" | "não"),
                name if KEYS.contains(&name) => opening.take(name, value)?,
                _ => {}
            }
        }
        Ok(opening)
    }

    fn take(&mut self, name: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        match name {
            "modo" => {
                self.mode = match value {
                    "completo" => Mode::Whole,
                    "vitrine" => Mode::Showcase,
                    "componentes" => Mode::Components,
                    "playground" => Mode::Playground,
                    _ => return Err(format!("modo {value} não existe; use {MODES}")),
                }
            }
            "tema" => {
                self.look = Some(match value {
                    "escuro" => Look::Dark,
                    "claro" => Look::Light,
                    "vidro" => Look::Glass,
                    _ => return Err(format!("tema {value} não existe; use {LOOKS}")),
                })
            }
            "idioma" => {
                self.speech = Some(match value {
                    "pt-BR" | "pt" => Speech::Portuguese,
                    "en" => Speech::English,
                    _ => return Err(format!("idioma {value} não existe; use {SPEECHES}")),
                })
            }
            "pagina" | "componente" => {
                if value.is_empty() || !value.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c)) {
                    return Err(format!("{name} {value:?} não é um nome de página"));
                }
                let slot = if name == "pagina" { &mut self.page } else { &mut self.component };
                *slot = Some(value.to_owned());
            }
            _ => {}
        }
        Ok(())
    }
}

/// The parameters that take a value.
const KEYS: [&str; 5] = ["modo", "pagina", "componente", "tema", "idioma"];

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(line: &str) -> Result<Opening, String> {
        Opening::from_arguments(line.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn with_nothing_it_is_the_whole_center() {
        assert_eq!(arguments(""), Ok(Opening::default()));
        assert_eq!(Opening::from_pairs([]), Ok(Opening::default()));
    }

    #[test]
    fn the_command_line_takes_every_parameter() {
        let opening =
            arguments("--modo componentes --pagina CablePlugDemo --incorporado --tema claro --idioma en")
                .expect("all known");
        assert_eq!(opening.mode, Mode::Components);
        assert_eq!(opening.page.as_deref(), Some("CablePlugDemo"));
        assert!(opening.embedded);
        assert_eq!(opening.look, Some(Look::Light));
        assert_eq!(opening.speech, Some(Speech::English));
        assert!(arguments("caixa").expect("the old word").counter);
        let playground = arguments("--modo playground --componente Button").expect("known");
        assert_eq!((playground.mode, playground.component.as_deref()), (Mode::Playground, Some("Button")));
    }

    #[test]
    fn a_wrong_value_says_what_it_takes() {
        assert_eq!(arguments("--modo tudo"), Err(format!("modo tudo não existe; use {MODES}")));
        assert_eq!(arguments("--tema rosa"), Err(format!("tema rosa não existe; use {LOOKS}")));
        assert_eq!(arguments("--idioma fr"), Err(format!("idioma fr não existe; use {SPEECHES}")));
        assert!(arguments("--modo").is_err(), "a flag with no value");
        assert!(arguments("--api http://outro").is_err(), "no parameter takes an address");
        assert!(arguments("vitrine").is_err(), "a word that is not a flag");
    }

    #[test]
    fn the_address_takes_the_same_and_leaves_alone_what_it_does_not_know() {
        let opening = Opening::from_pairs([
            ("modo", "vitrine"),
            ("pagina", "ValidationRules_ListView"),
            ("tema", "vidro"),
            ("utm_source", "site"),
            ("api", "http://outro"),
        ])
        .expect("known values");
        assert_eq!(opening.mode, Mode::Showcase);
        assert_eq!(opening.page.as_deref(), Some("ValidationRules_ListView"));
        assert_eq!(opening.look, Some(Look::Glass));
        assert!(Opening::from_pairs([("incorporado", "")]).expect("on").embedded);
        assert!(!Opening::from_pairs([("incorporado", "0")]).expect("off").embedded);
        assert!(Opening::from_pairs([("modo", "x")]).is_err());
    }

    #[test]
    fn a_page_is_a_name_and_never_an_address() {
        for wrong in ["http://outro/x", "../segredo", "a b", ""] {
            assert!(Opening::from_pairs([("pagina", wrong)]).is_err(), "{wrong}");
        }
        assert!(Opening::from_pairs([("pagina", "Painel-1.v2")]).is_ok());
    }
}
