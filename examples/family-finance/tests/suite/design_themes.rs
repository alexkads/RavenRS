//! The themes of Raven Finanças, written where RavenComponentBook reads them for the design
//! system (`examples/family-finance/design/`), so a screen designed for the app has its colors.
//! A theme changed in `themes.rs` and not written again fails here:
//!
//! ```text
//! FF_WRITE_THEMES=1 cargo test -p family-finance --test suite design_themes::
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::path::Path;

use family_finance::themes;

#[test]
fn the_design_system_has_the_themes_of_the_app() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("design");
    let write = std::env::var_os("FF_WRITE_THEMES").is_some();
    for (file, theme) in [("claro", themes::light()), ("escuro", themes::dark()), ("vidro", themes::glass())]
    {
        let path = folder.join(format!("{file}.theme.json"));
        let json = theme.to_tokens_json() + "\n";
        if write {
            std::fs::create_dir_all(&folder).expect("folder");
            std::fs::write(&path, &json).expect("write");
        }
        let kept = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(kept, json, "{} changed: write it again with FF_WRITE_THEMES=1", path.display());
    }
}
