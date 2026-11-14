//! The directories that differ between worktrees, and how they are taken out of a compilation.
//!
//! rustc writes the paths it sees into what it produces, and hashes them into the crate's identity,
//! so the same crate built under two directories is two different artifacts that do not even mix
//! (error E0460). With `--remap-path-prefix` for the build directory and the working directory the
//! artifacts come out byte for byte the same. Whatever still carries the real path afterwards is
//! data the crate embedded, such as `env!("OUT_DIR")` in a constant, and cannot be shared.

use std::path::Path;

/// One directory that changes from worktree to worktree.
#[derive(Debug, Clone)]
pub(crate) struct Root {
    /// The path on this machine.
    pub real: String,
    /// What stands for it in a cache key.
    pub marker: &'static str,
    /// What rustc is told to write in its place.
    pub remapped: &'static str,
}

/// The roots of one compilation, longest first.
#[derive(Debug, Clone, Default)]
pub(crate) struct Roots(pub Vec<Root>);

impl Roots {
    /// The build directory `out_dir` is in, and the working directory unless it is inside Cargo's
    /// home, where registry crates live at the same path for every worktree.
    pub(crate) fn find(out_dir: &Path, cwd: &Path, cargo_home: Option<&Path>) -> Roots {
        let mut roots = Vec::new();
        let target =
            out_dir.ancestors().find(|folder| folder.join("CACHEDIR.TAG").is_file()).unwrap_or(out_dir);
        if let Some(real) = target.to_str() {
            roots.push(Root { real: real.to_owned(), marker: "<xcache:target>", remapped: "/xcache/target" });
        }
        let in_cargo_home = cargo_home.is_some_and(|home| cwd.starts_with(home));
        if let (false, Some(real)) = (in_cargo_home, cwd.to_str())
            && roots.iter().all(|root| root.real != real)
        {
            roots.push(Root { real: real.to_owned(), marker: "<xcache:cwd>", remapped: "/xcache/cwd" });
        }
        roots.sort_by_key(|root| std::cmp::Reverse(root.real.len()));
        Roots(roots)
    }

    /// `text` with each root replaced by its marker, where the root is a whole path prefix.
    pub(crate) fn norm(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for root in &self.0 {
            text = replace_prefix(&text, &root.real, root.marker);
        }
        text
    }

    /// The reverse of [`Roots::norm`], for the roots of this compilation.
    pub(crate) fn denorm(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for root in &self.0 {
            text = text.replace(root.marker, &root.real);
        }
        text
    }

    /// The `--remap-path-prefix` arguments, or why they cannot be given. rustc lets the last match
    /// win, so the longest root goes last.
    pub(crate) fn remap_args(&self) -> Result<Vec<String>, String> {
        let mut args = Vec::new();
        for root in self.0.iter().rev() {
            if root.real.contains('=') {
                return Err(format!("the path {} has an = in it", root.real));
            }
            args.push(format!("--remap-path-prefix={}={}", root.real, root.remapped));
        }
        Ok(args)
    }

    /// The real paths, as one line: what a result that embeds them is tied to.
    pub(crate) fn raw(&self) -> String {
        self.0.iter().map(|root| root.real.as_str()).collect::<Vec<_>>().join("|")
    }

    /// The first root whose real path is in `bytes`.
    pub(crate) fn leaked_in(&self, bytes: &[u8]) -> Option<&str> {
        self.0.iter().find(|root| contains(bytes, root.real.as_bytes())).map(|root| root.real.as_str())
    }
}

/// Replaces `prefix` by `with` where what follows cannot be more of the same path component, so
/// `/work/app` does not match inside `/work/app2`.
fn replace_prefix(text: &str, prefix: &str, with: &str) -> String {
    if prefix.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(prefix) {
        let after = &rest[at + prefix.len()..];
        let whole =
            after.chars().next().is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')));
        out.push_str(&rest[..at]);
        out.push_str(if whole { with } else { prefix });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Whether `needle` is in `haystack` (Boyer-Moore-Horspool: artifacts are large, paths are long).
pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let size = needle.len();
    if size == 0 || haystack.len() < size {
        return size == 0;
    }
    let mut skip = [size; 256];
    for (index, byte) in needle[..size - 1].iter().enumerate() {
        skip[usize::from(*byte)] = size - 1 - index;
    }
    let mut at = 0;
    while at + size <= haystack.len() {
        if &haystack[at..at + size] == needle {
            return true;
        }
        at += skip[usize::from(haystack[at + size - 1])];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> Roots {
        Roots(vec![
            Root { real: "/work/app/target".into(), marker: "<xcache:target>", remapped: "/xcache/target" },
            Root { real: "/work/app".into(), marker: "<xcache:cwd>", remapped: "/xcache/cwd" },
        ])
    }

    #[test]
    fn roots_become_markers_and_come_back() {
        let roots = roots();
        let text = "OUT_DIR=/work/app/target/debug/build/x/out src=/work/app/crates/x/src/lib.rs";
        let normal = roots.norm(text);
        assert_eq!(normal, "OUT_DIR=<xcache:target>/debug/build/x/out src=<xcache:cwd>/crates/x/src/lib.rs");
        assert_eq!(roots.denorm(&normal), text);
    }

    #[test]
    fn a_longer_folder_name_is_another_folder() {
        let roots = roots();
        assert_eq!(roots.norm("/work/app2/src"), "/work/app2/src");
        assert_eq!(roots.norm("/work/app"), "<xcache:cwd>");
        assert_eq!(roots.norm("\"/work/app\""), "\"<xcache:cwd>\"");
    }

    #[test]
    fn the_longest_root_is_remapped_last_so_it_wins() {
        let args = roots().remap_args().unwrap_or_default();
        assert_eq!(
            args,
            [
                "--remap-path-prefix=/work/app=/xcache/cwd",
                "--remap-path-prefix=/work/app/target=/xcache/target",
            ]
        );
        let odd = Roots(vec![Root { real: "/a=b".into(), marker: "<xcache:cwd>", remapped: "/xcache/cwd" }]);
        assert!(odd.remap_args().is_err());
    }

    #[test]
    fn a_path_left_in_the_bytes_is_found() {
        let roots = roots();
        assert_eq!(roots.leaked_in(b"\0\0/work/app/target/debug/build/x/out\0"), Some("/work/app/target"));
        assert_eq!(roots.leaked_in(b"\0/xcache/target/debug\0/xcache/cwd/src\0"), None);
        assert!(contains(b"abcabcabd", b"abcabd"));
        assert!(!contains(b"abcabcabc", b"abcabd"));
        assert!(!contains(b"ab", b"abc"));
    }

    #[test]
    fn registry_crates_keep_their_working_directory() {
        let folder = std::env::temp_dir().join(format!("xcache-roots-{}", std::process::id()));
        let out = folder.join("target/debug/deps");
        std::fs::create_dir_all(&out).unwrap_or_default();
        std::fs::write(folder.join("target/CACHEDIR.TAG"), "").unwrap_or_default();
        let home = folder.join("cargo-home");
        let member = Roots::find(&out, &folder.join("ws"), Some(&home));
        assert_eq!(member.0.len(), 2);
        let registry = Roots::find(&out, &home.join("registry/src/x/serde-1.0.0"), Some(&home));
        assert_eq!(registry.0.len(), 1);
        assert_eq!(registry.0[0].real, folder.join("target").to_string_lossy());
        std::fs::remove_dir_all(&folder).unwrap_or_default();
    }
}
