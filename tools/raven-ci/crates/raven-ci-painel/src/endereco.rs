//! The address of each page of the panel (#969): what the browser shows after `#`, what a notice
//! of a build links to, and what the desktop panel opens with `--abrir`.
//!
//! | Page | Address |
//! |---|---|
//! | [`Pagina::Jobs`] | `#/jobs` |
//! | [`Pagina::Historico`] | `#/historico` |
//! | [`Pagina::Build`] | `#/build/<job>/<número>`, the job escaped whole: `apps/site` is `apps%2Fsite` |
//! | [`Pagina::Fila`] | `#/fila` |
//! | [`Pagina::Configuracao`] | `#/configuracao` |
//! | [`Pagina::Plugins`] | `#/plugins` |
//! | [`Pagina::Credenciais`] | `#/credenciais` |
//! | [`Pagina::Usuarios`] | `#/usuarios` |
//! | [`Pagina::Plugin`] | `#/plugin/<nome>` |
//!
//! [`Endereco`] is how the panel and the place that shows the address (the browser's bar) talk:
//! the place asks for a page with [`Endereco::ask`], and the panel tells it each page it went to.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::prelude::UiWaker;

use crate::Pagina;

impl Pagina {
    /// The address of the page, without the `#`: `/build/apps%2Fsite/3`.
    pub fn to_path(&self) -> String {
        match self {
            Self::Jobs => "/jobs".to_owned(),
            Self::Historico => "/historico".to_owned(),
            Self::Build(job, number) => format!("/build/{}/{number}", escape(job)),
            Self::Fila => "/fila".to_owned(),
            Self::Configuracao => "/configuracao".to_owned(),
            Self::Plugins => "/plugins".to_owned(),
            Self::Credenciais => "/credenciais".to_owned(),
            Self::Usuarios => "/usuarios".to_owned(),
            Self::Plugin(name) => format!("/plugin/{}", escape(name)),
        }
    }

    /// The page at `address`: a path as [`to_path`](Self::to_path) gives, with or without the
    /// `#`, or the whole address of the panel (`https://ci/painel/#/build/apps%2Fsite/3`).
    /// `None` when it names no page.
    pub fn from_path(address: &str) -> Option<Self> {
        let path = address.rsplit_once('#').map_or(address, |(_, after)| after);
        let path = path.trim_start_matches('/').trim_end_matches('/');
        let parts: Vec<&str> = path.split('/').collect();
        Some(match parts.as_slice() {
            ["jobs"] | [""] => Self::Jobs,
            ["historico"] => Self::Historico,
            ["fila"] => Self::Fila,
            ["configuracao"] => Self::Configuracao,
            ["plugins"] => Self::Plugins,
            ["credenciais"] => Self::Credenciais,
            ["usuarios"] => Self::Usuarios,
            ["plugin", name] if !name.is_empty() => Self::Plugin(unescape(name)?),
            ["build", job, number] if !job.is_empty() => Self::Build(unescape(job)?, number.parse().ok()?),
            _ => return None,
        })
    }
}

/// Text put in one part of an address: everything but letters, digits and `-_.~` escaped, the
/// `/` of a folder included.
fn escape(text: &str) -> String {
    crate::ui::escaped(text)
}

/// The text of one escaped part, or `None` when an escape is broken or the bytes are no UTF-8.
fn unescape(part: &str) -> Option<String> {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

type Shown = Box<dyn Fn(&Pagina)>;

/// Where the address of the panel's page lives outside it: the browser's bar, or the argument the
/// desktop panel opened with. Clones share it.
#[derive(Clone, Default)]
pub struct Endereco {
    inner: Rc<Inner>,
}

#[derive(Default)]
struct Inner {
    asked: RefCell<Option<Pagina>>,
    shown: RefCell<Option<Shown>>,
    waker: RefCell<Option<UiWaker>>,
}

impl std::fmt::Debug for Endereco {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endereco").field("asked", &self.inner.asked.borrow()).finish_non_exhaustive()
    }
}

impl Endereco {
    /// An address with nothing asked and no one told.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calls `shown` with each page the panel goes to: the browser puts it in its bar.
    #[must_use]
    pub fn on_shown(self, shown: impl Fn(&Pagina) + 'static) -> Self {
        *self.inner.shown.borrow_mut() = Some(Box::new(shown));
        self
    }

    /// Asks the panel for `page` (the address changed: back, forward, a link) and wakes it. Not
    /// signed in, the panel goes there once someone signs in.
    pub fn ask(&self, page: Pagina) {
        *self.inner.asked.borrow_mut() = Some(page);
        if let Some(waker) = self.inner.waker.borrow().as_ref() {
            waker.wake();
        }
    }

    /// The page asked and not yet taken.
    pub(crate) fn take(&self) -> Option<Pagina> {
        self.inner.asked.borrow_mut().take()
    }

    pub(crate) fn wake_with(&self, waker: UiWaker) {
        *self.inner.waker.borrow_mut() = Some(waker);
    }

    pub(crate) fn show(&self, page: &Pagina) {
        if let Some(shown) = self.inner.shown.borrow().as_ref() {
            shown(page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_page() -> Vec<Pagina> {
        vec![
            Pagina::Jobs,
            Pagina::Historico,
            Pagina::Build("apps/site".to_owned(), 3),
            Pagina::Build("PR-12".to_owned(), 1),
            Pagina::Build("relatórios e notas".to_owned(), 42),
            Pagina::Fila,
            Pagina::Configuracao,
            Pagina::Plugins,
            Pagina::Credenciais,
            Pagina::Usuarios,
            Pagina::Plugin("demonstracao/travas".to_owned()),
        ]
    }

    #[test]
    fn every_page_comes_back_from_its_address() {
        for page in every_page() {
            let path = page.to_path();
            assert_eq!(Pagina::from_path(&path), Some(page.clone()), "{path}");
            assert_eq!(Pagina::from_path(&format!("#{path}")), Some(page.clone()), "#{path}");
        }
    }

    #[test]
    fn a_job_in_a_folder_is_escaped_whole() {
        assert_eq!(Pagina::Build("apps/site".to_owned(), 3).to_path(), "/build/apps%2Fsite/3");
        assert_eq!(
            Pagina::from_path("#/build/apps%2Fsite/3"),
            Some(Pagina::Build("apps/site".to_owned(), 3))
        );
    }

    #[test]
    fn the_whole_address_of_the_panel_opens_the_page() {
        assert_eq!(
            Pagina::from_path("https://ci.exemplo/painel/#/build/apps%2Fsite/7"),
            Some(Pagina::Build("apps/site".to_owned(), 7))
        );
        assert_eq!(Pagina::from_path("https://ci.exemplo/painel/"), None);
        assert_eq!(Pagina::from_path("https://ci.exemplo/painel/#/"), Some(Pagina::Jobs));
        assert_eq!(Pagina::from_path(""), Some(Pagina::Jobs));
    }

    #[test]
    fn an_address_that_names_no_page_is_none() {
        for wrong in ["#/nada", "#/build/apps", "#/build/apps/x", "#/build//3", "#/build/a%2/3", "#/plugin/"]
        {
            assert_eq!(Pagina::from_path(wrong), None, "{wrong}");
        }
    }
}
