//! An index of the public API, parsed from the embedded sources.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Expr, Fields, ImplItem, Item as SynItem, TraitItem, UseTree, Visibility};

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// Longest usage kept as an example, in bytes. Whole pages teach little.
const MAX_EXAMPLE: usize = 4000;

/// A public struct, enum, trait, function or type alias.
#[derive(Debug, Default)]
pub struct Item {
    pub krate: String,
    pub name: String,
    pub kind: &'static str,
    pub file: String,
    pub line: usize,
    pub signature: String,
    pub docs: String,
    pub non_exhaustive: bool,
    /// Public fields of a struct.
    pub fields: Vec<Member>,
    /// Variants of an enum.
    pub variants: Vec<Member>,
    /// Public inherent methods, or the methods of a trait.
    pub methods: Vec<Member>,
    /// Traits implemented in the defining crate, such as `View`.
    pub traits: BTreeSet<String>,
}

/// A field, variant or method of an [`Item`].
#[derive(Debug)]
pub struct Member {
    pub name: String,
    pub signature: String,
    pub docs: String,
    pub line: usize,
}

/// A call chain in an example, such as `Button::new("Save").style(...)`.
#[derive(Debug)]
pub struct Usage {
    /// The type or function the chain starts from.
    pub root: String,
    pub file: String,
    pub line: usize,
    pub code: String,
    pub methods: BTreeSet<String>,
}

/// Everything parsed from the sources.
#[derive(Debug, Default)]
pub struct Index {
    pub items: Vec<Item>,
    pub usages: Vec<Usage>,
    /// Names `ravenrs-widgets` exports from its root.
    widget_exports: BTreeSet<String>,
    /// Sources that failed to parse, as `(file, error)`. Always empty in a healthy build.
    pub failures: Vec<(String, String)>,
}

impl Index {
    /// The index, parsed on first use.
    pub fn get() -> &'static Index {
        static INDEX: OnceLock<Index> = OnceLock::new();
        INDEX.get_or_init(Index::parse)
    }

    fn parse() -> Index {
        let mut index = Index::default();
        let mut impls = Vec::new();
        for (file, text) in RUST_SOURCES {
            let parsed = match syn::parse_file(text) {
                Ok(parsed) => parsed,
                Err(error) => {
                    index.failures.push(((*file).to_owned(), error.to_string()));
                    continue;
                }
            };
            match file.strip_prefix("crates/").and_then(|rest| rest.split('/').next()) {
                Some(krate) => {
                    let cx = FileCx { krate, file, text };
                    cx.collect(&parsed.items, &mut index.items, &mut impls);
                    if *file == "crates/ravenrs-widgets/src/lib.rs" {
                        for item in &parsed.items {
                            if let SynItem::Use(export) = item
                                && matches!(export.vis, Visibility::Public(_))
                            {
                                use_names(&export.tree, &mut index.widget_exports);
                            }
                        }
                    }
                }
                None => UsageFinder { file, text, usages: &mut index.usages }.visit_file(&parsed),
            }
        }
        for found in impls {
            let target =
                index.items.iter_mut().find(|item| item.krate == found.krate && item.name == found.name);
            if let Some(item) = target {
                item.methods.extend(found.methods);
                item.traits.extend(found.traits);
            }
        }
        index
    }

    /// Every public item with this name, in any crate. Case does not matter.
    pub fn find(&self, name: &str) -> Vec<&Item> {
        self.items.iter().filter(|item| item.name.eq_ignore_ascii_case(name)).collect()
    }

    /// The closest item names, for a name that was not found.
    pub fn suggest(&self, name: &str) -> Vec<&str> {
        let name = name.to_lowercase();
        let mut scored: Vec<(usize, &str)> = self
            .items
            .iter()
            .filter_map(|item| {
                let candidate = item.name.to_lowercase();
                let distance = edit_distance(&name, &candidate);
                let near = distance <= 2 || candidate.contains(&name) || name.contains(&candidate);
                near.then_some((distance, item.name.as_str()))
            })
            .collect();
        scored.sort();
        scored.dedup();
        scored.into_iter().take(5).map(|(_, name)| name).collect()
    }

    /// The items that are components: exported from `ravenrs-widgets` and drawable as a view.
    pub fn components(&self) -> Vec<&Item> {
        let mut components: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| {
                item.krate == "ravenrs-widgets"
                    && item.kind == "struct"
                    && self.widget_exports.contains(&item.name)
                    && (item.traits.contains("View") || item.traits.contains("Component"))
                    // View plumbing, not something to put on a screen.
                    && item.file != "crates/ravenrs-widgets/src/view.rs"
                    && item.file != "crates/ravenrs-widgets/src/modifiers.rs"
            })
            .collect();
        components.sort_by(|a, b| a.name.cmp(&b.name));
        components
    }
}

/// An `impl` block's contribution to an item.
struct FoundImpl {
    krate: String,
    name: String,
    methods: Vec<Member>,
    traits: Vec<String>,
}

struct FileCx<'a> {
    krate: &'a str,
    file: &'a str,
    text: &'a str,
}

impl FileCx<'_> {
    fn collect(&self, items: &[SynItem], out: &mut Vec<Item>, impls: &mut Vec<FoundImpl>) {
        for item in items {
            match item {
                SynItem::Mod(module) if !is_test(&module.attrs) => {
                    if let Some((_, nested)) = &module.content {
                        self.collect(nested, out, impls);
                    }
                }
                SynItem::Struct(item) if is_pub(&item.vis) => {
                    let mut entry = self.entry("struct", &item.ident, &item.attrs);
                    entry.signature = format!("pub struct {}{}", item.ident, self.generics(&item.generics));
                    if let Fields::Named(fields) = &item.fields {
                        for field in fields.named.iter().filter(|field| is_pub(&field.vis)) {
                            if let Some(ident) = &field.ident {
                                entry.fields.push(Member {
                                    name: ident.to_string(),
                                    signature: format!("{ident}: {}", self.source(field.ty.span())),
                                    docs: docs(&field.attrs),
                                    line: ident.span().start().line,
                                });
                            }
                        }
                    }
                    out.push(entry);
                }
                SynItem::Enum(item) if is_pub(&item.vis) => {
                    let mut entry = self.entry("enum", &item.ident, &item.attrs);
                    entry.signature = format!("pub enum {}{}", item.ident, self.generics(&item.generics));
                    for variant in &item.variants {
                        let fields = match &variant.fields {
                            Fields::Unit => String::new(),
                            fields => self.source(fields.span()),
                        };
                        entry.variants.push(Member {
                            name: variant.ident.to_string(),
                            signature: format!("{}{fields}", variant.ident),
                            docs: docs(&variant.attrs),
                            line: variant.ident.span().start().line,
                        });
                    }
                    out.push(entry);
                }
                SynItem::Trait(item) if is_pub(&item.vis) => {
                    let mut entry = self.entry("trait", &item.ident, &item.attrs);
                    let supertraits = match item.supertraits.is_empty() {
                        true => String::new(),
                        false => format!(": {}", self.source(item.supertraits.span())),
                    };
                    entry.signature =
                        format!("pub trait {}{}{supertraits}", item.ident, self.generics(&item.generics));
                    for member in &item.items {
                        if let TraitItem::Fn(method) = member {
                            entry.methods.push(self.method(&method.sig, &method.attrs, ""));
                        }
                    }
                    out.push(entry);
                }
                SynItem::Fn(item) if is_pub(&item.vis) => {
                    let mut entry = self.entry("function", &item.sig.ident, &item.attrs);
                    entry.signature = format!("pub {}", self.source(item.sig.span()));
                    out.push(entry);
                }
                SynItem::Type(item) if is_pub(&item.vis) => {
                    let mut entry = self.entry("type alias", &item.ident, &item.attrs);
                    entry.signature = format!("pub type {} = {}", item.ident, self.source(item.ty.span()));
                    out.push(entry);
                }
                SynItem::Macro(invocation) => {
                    self.collect_from_macro(invocation, items, out, impls);
                }
                SynItem::Impl(item) if !is_test(&item.attrs) => {
                    if let syn::Type::Path(target) = item.self_ty.as_ref()
                        && let Some(last) = target.path.segments.last()
                    {
                        let mut found = FoundImpl {
                            krate: self.krate.to_owned(),
                            name: last.ident.to_string(),
                            methods: Vec::new(),
                            traits: Vec::new(),
                        };
                        match &item.trait_ {
                            Some((_, path, _)) => {
                                found.traits.extend(path.segments.last().map(|s| s.ident.to_string()));
                            }
                            None => {
                                for member in &item.items {
                                    match member {
                                        ImplItem::Fn(method) if is_pub(&method.vis) => {
                                            found.methods.push(self.method(
                                                &method.sig,
                                                &method.attrs,
                                                "pub ",
                                            ));
                                        }
                                        ImplItem::Const(constant) if is_pub(&constant.vis) => {
                                            found.methods.push(Member {
                                                name: constant.ident.to_string(),
                                                signature: format!(
                                                    "pub const {}: {}",
                                                    constant.ident,
                                                    self.source(constant.ty.span())
                                                ),
                                                docs: docs(&constant.attrs),
                                                line: constant.ident.span().start().line,
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        impls.push(found);
                    }
                }
                _ => {}
            }
        }
    }

    /// Items a local `macro_rules!` defines, such as `VStack` and `HStack`. Expands only the
    /// `$(#[$doc:meta])* $name:ident, $axis:expr` shape; the README test catches anything else.
    fn collect_from_macro(
        &self,
        invocation: &syn::ItemMacro,
        siblings: &[SynItem],
        out: &mut Vec<Item>,
        impls: &mut Vec<FoundImpl>,
    ) {
        let definition = siblings.iter().find_map(|item| match item {
            SynItem::Macro(definition)
                if definition.mac.path.is_ident("macro_rules")
                    && definition.ident.as_ref().is_some_and(|ident| invocation.mac.path.is_ident(ident)) =>
            {
                Some(definition)
            }
            _ => None,
        });
        let body = definition.and_then(|definition| {
            definition.mac.tokens.clone().into_iter().find_map(|token| match token {
                proc_macro2::TokenTree::Group(group)
                    if group.delimiter() == proc_macro2::Delimiter::Brace =>
                {
                    self.text.get(group.span_open().byte_range().end..group.span_close().byte_range().start)
                }
                _ => None,
            })
        });
        let Some(body) = body else { return };
        let Ok(arguments) = invocation.mac.parse_body_with(|input: syn::parse::ParseStream<'_>| {
            let attrs = input.call(Attribute::parse_outer)?;
            let name: syn::Ident = input.parse()?;
            let _: syn::Token![,] = input.parse()?;
            let rest: proc_macro2::TokenStream = input.parse()?;
            Ok((attrs, name, rest))
        }) else {
            return;
        };
        let (attrs, name, rest) = arguments;
        let doc_lines: String = attrs.iter().map(|attr| format!("{}\n", self.source(attr.span()))).collect();
        let expanded = body
            .replace("$(#[$doc])*", &doc_lines)
            .replace("$name", &name.to_string())
            .replace("$axis", &rest.to_string());
        let Ok(parsed) = syn::parse_file(&expanded) else { return };

        let line = name.span().start().line;
        let (items_before, impls_before) = (out.len(), impls.len());
        let cx = FileCx { krate: self.krate, file: self.file, text: &expanded };
        cx.collect(&parsed.items, out, impls);
        // Spans point into the expansion; the invocation is where a reader should land.
        for item in &mut out[items_before..] {
            item.line = line;
            item.fields
                .iter_mut()
                .chain(&mut item.variants)
                .chain(&mut item.methods)
                .for_each(|m| m.line = line);
        }
        for found in &mut impls[impls_before..] {
            found.methods.iter_mut().for_each(|method| method.line = line);
        }
    }

    fn entry(&self, kind: &'static str, ident: &syn::Ident, attrs: &[Attribute]) -> Item {
        Item {
            krate: self.krate.to_owned(),
            name: ident.to_string(),
            kind,
            file: self.file.to_owned(),
            line: ident.span().start().line,
            docs: docs(attrs),
            non_exhaustive: attrs.iter().any(|attr| attr.path().is_ident("non_exhaustive")),
            ..Item::default()
        }
    }

    fn method(&self, sig: &syn::Signature, attrs: &[Attribute], prefix: &str) -> Member {
        Member {
            name: sig.ident.to_string(),
            signature: format!("{prefix}{}", self.source(sig.span())),
            docs: docs(attrs),
            line: sig.ident.span().start().line,
        }
    }

    fn generics(&self, generics: &syn::Generics) -> String {
        match generics.lt_token {
            Some(_) => self.source(generics.span()),
            None => String::new(),
        }
    }

    /// The source text of a span, as written, on one line.
    fn source(&self, span: proc_macro2::Span) -> String {
        let text = self.text.get(span.byte_range()).unwrap_or_default();
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg") && attr.parse_args::<syn::Ident>().is_ok_and(|ident| ident == "test")
    })
}

/// Rustdoc text from `///` attributes.
fn docs(attrs: &[Attribute]) -> String {
    let lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(pair) => match &pair.value {
                Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .map(|line| line.strip_prefix(' ').unwrap_or(&line).to_owned())
        .collect();
    lines.join("\n").trim().to_owned()
}

fn use_names(tree: &UseTree, out: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => use_names(&path.tree, out),
        UseTree::Name(name) => {
            out.insert(name.ident.to_string());
        }
        UseTree::Rename(rename) => {
            out.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => group.items.iter().for_each(|tree| use_names(tree, out)),
        UseTree::Glob(_) => {}
    }
}

/// Finds call chains in example code. Macro bodies are opaque to it.
struct UsageFinder<'a> {
    file: &'a str,
    text: &'a str,
    usages: &'a mut Vec<Usage>,
}

impl<'ast> Visit<'ast> for UsageFinder<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        let mut methods = BTreeSet::new();
        let mut current = expr;
        while let Expr::MethodCall(call) = current {
            methods.insert(call.method.to_string());
            call.args.iter().for_each(|arg| self.visit_expr(arg));
            current = &call.receiver;
        }
        if let Expr::Call(call) = current
            && let Some(root) = callee(&call.func)
        {
            let span = expr.span();
            let code = self.text.get(span.byte_range()).unwrap_or_default();
            if code.len() <= MAX_EXAMPLE {
                self.usages.push(Usage {
                    root,
                    file: self.file.to_owned(),
                    line: span.start().line,
                    code: dedent(code),
                    methods,
                });
            }
            call.args.iter().for_each(|arg| self.visit_expr(arg));
        } else if methods.is_empty() {
            syn::visit::visit_expr(self, expr);
        } else {
            self.visit_expr(current);
        }
    }
}

/// The type a call is made on (`Button` in `Button::new`), or a free function's name.
fn callee(func: &Expr) -> Option<String> {
    let Expr::Path(path) = func else { return None };
    let segments = &path.path.segments;
    let index = segments.len().saturating_sub(2);
    segments.get(index).map(|segment| segment.ident.to_string())
}

/// Removes the indentation the lines after the first share. The first line starts at its span.
fn dedent(code: &str) -> String {
    let indent = code
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut lines = code.lines();
    let mut out: Vec<&str> = lines.next().into_iter().collect();
    out.extend(lines.map(|line| line.get(indent..).unwrap_or(line.trim_start())));
    out.join("\n")
}

/// The Rust code blocks of a Markdown text that mention `name`.
pub fn code_blocks_with<'a>(markdown: &'a str, name: &str) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let mut parts = markdown.split("```");
    parts.next();
    while let Some(block) = parts.next() {
        let (language, code) = block.split_once('\n').unwrap_or(("", block));
        let rust = language.is_empty() || language.starts_with("rust");
        if rust && has_word(code, name) {
            blocks.push(code.trim_end());
        }
        parts.next();
    }
    blocks
}

/// Whether `text` contains `word` as a whole identifier.
pub fn has_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric() && c != '_').any(|token| token == word)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            let next = (row[j + 1] + 1).min(row[j] + 1).min(diagonal + cost);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_parses() {
        assert!(Index::get().failures.is_empty(), "{:?}", Index::get().failures);
        assert!(RUST_SOURCES.len() > 50, "the build script found the workspace sources");
    }

    #[test]
    fn signatures_are_the_source_text() {
        let button = Index::get().find("Button");
        let new = button[0].methods.iter().find(|m| m.name == "new").expect("Button::new");
        assert!(new.signature.starts_with("pub fn new("), "{}", new.signature);
        assert!(!new.docs.is_empty());
    }

    #[test]
    fn suggests_near_names() {
        assert!(Index::get().suggest("Buton").contains(&"Button"));
    }

    #[test]
    fn code_blocks_skip_other_languages() {
        let text = "```bash\nButton\n```\ntext Button\n```rust\nButton::new(\"a\")\n```\n```\nOther\n```\n";
        assert_eq!(code_blocks_with(text, "Button"), ["Button::new(\"a\")"]);
    }
}
