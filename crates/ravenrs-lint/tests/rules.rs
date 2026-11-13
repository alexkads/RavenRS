//! Each rule, with code that breaks it and code that follows it.

use std::path::Path;

use ravenrs_lint::{Config, Language, Level, Lint, check_source};

fn lints_in(source: &str, config: &Config) -> Vec<Lint> {
    match check_source(Path::new("src/ui.rs"), source, config) {
        Ok(found) => found.into_iter().map(|d| d.lint).collect(),
        Err(error) => panic!("{error}"),
    }
}

fn lints(source: &str) -> Vec<Lint> {
    lints_in(source, &Config::default())
}

fn portuguese() -> Config {
    let mut config = Config::default();
    config.language = Language::PortugueseBrazil;
    config
}

#[test]
fn literal_colors_are_caught_and_semantic_roles_pass() {
    assert_eq!(lints("fn v() { Text::new(\"a\").foreground(Color::rgb(1.0, 0.0, 0.0)); }"), [Lint::RawColor]);
    assert_eq!(lints("fn v() { x.background(ravenrs::Color::WHITE); }"), [Lint::RawColor]);
    assert_eq!(lints("fn v() { let c = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }; }"), [Lint::RawColor]);
    assert!(lints("fn v() { Text::new(\"a\").foreground(Semantic::InkMuted); }").is_empty());
    // Defining a theme is where colors belong.
    assert!(
        lints("fn light() -> Theme { Theme { palette: Palette { bg: Color::hex(0xffffff) } } }").is_empty()
    );
    assert!(lints("fn v() { let t = Theme { palette: Palette { bg: Color::hex(0xffffff) } }; }").is_empty());
    assert!(lints("fn v() { ImageData::from_fn(8, 8, |x, y| Color::rgb(0.2, 0.3, 0.4)); }").is_empty());
}

#[test]
fn font_sizes_are_caught_and_text_styles_pass() {
    assert_eq!(lints("fn v() { Text::new(\"a\").font_size(17.0); }"), [Lint::RawFontSize]);
    assert!(lints("fn v() { Text::new(\"a\").style(TextStyle::Title); }").is_empty());
    // A printed report measures in points.
    assert!(lints("fn r() { Style::new().font_size(9.0).padding(3.0); }").is_empty());
}

#[test]
fn spacing_off_the_grid_is_caught() {
    assert_eq!(lints("fn v() { VStack::new(()).spacing(10.0); }"), [Lint::OffGridSpacing]);
    assert_eq!(lints("fn v() { VStack::new(()).padding((8.0, 6.0)); }"), [Lint::OffGridSpacing]);
    assert_eq!(lints("fn v() { let i = Insets::symmetric(12.0, 5.0); }"), [Lint::OffGridSpacing]);
    assert!(lints("fn v() { VStack::new(()).spacing(16.0).padding(32.0).gap(0.0); }").is_empty());
    // A value computed elsewhere is not a literal and is left alone.
    assert!(lints("fn v(gap: f32) { VStack::new(()).spacing(gap); }").is_empty());
}

#[test]
fn captions_start_with_a_verb() {
    let caption = |text: &str, config: &Config| {
        lints_in(&format!("fn v() {{ Button::new(\"{text}\").style(ButtonStyle::Ghost); }}"), config)
    };
    assert!(caption("Save changes", &Config::default()).is_empty());
    assert_eq!(caption("Settings", &Config::default()), [Lint::ButtonWithoutVerb]);
    assert!(caption("Cadastrar membro", &portuguese()).is_empty());
    assert!(caption("Exportar PDF", &portuguese()).is_empty());
    assert_eq!(caption("Novo membro", &portuguese()), [Lint::ButtonWithoutVerb]);
    // Ends like an infinitive, but is a noun.
    assert_eq!(caption("Pastor", &portuguese()), [Lint::ButtonWithoutVerb]);
    let mut ok = Config::default();
    ok.extra_verbs = vec!["Ok".into()];
    assert!(caption("OK", &ok).is_empty());
}

#[test]
fn icon_buttons_need_a_label() {
    assert_eq!(
        lints("fn v() { IconButton::new(IconName::Search).on_click(|| {}); }"),
        [Lint::IconButtonWithoutLabel]
    );
    assert!(
        lints("fn v() { IconButton::new(IconName::Search).label(\"Search\").on_click(|| {}); }").is_empty()
    );
    assert!(
        lints("fn v() { Tooltip::new(IconButton::new(IconName::Search).on_click(|| {}), \"Search\"); }")
            .is_empty()
    );
}

#[test]
fn one_primary_button_per_function() {
    let two = "fn v() { (Button::new(\"Save\"), Button::new(\"Cancel\")); }";
    assert_eq!(lints(two), [Lint::ManyPrimaryButtons]);
    let styled = "fn v() { (Button::new(\"Save\"), Button::new(\"Cancel\").style(ButtonStyle::Secondary)); }";
    assert!(lints(styled).is_empty());
    let explicit = "fn v() { (Button::new(\"Save\"), Button::new(\"Send\").style(ButtonStyle::Primary)); }";
    assert_eq!(lints(explicit), [Lint::ManyPrimaryButtons]);
    // Separate functions are separate views.
    assert!(lints("fn a() { Button::new(\"Save\"); } fn b() { Button::new(\"Send\"); }").is_empty());
}

#[test]
fn buttons_inside_macros_are_seen() {
    assert_eq!(
        lints("fn v() { vec![Button::new(\"Save\"), Button::new(\"Send\")]; }"),
        [Lint::ManyPrimaryButtons]
    );
}

#[test]
fn state_belongs_in_the_component() {
    let inside =
        "impl Component for A { fn body(&self) -> impl View { let n = State::new(0); Text::new(\"\") } }";
    assert_eq!(lints(inside), [Lint::StateInBody]);
    let outside = "fn main() { let app = A { n: State::new(0) }; }";
    assert!(lints(outside).is_empty());
    // A closure runs later, not on every rebuild.
    let closure = "impl Component for A { fn body(&self) -> impl View { Button::new(\"Add\").on_click(|| { State::new(0); }) } }";
    assert!(lints(closure).is_empty());
}

#[test]
fn body_never_blocks() {
    let reads = "impl Component for A { fn body(&self) -> impl View { let t = std::fs::read_to_string(\"a\"); Text::new(\"\") } }";
    assert_eq!(lints(reads), [Lint::BlockingInBody]);
    let sleeps =
        "impl View for A { fn build(&self, cx: &mut BuildCx) -> NodeId { std::thread::sleep(d); todo() } }";
    assert_eq!(lints(sleeps), [Lint::BlockingInBody]);
    assert!(lints("fn load() { let t = std::fs::read_to_string(\"a\"); }").is_empty());
}

#[test]
fn test_code_is_not_checked() {
    assert!(lints("#[cfg(test)] mod tests { fn v() { x.foreground(Color::WHITE); } }").is_empty());
    assert!(lints("#[test] fn v() { x.font_size(3.0); }").is_empty());
}

#[test]
fn long_files_are_caught() {
    let mut config = Config::default();
    config.max_file_lines = 3;
    assert_eq!(lints_in("fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n", &config), [Lint::OversizedFile]);
    assert!(lints_in("fn a() {}\nfn b() {}\n", &config).is_empty());
}

#[test]
fn tests_do_not_make_a_file_long() {
    let mut config = Config::default();
    config.max_file_lines = 3;
    let tested = "fn a() {}\nfn b() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn a() {}\n}\n";
    assert!(lints_in(tested, &config).is_empty());
    let long = "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n#[cfg(test)]\nmod tests {}\n";
    assert_eq!(lints_in(long, &config), [Lint::OversizedFile]);
}

#[test]
fn an_exception_needs_a_reason() {
    let excused = "fn v() {\n    // ravenrs-lint: allow(raw_color) the logo keeps its brand color\n    x.foreground(Color::WHITE);\n}";
    assert!(lints(excused).is_empty());
    let same_line = "fn v() { x.foreground(Color::WHITE); } // ravenrs-lint: allow(raw_color) brand color";
    assert!(lints(same_line).is_empty());
    let bare = "fn v() {\n    // ravenrs-lint: allow(raw_color)\n    x.foreground(Color::WHITE);\n}";
    assert_eq!(lints(bare), [Lint::AllowWithoutReason]);
    let typo = "// ravenrs-lint: allow-file(raw_colour) brand colors\nfn v() {}";
    assert_eq!(lints(typo), [Lint::UnknownLint]);
    let file = "// ravenrs-lint: allow-file(raw_color) this module is the brand palette\nfn v() { x.foreground(Color::WHITE); }";
    assert!(lints(file).is_empty());
}

#[test]
fn every_rule_is_denied_by_default() {
    let config = Config::default();
    assert!(Lint::ALL.iter().all(|&lint| config.level(lint) == Level::Deny));
}

#[test]
fn lowering_a_rule_needs_a_reason() {
    let table = |levels: serde_json::Value| serde_json::json!({ "lints": levels });
    let bare = Config::from_metadata(None, Some(&table(serde_json::json!({ "raw_color": "allow" }))));
    assert!(bare.is_err());
    let unknown = Config::from_metadata(None, Some(&table(serde_json::json!({ "raw_colour": "deny" }))));
    assert!(unknown.is_err());
    let reasoned = serde_json::json!({ "raw_color": { "level": "warn", "reason": "the brand palette is being moved to tokens" } });
    let Ok(config) = Config::from_metadata(None, Some(&table(reasoned))) else { panic!("rejected a reason") };
    assert_eq!(config.level(Lint::RawColor), Level::Warn);
}

#[test]
fn the_package_overrides_the_workspace() {
    let workspace = serde_json::json!({ "language": "pt-BR", "max-file-lines": 400 });
    let package = serde_json::json!({ "max-file-lines": 300 });
    let Ok(config) = Config::from_metadata(Some(&workspace), Some(&package)) else { panic!("rejected") };
    assert_eq!(config.language, Language::PortugueseBrazil);
    assert_eq!(config.max_file_lines, 300);
}

#[test]
fn diagnostics_read_like_the_compiler() {
    let source = "fn v() {\n    x.foreground(Color::WHITE);\n}\n";
    let Ok(found) = check_source(Path::new("src/ui.rs"), source, &Config::default()) else { panic!() };
    let text = ravenrs_lint::render(&found, true);
    assert!(text.contains("error[ravenrs::raw_color]: literal color in the interface"), "{text}");
    assert!(text.contains("--> src/ui.rs:2:18"), "{text}");
    assert!(text.contains("2 |     x.foreground(Color::WHITE);"), "{text}");
    assert!(text.contains("  |                  ^^^^^^^^^^^^"), "{text}");
    assert!(text.contains("= help: use a `Semantic` role"), "{text}");
    assert!(text.contains("found 1 error and 0 warnings"), "{text}");
    let json = ravenrs_lint::render_json(&found);
    assert!(json.contains("\"lint\":\"ravenrs::raw_color\""), "{json}");
}

/// A crate written to a temporary folder: `files` are paths under `src/` and their text.
fn crate_with(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ravenrs-lint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let path = dir.join("src").join(path);
        let folder = path.parent().unwrap_or(&dir);
        if let Err(error) = std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, text)) {
            panic!("could not write {}: {error}", path.display());
        }
    }
    dir
}

fn module_findings(dir: &Path, config: &Config) -> Vec<(String, usize, String)> {
    let found = match ravenrs_lint::check_modules(&dir.join("src/lib.rs"), dir, config) {
        Ok(found) => found,
        Err(error) => panic!("{error}"),
    };
    let _ = std::fs::remove_dir_all(dir);
    found.into_iter().map(|d| (d.file.display().to_string(), d.line, d.message)).collect()
}

const THREE: &[(&str, &str)] = &[
    ("lib.rs", "mod a;\nmod b;\nmod c;\n"),
    ("a.rs", "use crate::b::B;\npub struct A;\n"),
    ("b.rs", "pub struct B;\nfn f() { let _ = crate::c::C; }\n"),
    ("c.rs", "\n\nuse crate::a::A;\npub struct C;\n"),
];

#[test]
fn a_pair_of_modules_that_use_each_other_is_allowed() {
    let dir = crate_with(
        "pair",
        &[
            ("lib.rs", "mod a;\nmod b;\n"),
            ("a.rs", "use crate::b::B;\npub struct A;\n"),
            ("b.rs", "use super::a::A;\n"),
        ],
    );
    assert!(module_findings(&dir, &Config::default()).is_empty());
}

#[test]
fn a_loop_of_three_modules_is_caught_at_the_use_that_closes_it() {
    let found = module_findings(&crate_with("three", THREE), &Config::default());
    assert_eq!(found.len(), 1, "{found:?}");
    let (file, line, message) = &found[0];
    assert!(message.starts_with("3 modules use each other in a loop"), "{message}");
    assert!(message.contains("a → b → c → a"), "{message}");
    assert_eq!((file.as_str(), *line), ("src/a.rs", 1));
}

#[test]
fn an_allow_comment_on_the_use_takes_that_edge_out() {
    let mut files = THREE.to_vec();
    files[3] =
        ("c.rs", "// ravenrs-lint: allow(module_cycle) c reads A only to print it\nuse crate::a::A;\n");
    assert!(module_findings(&crate_with("allowed", &files), &Config::default()).is_empty());
}

#[test]
fn the_limit_holds_a_crate_being_untangled_and_asks_to_come_down() {
    let mut config = Config::default();
    config.max_module_cycle = 3;
    assert!(module_findings(&crate_with("held", THREE), &config).is_empty());
    config.max_module_cycle = 5;
    let found = module_findings(&crate_with("lower", THREE), &config);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].0.as_str(), found[0].1), ("src/lib.rs", 1));
    assert!(found[0].2.contains("now has 3, fewer than the 5"), "{}", found[0].2);
}

#[test]
fn reexports_count_and_test_code_does_not() {
    // `crate::B` is `b::B` through the root's re-export; the test module's use of `c` is left out.
    let dir = crate_with(
        "reexports",
        &[
            ("lib.rs", "mod a;\nmod b;\nmod c;\npub use b::B;\n"),
            ("a.rs", "fn f() -> crate::B { todo!() }\n#[cfg(test)]\nmod tests { use crate::c::C; }\n"),
            ("b.rs", "pub struct B;\nuse crate::c::C;\n"),
            ("c.rs", "pub struct C;\nuse crate::a;\n"),
        ],
    );
    let found = module_findings(&dir, &Config::default());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].2.contains("a → b → c → a"), "{}", found[0].2);
    let dir = crate_with(
        "test-only",
        &[
            ("lib.rs", "mod a;\nmod b;\nmod c;\n"),
            ("a.rs", "use crate::b::B;\n"),
            ("b.rs", "pub struct B;\nuse crate::c::C;\n"),
            ("c.rs", "pub struct C;\n#[cfg(test)]\nmod tests { use crate::a; }\n"),
        ],
    );
    assert!(module_findings(&dir, &Config::default()).is_empty());
}

#[test]
fn super_inside_an_inline_module_is_its_parent_not_the_root() {
    // In `a.rs`, `mod inner { use super::c; }` names `a::c`, a function, not the module `c`.
    let dir = crate_with(
        "inline-super",
        &[
            ("lib.rs", "mod a;\nmod b;\nmod c;\n"),
            ("a.rs", "use crate::b::B;\nfn c() {}\nmod inner { use super::c; }\n"),
            ("b.rs", "pub struct B;\nuse crate::c::C;\n"),
            ("c.rs", "pub struct C;\n"),
        ],
    );
    assert!(module_findings(&dir, &Config::default()).is_empty());
}

#[test]
fn the_module_cycle_rule_is_listed_and_configured() {
    assert_eq!(Lint::from_name("module_cycle"), Some(Lint::ModuleCycle));
    let config =
        Config::from_metadata(None, Some(&serde_json::json!({ "max-module-cycle": 41 }))).expect("valid");
    assert_eq!(config.max_module_cycle, 41);
    assert!(Config::from_metadata(None, Some(&serde_json::json!({ "max-module-cycle": "many" }))).is_err());
}
