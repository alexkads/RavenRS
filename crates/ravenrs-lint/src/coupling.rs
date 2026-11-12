//! `module_cycle`: the top-level modules of a crate that use each other in a loop.
//!
//! The other rules read one file at a time; this one reads a whole crate. Each top-level module
//! (`mod grid;` in `lib.rs` or `main.rs`, with the files under it) is a node, and a module uses
//! another when its code names it: `use crate::grid::…`, `crate::grid::X` in an expression or a
//! type, `crate::X` that the root re-exports from `grid`, or `super::grid` in a file right under
//! the root. Test code is left out, as in the other rules.
//!
//! Modules that use each other, directly or around a loop, cannot be read, tested or moved to a
//! crate of their own apart from each other. Two of them (a screen and its helper) are common and
//! allowed; a longer loop is reported. `max-module-cycle` raises the limit for a crate that is
//! being untangled, and the rule asks to lower it once the loop shrinks, so it only goes down.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::Error;
use crate::config::{Config, DEFAULT_MAX_MODULE_CYCLE};
use crate::rules::{Allowances, Finding, Lint, is_test_code};

/// One module naming another, at the first place it does so in one file.
struct Edge {
    from: String,
    to: String,
    file: PathBuf,
    line: usize,
    column: usize,
    end_column: usize,
}

/// The top-level modules of one crate and which of them name which.
pub(crate) struct ModuleGraph {
    root: PathBuf,
    /// Each module and the lines of its files (an inline module counts its root's).
    lines: BTreeMap<String, usize>,
    edges: Vec<Edge>,
}

impl ModuleGraph {
    /// Reads the crate whose root is `root`, a `lib.rs` or `main.rs`.
    pub(crate) fn read(root: &Path) -> Result<Self, Error> {
        let source = read(root)?;
        let syntax = syn::parse_file(&source).map_err(|error| Error::Parse(root.to_owned(), error))?;
        let dir = root.parent().unwrap_or(Path::new("."));
        let modules = declared_modules(&syntax);
        let names: BTreeSet<String> = modules.iter().map(|(name, _)| name.clone()).collect();
        let exports = root_exports(&syntax, &names);

        let mut edges = Vec::new();
        let mut lines = BTreeMap::new();
        for (name, inline) in &modules {
            if let Some(items) = inline {
                // `mod name { .. }` written in the root itself.
                let allowed = Allowances::read(&source);
                collect(name, items, root, true, &names, &exports, &allowed, &mut edges);
                lines.insert(name.clone(), 0);
                continue;
            }
            let mut count = 0;
            for file in module_files(dir, name) {
                let text = read(&file)?;
                count += text.lines().count();
                let parsed = syn::parse_file(&text).map_err(|error| Error::Parse(file.clone(), error))?;
                let top = file.parent() == Some(dir) || file.ends_with(Path::new(name).join("mod.rs"));
                let allowed = Allowances::read(&text);
                collect(name, &parsed.items, &file, top, &names, &exports, &allowed, &mut edges);
            }
            lines.insert(name.clone(), count);
        }
        Ok(ModuleGraph { root: root.to_owned(), lines, edges })
    }

    fn adjacency(&self) -> BTreeMap<&str, BTreeSet<&str>> {
        let mut graph: BTreeMap<&str, BTreeSet<&str>> =
            self.lines.keys().map(|n| (n.as_str(), BTreeSet::new())).collect();
        for edge in &self.edges {
            graph.entry(edge.from.as_str()).or_default().insert(edge.to.as_str());
        }
        graph
    }

    /// Every loop longer than the limit, reported at the `use` that closes it, and the request to
    /// lower a limit set above what is needed.
    pub(crate) fn findings(&self, config: &Config) -> Vec<(PathBuf, Finding)> {
        let graph = self.adjacency();
        let loops = strongly_connected(&graph);
        let largest = loops.iter().map(Vec::len).max().unwrap_or(0);
        let limit = config.max_module_cycle;

        let mut findings = Vec::new();
        for members in loops.iter().filter(|members| members.len() > limit) {
            let (edge, path) = closing_edge(members, &graph, &self.edges);
            findings.push((
                edge.file.clone(),
                Finding {
                    lint: Lint::ModuleCycle,
                    line: edge.line,
                    column: edge.column,
                    end_column: edge.end_column,
                    message: format!(
                        "{} modules use each other in a loop, more than the {limit} allowed: {} (all of them: {})",
                        members.len(),
                        path.join(" → "),
                        listed(members, 12),
                    ),
                    help: "move what both sides need into a module below them, or let the lower module take a \
                           trait or a closure instead of naming the upper one; `cargo ravenrs-lint --modules` \
                           shows who uses whom. While a crate is being untangled, `max-module-cycle` in \
                           [package.metadata.ravenrs-lint] holds the size it has today"
                        .into(),
                },
            ));
        }
        if limit > DEFAULT_MAX_MODULE_CYCLE && largest < limit {
            let lower = largest.max(DEFAULT_MAX_MODULE_CYCLE);
            let help = if lower == DEFAULT_MAX_MODULE_CYCLE {
                "remove `max-module-cycle` from [package.metadata.ravenrs-lint]: the default is enough now"
                    .to_owned()
            } else {
                format!(
                    "set `max-module-cycle = {lower}` in [package.metadata.ravenrs-lint], so the loop cannot grow back"
                )
            };
            findings.push((
                self.root.clone(),
                Finding {
                    lint: Lint::ModuleCycle,
                    line: 1,
                    column: 1,
                    end_column: 2,
                    message: format!(
                        "the longest loop of modules now has {largest}, fewer than the {limit} `max-module-cycle` allows"
                    ),
                    help,
                },
            ));
        }
        findings
    }

    /// A table for whoever is untangling the crate: each module's size, how many it uses and
    /// how many use it, then the loops.
    pub(crate) fn report(&self) -> String {
        use std::fmt::Write as _;
        let graph = self.adjacency();
        let mut used_by: BTreeMap<&str, usize> = BTreeMap::new();
        for targets in graph.values() {
            for target in targets {
                *used_by.entry(target).or_default() += 1;
            }
        }
        let total: usize = self.lines.values().sum();
        let mut out =
            format!("{}: {total} lines in {} top-level modules\n\n", self.root.display(), self.lines.len());
        let _ = writeln!(out, "{:<28} {:>7} {:>5} {:>8}", "module", "lines", "uses", "used by");
        let mut rows: Vec<(&String, &usize)> = self.lines.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (name, lines) in rows {
            let uses = graph.get(name.as_str()).map_or(0, BTreeSet::len);
            let by = used_by.get(name.as_str()).copied().unwrap_or(0);
            let _ = writeln!(out, "{name:<28} {lines:>7} {uses:>5} {by:>8}");
        }
        let loops = strongly_connected(&graph);
        let _ = writeln!(
            out,
            "\nloops ({}): modules that use each other, directly or around a loop",
            loops.len()
        );
        for members in &loops {
            let (_, path) = closing_edge(members, &graph, &self.edges);
            let _ = writeln!(
                out,
                "  {} modules, e.g. {}: {}",
                members.len(),
                path.join(" → "),
                listed(members, usize::MAX)
            );
        }
        out
    }
}

fn listed(members: &[&str], at_most: usize) -> String {
    let shown: Vec<&str> = members.iter().take(at_most).copied().collect();
    let more = members.len().saturating_sub(shown.len());
    if more > 0 { format!("{} and {more} more", shown.join(", ")) } else { shown.join(", ") }
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|error| Error::Io(path.to_owned(), error))
}

/// The modules the root declares, outside test code: `mod a;` (no items) and `mod b { .. }`.
fn declared_modules(file: &syn::File) -> Vec<(String, Option<&[syn::Item]>)> {
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if !is_test_code(&module.attrs) => {
                Some((module.ident.to_string(), module.content.as_ref().map(|(_, items)| items.as_slice())))
            }
            _ => None,
        })
        .collect()
}

/// The names the root brings in from its modules (`pub use grid::{DataGrid, GridQuery};`), so
/// `crate::DataGrid` counts as a use of `grid`.
fn root_exports(file: &syn::File, modules: &BTreeSet<String>) -> HashMap<String, String> {
    fn walk(
        tree: &syn::UseTree,
        module: Option<&str>,
        modules: &BTreeSet<String>,
        out: &mut HashMap<String, String>,
    ) {
        match tree {
            syn::UseTree::Path(path) => {
                let name = path.ident.to_string();
                let module = module.or_else(|| modules.contains(&name).then_some(name.as_str()));
                walk(&path.tree, module, modules, out);
            }
            syn::UseTree::Name(name) => {
                if let Some(module) = module {
                    out.entry(name.ident.to_string()).or_insert_with(|| module.to_owned());
                }
            }
            syn::UseTree::Rename(rename) => {
                if let Some(module) = module {
                    out.entry(rename.rename.to_string()).or_insert_with(|| module.to_owned());
                }
            }
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    walk(tree, module, modules, out);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
    let mut out = HashMap::new();
    for item in &file.items {
        if let syn::Item::Use(item) = item {
            // `use self::grid::X` and `use crate::grid::X` in the root name the same module.
            let tree = match &item.tree {
                syn::UseTree::Path(path) if path.ident == "self" || path.ident == "crate" => &*path.tree,
                tree => tree,
            };
            walk(tree, None, modules, &mut out);
        }
    }
    out
}

/// `src/name.rs` or `src/name/mod.rs`, and every file under `src/name/`.
fn module_files(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let flat = dir.join(format!("{name}.rs"));
    if flat.is_file() {
        files.push(flat);
    }
    let mut stack = vec![dir.join(name)];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[allow(clippy::too_many_arguments)]
fn collect(
    from: &str,
    items: &[syn::Item],
    file: &Path,
    top: bool,
    modules: &BTreeSet<String>,
    exports: &HashMap<String, String>,
    allowed: &Allowances,
    edges: &mut Vec<Edge>,
) {
    let mut walker = Uses { from, top, depth: 0, modules, exports, found: Vec::new() };
    for item in items {
        walker.visit_item(item);
    }
    let mut seen = BTreeSet::new();
    walker.found.sort_by_key(|(_, span)| (span.0, span.1));
    for (to, (line, column, end_column)) in walker.found {
        if allowed.covers(Lint::ModuleCycle, line) || !seen.insert(to.clone()) {
            continue;
        }
        edges.push(Edge { from: from.to_owned(), to, file: file.to_owned(), line, column, end_column });
    }
}

/// Finds the modules one file names.
struct Uses<'a> {
    from: &'a str,
    /// The file sits right under the root, so `super::` is the root outside inline modules.
    top: bool,
    /// How many inline `mod x { .. }` the walk is inside; `super::` there is not the root.
    depth: usize,
    modules: &'a BTreeSet<String>,
    exports: &'a HashMap<String, String>,
    found: Vec<(String, (usize, usize, usize))>,
}

impl Uses<'_> {
    fn root_is_super(&self) -> bool {
        self.top && self.depth == 0
    }

    fn record(&mut self, head: &syn::Ident, span: proc_macro2::Span) {
        let name = head.to_string();
        let target = if self.modules.contains(&name) { Some(name) } else { self.exports.get(&name).cloned() };
        let Some(target) = target.filter(|t| t != self.from) else { return };
        let start = span.start();
        let end = span.end();
        let end_column = if end.line == start.line && end.column > start.column {
            end.column + 1
        } else {
            start.column + 2
        };
        self.found.push((target, (start.line, start.column + 1, end_column)));
    }

    /// `crate::a::…`, `crate::{a::X, b}`, `super::a` in a top file: the module after the prefix.
    fn use_tree(&mut self, tree: &syn::UseTree, whole: proc_macro2::Span) {
        let syn::UseTree::Path(path) = tree else { return };
        let rooted = path.ident == "crate" || (self.root_is_super() && path.ident == "super");
        if !rooted {
            return;
        }
        self.heads(&path.tree, whole);
    }

    fn heads(&mut self, tree: &syn::UseTree, whole: proc_macro2::Span) {
        match tree {
            syn::UseTree::Path(path) => self.record(&path.ident, whole),
            syn::UseTree::Name(name) => self.record(&name.ident, whole),
            syn::UseTree::Rename(rename) => self.record(&rename.ident, whole),
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.heads(tree, whole);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
}

impl<'ast> Visit<'ast> for Uses<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let attrs: &[syn::Attribute] = match item {
            syn::Item::Mod(m) => &m.attrs,
            syn::Item::Fn(f) => &f.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Use(u) => &u.attrs,
            _ => &[],
        };
        if is_test_code(attrs) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.depth += 1;
        visit::visit_item_mod(self, node);
        self.depth -= 1;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if !is_test_code(&node.attrs) {
            visit::visit_impl_item_fn(self, node);
        }
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.use_tree(&node.tree, node.span());
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let mut segments = path.segments.iter();
        if let (Some(first), Some(second)) = (segments.next(), segments.next())
            && (first.ident == "crate" || (self.root_is_super() && first.ident == "super"))
        {
            self.record(&second.ident, path.span());
        }
        visit::visit_path(self, path);
    }
}

/// Tarjan's strongly connected components with more than one module, each sorted, largest first.
fn strongly_connected<'a>(graph: &BTreeMap<&'a str, BTreeSet<&'a str>>) -> Vec<Vec<&'a str>> {
    struct State<'a> {
        index: HashMap<&'a str, usize>,
        low: HashMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on_stack: BTreeSet<&'a str>,
        next: usize,
        out: Vec<Vec<&'a str>>,
    }
    fn visit<'a>(v: &'a str, graph: &BTreeMap<&'a str, BTreeSet<&'a str>>, s: &mut State<'a>) {
        s.index.insert(v, s.next);
        s.low.insert(v, s.next);
        s.next += 1;
        s.stack.push(v);
        s.on_stack.insert(v);
        for &w in graph.get(v).into_iter().flatten() {
            if !s.index.contains_key(w) {
                visit(w, graph, s);
                let low = s.low[v].min(s.low[w]);
                s.low.insert(v, low);
            } else if s.on_stack.contains(w) {
                let low = s.low[v].min(s.index[w]);
                s.low.insert(v, low);
            }
        }
        if s.low[v] == s.index[v] {
            let mut component = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack.remove(w);
                component.push(w);
                if w == v {
                    break;
                }
            }
            if component.len() > 1 {
                component.sort_unstable();
                s.out.push(component);
            }
        }
    }
    let mut state = State {
        index: HashMap::new(),
        low: HashMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        next: 0,
        out: Vec::new(),
    };
    for &v in graph.keys() {
        if !state.index.contains_key(v) {
            visit(v, graph, &mut state);
        }
    }
    state.out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    state.out
}

/// Where to report a loop: the module that names the most others in it (the knot to untie
/// first), at its first edge into the loop, with the shortest way back to it.
fn closing_edge<'e>(
    members: &[&str],
    graph: &BTreeMap<&str, BTreeSet<&str>>,
    edges: &'e [Edge],
) -> (&'e Edge, Vec<String>) {
    let inside: BTreeSet<&str> = members.iter().copied().collect();
    let out_degree = |m: &str| graph.get(m).map_or(0, |to| to.iter().filter(|t| inside.contains(*t)).count());
    let hub =
        members.iter().copied().max_by_key(|m| (out_degree(m), std::cmp::Reverse(*m))).unwrap_or(members[0]);
    // The hub's edges into the loop, in file order; the first with the shortest way back wins.
    let mut best: Option<(&Edge, Vec<String>)> = None;
    for edge in edges.iter().filter(|e| e.from == hub && inside.contains(e.to.as_str())) {
        if let Some(back) = shortest_path(&edge.to, hub, graph, &inside) {
            let mut path = vec![hub.to_owned()];
            path.extend(back);
            if best.as_ref().is_none_or(|(_, p)| path.len() < p.len()) {
                best = Some((edge, path));
            }
        }
    }
    best.unwrap_or_else(|| {
        let edge = edges.iter().find(|e| inside.contains(e.from.as_str()) && inside.contains(e.to.as_str()));
        // A loop always has an edge inside it.
        let edge = edge.unwrap_or(&edges[0]);
        (edge, vec![edge.from.clone(), edge.to.clone()])
    })
}

/// The modules from `from` to `to` (both included), walking only inside the loop.
fn shortest_path(
    from: &str,
    to: &str,
    graph: &BTreeMap<&str, BTreeSet<&str>>,
    inside: &BTreeSet<&str>,
) -> Option<Vec<String>> {
    let mut previous: HashMap<&str, &str> = HashMap::new();
    let mut queue = VecDeque::from([from]);
    let mut seen = BTreeSet::from([from]);
    while let Some(v) = queue.pop_front() {
        if v == to {
            let mut path = vec![v.to_owned()];
            let mut at = v;
            while let Some(&p) = previous.get(at) {
                path.push(p.to_owned());
                at = p;
            }
            path.reverse();
            return Some(path);
        }
        for &w in graph.get(v).into_iter().flatten() {
            if inside.contains(w) && seen.insert(w) {
                previous.insert(w, v);
                queue.push_back(w);
            }
        }
    }
    None
}
