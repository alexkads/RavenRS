//! The rules and the syntax-tree walk that finds them.

use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Lit};

use crate::config::{Config, Language};

/// A rule `ravenrs-lint` checks. Every rule is `deny` unless the configuration lowers it, and
/// lowering one to `allow` needs a reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Lint {
    /// A `Color` value written in the interface (`Color::rgb`, `Color::WHITE`, `Color { .. }`).
    /// It looks right in one theme and wrong in the others. Defining a theme (`Theme { .. }`,
    /// `Palette { .. }` or a function returning one) is where colors belong, so it is left alone.
    RawColor,
    /// `.font_size(..)`: text sizes come from `TextStyle` steps. A printed report's `Style` is
    /// measured in points and left alone.
    RawFontSize,
    /// `.padding`, `.spacing`, `.gap` or `Insets` with a literal that is not a multiple of 4.
    OffGridSpacing,
    /// A `Button` caption that does not start with a verb.
    ButtonWithoutVerb,
    /// An `IconButton` with no `.label(..)`: screen readers would only say the icon's name.
    IconButtonWithoutLabel,
    /// More than one primary `Button` in one function. `Button` is primary unless `.style(..)`
    /// says otherwise.
    ManyPrimaryButtons,
    /// `State::new` inside `body` or `build`, which run on every rebuild and start it over.
    StateInBody,
    /// Files, processes, sleeping or blocking network calls inside `body` or `build`, which
    /// freeze the window on every rebuild.
    BlockingInBody,
    /// A source file longer than `max-file-lines` (600 by default).
    OversizedFile,
    /// A `ravenrs-lint: allow(..)` comment that does not say why.
    AllowWithoutReason,
    /// A `ravenrs-lint: allow(..)` comment naming a rule that does not exist.
    UnknownLint,
}

impl Lint {
    /// Every rule, in the order they are documented.
    pub const ALL: &'static [Lint] = &[
        Lint::RawColor,
        Lint::RawFontSize,
        Lint::OffGridSpacing,
        Lint::ButtonWithoutVerb,
        Lint::IconButtonWithoutLabel,
        Lint::ManyPrimaryButtons,
        Lint::StateInBody,
        Lint::BlockingInBody,
        Lint::OversizedFile,
        Lint::AllowWithoutReason,
        Lint::UnknownLint,
    ];

    /// The name used in diagnostics, configuration and `allow(..)` comments.
    pub fn name(self) -> &'static str {
        match self {
            Lint::RawColor => "raw_color",
            Lint::RawFontSize => "raw_font_size",
            Lint::OffGridSpacing => "off_grid_spacing",
            Lint::ButtonWithoutVerb => "button_without_verb",
            Lint::IconButtonWithoutLabel => "icon_button_without_label",
            Lint::ManyPrimaryButtons => "many_primary_buttons",
            Lint::StateInBody => "state_in_body",
            Lint::BlockingInBody => "blocking_in_body",
            Lint::OversizedFile => "oversized_file",
            Lint::AllowWithoutReason => "allow_without_reason",
            Lint::UnknownLint => "unknown_lint",
        }
    }

    /// The rule called `name`, if there is one.
    pub fn from_name(name: &str) -> Option<Lint> {
        Lint::ALL.iter().copied().find(|lint| lint.name() == name)
    }

    /// One line on what the rule protects.
    pub fn summary(self) -> &'static str {
        match self {
            Lint::RawColor => "colors come from `Semantic` roles, so every theme stays right",
            Lint::RawFontSize => "text sizes come from `TextStyle` steps",
            Lint::OffGridSpacing => "padding, spacing and gaps stay on the 4px grid",
            Lint::ButtonWithoutVerb => "button captions start with a verb",
            Lint::IconButtonWithoutLabel => "icon buttons have a name for screen readers",
            Lint::ManyPrimaryButtons => "one primary action per view",
            Lint::StateInBody => "state lives in the component, not in `body`",
            Lint::BlockingInBody => "`body` and `build` never block",
            Lint::OversizedFile => "source files stay small enough to read",
            Lint::AllowWithoutReason => "every exception says why",
            Lint::UnknownLint => "exceptions name real rules",
        }
    }
}

/// A rule broken at one place, before the configuration and the `allow` comments apply.
#[derive(Debug)]
pub(crate) struct Finding {
    pub lint: Lint,
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
    pub message: String,
    pub help: String,
}

impl Finding {
    fn at(lint: Lint, span: Span, message: impl Into<String>, help: impl Into<String>) -> Self {
        let start = span.start();
        let end = span.end();
        let end_column = if end.line == start.line && end.column > start.column {
            end.column + 1
        } else {
            start.column + 2
        };
        Finding {
            lint,
            line: start.line,
            column: start.column + 1,
            end_column,
            message: message.into(),
            help: help.into(),
        }
    }
}

pub(crate) fn check(file: &syn::File, config: &Config, lines: usize) -> Vec<Finding> {
    let mut walker = Walker {
        config,
        findings: Vec::new(),
        chains: HashMap::new(),
        in_body: 0,
        in_closure: 0,
        primaries: Vec::new(),
        in_theme: 0,
        named: HashSet::new(),
    };
    walker.visit_file(file);
    let mut findings = walker.findings;
    if lines > config.max_file_lines {
        findings.push(Finding {
            lint: Lint::OversizedFile,
            line: 1,
            column: 1,
            end_column: 2,
            message: format!("this file has {lines} lines, more than the {} allowed", config.max_file_lines),
            help:
                "split it into modules by responsibility, such as one per screen, a `model` and a `storage` \
                   module; move each `impl` with its type"
                    .into(),
        });
    }
    findings
}

/// The methods called on one builder, from the expression that made it.
#[derive(Default)]
struct Chain {
    methods: Vec<String>,
    /// `Some(true)` for `.style(ButtonStyle::Primary)`, `Some(false)` for any other style.
    primary_style: Option<bool>,
}

struct Walker<'c> {
    config: &'c Config,
    findings: Vec<Finding>,
    /// Chains keyed by where the expression that starts them begins.
    chains: HashMap<(usize, usize), Chain>,
    /// Inside `fn body(&self)` or `fn build(&self, ..)`.
    in_body: usize,
    /// Closures inside the current function; they run later, not on every rebuild.
    in_closure: usize,
    /// Primary buttons seen in each function being walked.
    primaries: Vec<Vec<Span>>,
    /// Inside a theme being defined (`Theme { .. }`, `Palette { .. }`, or a function returning one),
    /// where literal colors belong.
    in_theme: usize,
    /// Icon buttons a `Tooltip` names, keyed like `chains`.
    named: HashSet<(usize, usize)>,
}

fn key(span: Span) -> (usize, usize) {
    let start = span.start();
    (start.line, start.column)
}

fn is_test_code(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        path.is_ident("test")
            || path.segments.last().is_some_and(|s| s.ident == "test")
            || (path.is_ident("cfg") && attr.parse_args::<syn::Ident>().is_ok_and(|ident| ident == "test"))
    })
}

fn segments(path: &syn::Path) -> Vec<String> {
    path.segments.iter().map(|s| s.ident.to_string()).collect()
}

fn ends_with(path: &syn::Path, tail: &[&str]) -> bool {
    let segments = segments(path);
    segments.len() >= tail.len()
        && segments[segments.len() - tail.len()..].iter().zip(tail).all(|(a, b)| a == b)
}

fn call_path(call: &syn::ExprCall) -> Option<&syn::Path> {
    match &*call.func {
        Expr::Path(path) => Some(&path.path),
        _ => None,
    }
}

/// A number written in the source, with an optional minus sign.
fn literal_number(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Float(value) => value.base10_parse().ok(),
            Lit::Int(value) => value.base10_parse().ok(),
            _ => None,
        },
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            literal_number(&unary.expr).map(|v| -v)
        }
        Expr::Paren(paren) => literal_number(&paren.expr),
        _ => None,
    }
}

impl Walker<'_> {
    fn report(&mut self, lint: Lint, span: Span, message: impl Into<String>, help: impl Into<String>) {
        self.findings.push(Finding::at(lint, span, message, help));
    }

    fn in_rebuild(&self) -> bool {
        self.in_body > 0 && self.in_closure == 0
    }

    fn check_grid(&mut self, what: &str, args: &Punctuated<Expr, syn::Token![,]>) {
        for arg in args {
            let values: Vec<&Expr> = match arg {
                Expr::Tuple(tuple) => tuple.elems.iter().collect(),
                Expr::Array(array) => array.elems.iter().collect(),
                other => vec![other],
            };
            for value in values {
                let Some(number) = literal_number(value) else { continue };
                if number % 4.0 != 0.0 {
                    let near = (number / 4.0).round() * 4.0;
                    self.report(
                        Lint::OffGridSpacing,
                        value.span(),
                        format!("`{what}` of {number} is off the 4px grid"),
                        format!(
                            "use a multiple of 4, such as {near}: screens pad by 32, cards by 24, stacks space by 16, \
                             related items by 8"
                        ),
                    );
                }
            }
        }
    }

    fn check_caption(&mut self, caption: &syn::LitStr) {
        let text = caption.value();
        let Some(first) = text.split_whitespace().next() else { return };
        let word: String = first.chars().filter(|c| c.is_alphabetic()).collect::<String>().to_lowercase();
        if word.is_empty() || is_verb(&word, self.config) {
            return;
        }
        let example = match self.config.language {
            Language::PortugueseBrazil => "\"Salvar alterações\", \"Cadastrar membro\", \"Exportar PDF\"",
            Language::English => "\"Save changes\", \"Add member\", \"Export PDF\"",
        };
        self.report(
            Lint::ButtonWithoutVerb,
            caption.span(),
            format!("button caption \"{text}\" does not start with a verb"),
            format!(
                "say what the button does, starting with a verb: {example}. If \"{first}\" is a verb, add it \
                 to `extra-verbs` in [package.metadata.ravenrs-lint]"
            ),
        );
    }

    fn button(&mut self, call: &syn::ExprCall) {
        let chain = self.chains.get(&key(call.span()));
        let primary = match chain.and_then(|c| c.primary_style) {
            Some(primary) => primary,
            None => !chain.is_some_and(|c| c.methods.iter().any(|m| m == "style")),
        };
        if let Some(Expr::Lit(syn::ExprLit { lit: Lit::Str(caption), .. })) = call.args.first() {
            self.check_caption(caption);
        }
        if !primary {
            return;
        }
        let span = call.span();
        let Some(seen) = self.primaries.last_mut() else { return };
        seen.push(span);
        if seen.len() > 1 {
            let first = seen[0].start().line;
            self.report(
                Lint::ManyPrimaryButtons,
                span,
                format!("a second primary button; the first is on line {first}"),
                "`Button` is primary unless styled: keep one primary action per view and give the others \
                 `.style(ButtonStyle::Secondary)` or `.style(ButtonStyle::Ghost)`",
            );
        }
    }

    fn icon_button(&mut self, call: &syn::ExprCall) {
        let at = key(call.span());
        let labeled = self.named.contains(&at)
            || self.chains.get(&at).is_some_and(|c| c.methods.iter().any(|m| m == "label"));
        if !labeled {
            self.report(
                Lint::IconButtonWithoutLabel,
                call.span(),
                "icon button without a name for screen readers",
                "add `.label(\"...\")` with the action, such as `.label(\"Search\")`, or wrap it in a `Tooltip`; \
                 the icon's name rarely says what it does",
            );
        }
    }

    fn blocking(&mut self, path: &syn::Path) {
        let segments = segments(path);
        let what = if segments.iter().rev().skip(1).any(|s| s == "fs") {
            Some("reads or writes files")
        } else if ends_with(path, &["thread", "sleep"]) || ends_with(path, &["sleep"]) && segments.len() == 1
        {
            Some("sleeps")
        } else if ends_with(path, &["File", "open"]) || ends_with(path, &["File", "create"]) {
            Some("opens a file")
        } else if ends_with(path, &["Command", "new"]) {
            Some("starts a process")
        } else if segments.iter().any(|s| s == "blocking") {
            Some("waits on the network")
        } else if ends_with(path, &["block_on"]) {
            Some("blocks on a future")
        } else {
            None
        };
        if let Some(what) = what {
            self.report(
                Lint::BlockingInBody,
                path.span(),
                format!("`{}` {what} inside a rebuild", segments.join("::")),
                "`body` and `build` run on every rebuild, so this freezes the window each time: load it once \
                 before the window opens or in a background task, keep the result in a `State`, and only read \
                 that state here",
            );
        }
    }

    fn walk_fn(
        &mut self,
        name: &syn::Ident,
        sig: &syn::Signature,
        attrs: &[syn::Attribute],
        f: impl FnOnce(&mut Self),
    ) {
        if is_test_code(attrs) {
            return;
        }
        let rebuild = (name == "body" || name == "build") && sig.receiver().is_some();
        let theme = match &sig.output {
            syn::ReturnType::Type(_, ty) => match &**ty {
                syn::Type::Path(path) => is_theme(&path.path),
                _ => false,
            },
            syn::ReturnType::Default => false,
        };
        let closures = std::mem::take(&mut self.in_closure);
        self.in_body += usize::from(rebuild);
        self.in_theme += usize::from(theme);
        self.primaries.push(Vec::new());
        f(self);
        self.primaries.pop();
        self.in_theme -= usize::from(theme);
        self.in_body -= usize::from(rebuild);
        self.in_closure = closures;
    }
}

impl<'ast> Visit<'ast> for Walker<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !is_test_code(&node.attrs) {
            visit::visit_item_mod(self, node);
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !is_test_code(&node.attrs) {
            visit::visit_item_impl(self, node);
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.walk_fn(&node.sig.ident, &node.sig, &node.attrs, |w| visit::visit_item_fn(w, node));
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.walk_fn(&node.sig.ident, &node.sig, &node.attrs, |w| visit::visit_impl_item_fn(w, node));
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        self.in_closure += 1;
        visit::visit_expr_closure(self, node);
        self.in_closure -= 1;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // The outermost call of a chain is visited first, so it records the whole chain.
        let mut methods = Vec::new();
        let mut primary_style = None;
        let mut root: &Expr = &node.receiver;
        methods.push(node.method.to_string());
        if node.method == "style" {
            primary_style = style_is_primary(node.args.first());
        }
        while let Expr::MethodCall(inner) = root {
            methods.push(inner.method.to_string());
            if inner.method == "style" && primary_style.is_none() {
                primary_style = style_is_primary(inner.args.first());
            }
            root = &inner.receiver;
        }
        // A printed report's `Style` is measured in points, not the interface.
        let mut report_style = false;
        if let Expr::Call(call) = root {
            report_style = call_path(call).is_some_and(|path| ends_with(path, &["Style", "new"]));
            self.chains.entry(key(root.span())).or_insert(Chain { methods, primary_style });
        }

        let method = node.method.to_string();
        match method.as_str() {
            _ if report_style => {}
            "font_size" => self.report(
                Lint::RawFontSize,
                node.method.span(),
                "literal font size in the interface",
                "use a `TextStyle` step, such as `.style(TextStyle::Title)`, `Body`, `Callout` or `Caption`, so \
                 sizes stay consistent and follow the theme",
            ),
            "padding" | "padding_x" | "padding_y" | "spacing" | "gap" | "row_spacing" | "column_spacing" => {
                self.check_grid(&method, &node.args);
            }
            _ => {}
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(path) = call_path(node) {
            if ends_with(path, &["Tooltip", "new"])
                && let Some(mut inner) = node.args.first()
            {
                while let Expr::MethodCall(call) = inner {
                    inner = &call.receiver;
                }
                self.named.insert(key(inner.span()));
            }
            if ends_with(path, &["Button", "new"]) {
                self.button(node);
            } else if ends_with(path, &["IconButton", "new"]) {
                self.icon_button(node);
            } else if ends_with(path, &["Insets", "uniform"]) || ends_with(path, &["Insets", "symmetric"]) {
                self.check_grid("Insets", &node.args);
            }
            if self.in_rebuild() {
                if ends_with(path, &["State", "new"]) {
                    self.report(
                        Lint::StateInBody,
                        path.span(),
                        "`State::new` inside a rebuild",
                        "`body` runs on every rebuild, so this state starts over each time and loses what the \
                         user did: keep the `State<T>` in a field of the component and clone it here, or use \
                         `cx.use_state` in a custom `View`",
                    );
                }
                self.blocking(path);
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        let segments = segments(&node.path);
        if self.in_theme == 0 && segments.len() >= 2 && segments[segments.len() - 2] == "Color" {
            self.report(
                Lint::RawColor,
                node.span(),
                "literal color in the interface",
                "use a `Semantic` role, such as `.foreground(Semantic::InkMuted)`, `.background(Semantic::Control)` \
                 or `cx.theme().color(Semantic::Accent)`, so it stays right in Dark, Light and Glass",
            );
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let theme = is_theme(&node.path);
        self.in_theme += usize::from(theme);
        if self.in_theme == 0 && node.path.segments.last().is_some_and(|s| s.ident == "Color") {
            self.report(
                Lint::RawColor,
                node.path.span(),
                "literal color in the interface",
                "use a `Semantic` role, such as `.foreground(Semantic::InkMuted)`, so it stays right in every theme",
            );
        }
        visit::visit_expr_struct(self, node);
        self.in_theme -= usize::from(theme);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        // `vec![..]`, `format!(..)` and most view macros hold expressions separated by commas.
        let parsed = node.parse_body_with(Punctuated::<Expr, syn::Token![,]>::parse_terminated);
        if let Ok(exprs) = parsed {
            for expr in &exprs {
                self.visit_expr(expr);
            }
        }
        visit::visit_macro(self, node);
    }
}

/// A theme or its palette, where literal colors are defined.
fn is_theme(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|s| s.ident == "Theme" || s.ident == "Palette")
}

fn style_is_primary(arg: Option<&Expr>) -> Option<bool> {
    match arg? {
        Expr::Path(path) => Some(path.path.segments.last().is_some_and(|s| s.ident == "Primary")),
        _ => Some(false),
    }
}

/// Imperative verbs that start English button captions.
const ENGLISH_VERBS: &[&str] = &[
    "accept",
    "activate",
    "add",
    "allow",
    "apply",
    "approve",
    "archive",
    "ask",
    "assign",
    "attach",
    "back",
    "block",
    "book",
    "browse",
    "build",
    "buy",
    "calculate",
    "call",
    "cancel",
    "change",
    "check",
    "choose",
    "clear",
    "click",
    "clone",
    "close",
    "collapse",
    "compare",
    "complete",
    "configure",
    "confirm",
    "connect",
    "continue",
    "convert",
    "copy",
    "create",
    "crop",
    "cut",
    "decline",
    "delete",
    "deny",
    "deploy",
    "describe",
    "disable",
    "disconnect",
    "discard",
    "dismiss",
    "do",
    "download",
    "drag",
    "draw",
    "drop",
    "duplicate",
    "edit",
    "email",
    "empty",
    "enable",
    "end",
    "enter",
    "erase",
    "exit",
    "expand",
    "explore",
    "export",
    "fetch",
    "fill",
    "filter",
    "find",
    "finish",
    "fix",
    "flag",
    "follow",
    "forget",
    "format",
    "forward",
    "generate",
    "get",
    "give",
    "go",
    "grant",
    "group",
    "hide",
    "highlight",
    "import",
    "insert",
    "install",
    "invite",
    "join",
    "keep",
    "launch",
    "learn",
    "leave",
    "like",
    "link",
    "list",
    "load",
    "lock",
    "log",
    "login",
    "logout",
    "make",
    "manage",
    "mark",
    "merge",
    "minimize",
    "move",
    "mute",
    "name",
    "new",
    "next",
    "notify",
    "ok",
    "open",
    "order",
    "pause",
    "paste",
    "pay",
    "pick",
    "pin",
    "place",
    "play",
    "post",
    "preview",
    "previous",
    "print",
    "publish",
    "pull",
    "push",
    "quit",
    "rate",
    "read",
    "record",
    "redo",
    "refresh",
    "register",
    "reject",
    "reload",
    "remind",
    "remove",
    "rename",
    "reopen",
    "reorder",
    "repeat",
    "replace",
    "reply",
    "report",
    "request",
    "reset",
    "resize",
    "restart",
    "restore",
    "resume",
    "retry",
    "return",
    "review",
    "revert",
    "rotate",
    "run",
    "save",
    "scan",
    "schedule",
    "search",
    "see",
    "select",
    "send",
    "set",
    "share",
    "show",
    "sign",
    "skip",
    "sort",
    "split",
    "start",
    "stop",
    "submit",
    "subscribe",
    "swap",
    "switch",
    "sync",
    "take",
    "tag",
    "test",
    "toggle",
    "track",
    "transfer",
    "translate",
    "try",
    "turn",
    "undo",
    "unlock",
    "unmute",
    "unpin",
    "unsubscribe",
    "update",
    "upgrade",
    "upload",
    "use",
    "validate",
    "verify",
    "view",
    "visit",
    "vote",
    "watch",
    "write",
    "zoom",
];

/// Portuguese nouns and adjectives that end like an infinitive.
const PORTUGUESE_NOT_VERBS: &[&str] = &[
    "altar",
    "amor",
    "ar",
    "autor",
    "bar",
    "calor",
    "celular",
    "cor",
    "credor",
    "devedor",
    "doutor",
    "escolar",
    "familiar",
    "favor",
    "flor",
    "interior",
    "lar",
    "lugar",
    "maior",
    "mar",
    "melhor",
    "menor",
    "mulher",
    "pastor",
    "par",
    "pior",
    "popular",
    "professor",
    "regular",
    "senhor",
    "setor",
    "similar",
    "superior",
    "titular",
    "valor",
    "vapor",
    "exterior",
    "anterior",
    "posterior",
    "inferior",
    "colher",
    "prazer",
    "lazer",
    "auxiliar",
    "militar",
    "solar",
    "primar",
    "dólar",
    "açúcar",
    "radar",
    "cantor",
    "leitor",
    "tutor",
];

fn is_verb(word: &str, config: &Config) -> bool {
    if config.extra_verbs.iter().any(|verb| verb.to_lowercase() == word) {
        return true;
    }
    match config.language {
        Language::English => ENGLISH_VERBS.contains(&word),
        Language::PortugueseBrazil => {
            // Infinitives end in -ar, -er, -ir, -or or -ôr (pôr and its family).
            ["ar", "er", "ir", "or", "ôr"].iter().any(|ending| word.ends_with(ending))
                && !PORTUGUESE_NOT_VERBS.contains(&word)
        }
    }
}

/// The `ravenrs-lint: allow(..)` comments of one file.
#[derive(Default)]
pub(crate) struct Allowances {
    whole_file: HashSet<Lint>,
    by_line: HashMap<usize, HashSet<Lint>>,
}

struct AllowComment<'a> {
    line: usize,
    column: usize,
    whole_file: bool,
    names: Vec<&'a str>,
    reason: &'a str,
    /// The comment has the line to itself, so it applies to the next line of code.
    own_line: bool,
}

const MARKER: &str = "ravenrs-lint:";

fn allow_comments(source: &str) -> Vec<AllowComment<'_>> {
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let Some(comment_at) = line.find("//") else { continue };
        let comment = &line[comment_at..];
        let Some(marker_at) = comment.find(MARKER) else { continue };
        let rest = comment[marker_at + MARKER.len()..].trim_start();
        let (whole_file, rest) = if let Some(rest) = rest.strip_prefix("allow-file(") {
            (true, rest)
        } else if let Some(rest) = rest.strip_prefix("allow(") {
            (false, rest)
        } else {
            continue;
        };
        let Some(close) = rest.find(')') else { continue };
        let names = rest[..close].split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
        let reason = rest[close + 1..].trim().trim_start_matches(['-', '—', ':', '–']).trim();
        found.push(AllowComment {
            line: index + 1,
            column: comment_at + marker_at + 1,
            whole_file,
            names,
            reason,
            own_line: line[..comment_at].trim().is_empty(),
        });
    }
    found
}

impl Allowances {
    pub(crate) fn read(source: &str) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let mut allowances = Allowances::default();
        for comment in allow_comments(source) {
            let lints = comment.names.iter().filter_map(|name| Lint::from_name(name));
            if comment.whole_file {
                allowances.whole_file.extend(lints);
                continue;
            }
            let mut target = comment.line;
            if comment.own_line {
                // The next line that is not blank and not only a comment.
                target = (comment.line..lines.len())
                    .find(|&i| {
                        let text = lines[i].trim();
                        !text.is_empty() && !text.starts_with("//")
                    })
                    .map_or(comment.line, |i| i + 1);
            }
            allowances.by_line.entry(target).or_default().extend(lints);
        }
        allowances
    }

    pub(crate) fn covers(&self, lint: Lint, line: usize) -> bool {
        self.whole_file.contains(&lint) || self.by_line.get(&line).is_some_and(|lints| lints.contains(&lint))
    }
}

pub(crate) fn check_allow_comments(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for comment in allow_comments(source) {
        let at = |lint, message: String, help: &str| Finding {
            lint,
            line: comment.line,
            column: comment.column,
            end_column: comment.column + MARKER.len(),
            message,
            help: help.to_owned(),
        };
        for name in &comment.names {
            if Lint::from_name(name).is_none() {
                let known: Vec<&str> = Lint::ALL.iter().map(|l| l.name()).collect();
                findings.push(at(
                    Lint::UnknownLint,
                    format!("unknown rule `{name}`"),
                    &format!("the rules are: {}", known.join(", ")),
                ));
            }
        }
        if comment.reason.is_empty() {
            findings.push(at(
                Lint::AllowWithoutReason,
                "exception without a reason".into(),
                "fix the code instead; when the exception is right, say why after the parenthesis, such as \
                 `// ravenrs-lint: allow(raw_color) the logo keeps its brand color in every theme`",
            ));
        }
    }
    findings
}
