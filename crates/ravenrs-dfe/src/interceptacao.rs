//! The chain of handlers of the sandboxes, as in MSW: the newest handler answers first, a `use_once`
//! goes away once it answers, `reset_handlers` brings back the initial ones, and a request nothing
//! answers follows [`OnUnhandled`]. Each engine says what a handler is and what it answers; for a
//! REST service, [`Rota`] matches the method and the path, with `{name}` for a part that varies.

use std::sync::Arc;

use crate::transporte::{HttpRequest, Metodo, Transport};

/// What to do with a request nothing answers, like `onUnhandledRequest` in MSW.
#[derive(Clone, Default)]
pub enum OnUnhandled {
    /// Fails it with `TransportError::NaoTratado`. The default.
    #[default]
    Error,
    /// Records a warning and answers 404, in the format of the service.
    ///
    /// Unlike MSW, a warning never goes to the network: only `Passthrough` does that.
    Warn,
    /// Sends it through this transport. This is the only way a request can leave a sandbox.
    Passthrough(Arc<dyn Transport>),
}

impl std::fmt::Debug for OnUnhandled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Error => "Error",
            Self::Warn => "Warn",
            Self::Passthrough(_) => "Passthrough",
        })
    }
}

struct Entrada<H> {
    handler: H,
    once: bool,
    id: u64,
}

/// The handlers of a sandbox, in the order they were added.
///
/// The sandbox keeps it under its lock and calls a handler with the lock released, so a handler can
/// call back into the sandbox: take the [`Cadeia::candidatos`], then [`Cadeia::consumir`] each
/// `use_once` before calling it.
pub struct Cadeia<H> {
    iniciais: Vec<H>,
    entradas: Vec<Entrada<H>>,
    proximo_id: u64,
}

impl<H: Clone> Cadeia<H> {
    /// A chain with these initial handlers, which [`Self::restaurar`] brings back.
    #[must_use]
    pub fn com_iniciais(iniciais: Vec<H>) -> Self {
        let mut cadeia = Self { iniciais, entradas: Vec::new(), proximo_id: 0 };
        cadeia.restaurar();
        cadeia
    }

    /// Adds a handler that stays, or one that answers once and goes away.
    pub fn adicionar(&mut self, handler: H, once: bool) {
        let id = self.proximo_id;
        self.proximo_id += 1;
        self.entradas.push(Entrada { handler, once, id });
    }

    /// Goes back to the initial handlers.
    pub fn restaurar(&mut self) {
        self.entradas.clear();
        for handler in self.iniciais.clone() {
            self.adicionar(handler, false);
        }
    }

    /// The handlers that take this request, newest first, as `(id, once, handler)`.
    pub fn candidatos(&self, atende: impl Fn(&H) -> bool) -> Vec<(u64, bool, H)> {
        self.entradas
            .iter()
            .rev()
            .filter(|entrada| atende(&entrada.handler))
            .map(|entrada| (entrada.id, entrada.once, entrada.handler.clone()))
            .collect()
    }

    /// Takes out a `use_once` that is about to answer. `false` means another thread used it first.
    pub fn consumir(&mut self, id: u64) -> bool {
        match self.entradas.iter().position(|entrada| entrada.id == id) {
            Some(posicao) => {
                self.entradas.remove(posicao);
                true
            }
            None => false,
        }
    }

    /// How many handlers are in place.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entradas.len()
    }

    /// Whether there is no handler in place.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entradas.is_empty()
    }
}

impl<H> std::fmt::Debug for Cadeia<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cadeia").field("handlers", &self.entradas.len()).finish_non_exhaustive()
    }
}

impl<H: Clone> Default for Cadeia<H> {
    fn default() -> Self {
        Self::com_iniciais(Vec::new())
    }
}

/// A method and a path, with `{name}` for a part that varies: `GET /nfse/{chave}`.
///
/// The path is compared from its end, so the same route takes a base with or without a prefix such
/// as `/SefinNacional` or `/API/SefinNacional`, which the sources do not agree on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rota {
    /// The method.
    pub metodo: Metodo,
    partes: Vec<String>,
}

impl Rota {
    /// A route for this method and path.
    #[must_use]
    pub fn new(metodo: Metodo, caminho: &str) -> Self {
        Self { metodo, partes: partes(caminho).map(str::to_owned).collect() }
    }

    /// The parts that vary, by name, if the request takes this route.
    #[must_use]
    pub fn casar(&self, pedido: &HttpRequest) -> Option<Vec<(String, String)>> {
        if pedido.metodo != self.metodo {
            return None;
        }
        let recebidas: Vec<&str> = partes(pedido.caminho()).collect();
        if recebidas.len() < self.partes.len() {
            return None;
        }
        let fim = &recebidas[recebidas.len() - self.partes.len()..];
        let mut valores = Vec::new();
        for (esperada, recebida) in self.partes.iter().zip(fim) {
            match esperada.strip_prefix('{').and_then(|nome| nome.strip_suffix('}')) {
                Some(nome) => valores.push((nome.to_owned(), (*recebida).to_owned())),
                None if esperada.eq_ignore_ascii_case(recebida) => {}
                None => return None,
            }
        }
        Some(valores)
    }
}

impl std::fmt::Display for Rota {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} /{}", self.metodo.nome(), self.partes.join("/"))
    }
}

fn partes(caminho: &str) -> impl Iterator<Item = &str> {
    caminho.split('/').filter(|parte| !parte.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_and_once_goes_away() {
        let mut cadeia = Cadeia::com_iniciais(vec!["inicial"]);
        cadeia.adicionar("depois", false);
        cadeia.adicionar("uma vez", true);
        let todos = cadeia.candidatos(|_| true);
        assert_eq!(todos.iter().map(|(_, _, h)| *h).collect::<Vec<_>>(), ["uma vez", "depois", "inicial"]);
        let (id, once, _) = todos[0];
        assert!(once && cadeia.consumir(id));
        assert!(!cadeia.consumir(id), "only once");
        assert_eq!(cadeia.len(), 2);
        cadeia.restaurar();
        assert_eq!(cadeia.candidatos(|_| true).len(), 1);
    }

    #[test]
    fn a_route_takes_its_method_and_path_with_any_prefix() {
        let rota = Rota::new(Metodo::Get, "/nfse/{chave}");
        let pedido = HttpRequest::get("https://sefin.nfse.gov.br/SefinNacional/nfse/NFS123?x", Vec::new());
        assert_eq!(rota.casar(&pedido), Some(vec![("chave".to_owned(), "NFS123".to_owned())]));
        assert_eq!(rota.casar(&HttpRequest::head("https://h/nfse/NFS123", Vec::new())), None, "method");
        assert_eq!(rota.casar(&HttpRequest::get("https://h/dps/1", Vec::new())), None, "path");
        assert_eq!(rota.to_string(), "GET /nfse/{chave}");
    }
}
