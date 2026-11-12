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
