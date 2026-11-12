//! The RavenRS rules, checked on every `cargo test`. Written by `cargo ravenrs-lint --init`.

#[test]
fn follows_the_ravenrs_rules() {
    ravenrs_lint::assert_clean();
}
