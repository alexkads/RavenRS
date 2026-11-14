//! The wrapper in front of the real rustc: what is shared between build directories, and what is
//! not allowed to be.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A place with its own cache and as many "worktrees" as a test asks for, each with a checkout and
/// a build directory.
struct Bench {
    root: PathBuf,
    rustc: PathBuf,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let root = std::env::temp_dir().join(format!("xcache-it-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let sysroot = Command::new("rustc").args(["--print", "sysroot"]).output().unwrap();
        let rustc =
            PathBuf::from(String::from_utf8(sysroot.stdout).unwrap().trim()).join("bin").join("rustc");
        Bench { root: fs::canonicalize(root).unwrap(), rustc }
    }

    /// A worktree: `<name>/ws` is the checkout, `<name>/target` the build directory.
    fn tree(&self, name: &str) -> PathBuf {
        let tree = self.root.join(name);
        fs::create_dir_all(tree.join("ws/src")).unwrap();
        fs::create_dir_all(tree.join("target/debug/deps")).unwrap();
        fs::create_dir_all(tree.join("target/debug/build/demo/out")).unwrap();
        // No CACHEDIR.TAG: a script that makes the build directory before Cargo leaves it without one.
        tree
    }

    /// Compiles `ws/src/<source>` of a tree as Cargo would, through the wrapper.
    fn compile(&self, tree: &Path, crate_name: &str, source: &str, extra: &[&str]) -> Output {
        let deps = tree.join("target/debug/deps");
        Command::new(env!("CARGO_BIN_EXE_ravenrs-xcache"))
            .arg(&self.rustc)
            .args(["--crate-name", crate_name, "--edition=2024"])
            .arg(format!("src/{source}"))
            .args(["--error-format=json", "--json=diagnostic-rendered-ansi,artifacts,future-incompat"])
            .args([
                "--emit=dep-info,metadata,link",
                "-C",
                "embed-bitcode=no",
                "-C",
                "debuginfo=line-tables-only",
            ])
            .args(["-C", "metadata=0123", "-C", "extra-filename=-0123", "--out-dir"])
            .arg(&deps)
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .args(extra)
            .current_dir(tree.join("ws"))
            .env("RAVENRS_XCACHE_DIR", self.root.join("store"))
            .env("OUT_DIR", tree.join("target/debug/build/demo/out"))
            .env_remove("RAVENRS_XCACHE_DISABLE")
            .env_remove("RAVENRS_XCACHE_VERIFY")
            .output()
            .unwrap()
    }

    fn library(&self, tree: &Path, crate_name: &str, source: &str, extra: &[&str]) -> Output {
        let mut args = vec!["--crate-type", "lib"];
        args.extend_from_slice(extra);
        let output = self.compile(tree, crate_name, source, &args);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        output
    }

    /// What became of each request so far: `(kind, crate, reason)`.
    fn log(&self) -> Vec<(String, String, String)> {
        let text = fs::read_to_string(self.root.join("store/events.tsv")).unwrap_or_default();
        text.lines()
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                (parts[1].to_owned(), parts[2].to_owned(), parts[3].to_owned())
            })
            .collect()
    }

    fn last(&self) -> (String, String, String) {
        self.log().last().cloned().expect("a request in the log")
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const LIB: &str = "pub fn answer() -> u32 { 41 }\npub fn here() -> &'static str { file!() }\n";

fn artifact(tree: &Path, name: &str) -> Vec<u8> {
    fs::read(tree.join("target/debug/deps").join(name)).unwrap()
}

#[test]
fn the_same_crate_in_another_worktree_comes_from_the_cache_byte_for_byte() {
    let bench = Bench::new("shared");
    let (one, two) = (bench.tree("one"), bench.tree("two"));
    for tree in [&one, &two] {
        fs::write(tree.join("ws/src/lib.rs"), LIB).unwrap();
    }
    bench.library(&one, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "miss");
    let restored = bench.library(&two, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "hit", "{:?}", bench.last());
    for name in ["libdemo-0123.rlib", "libdemo-0123.rmeta"] {
        assert_eq!(artifact(&one, name), artifact(&two, name), "{name}");
    }
    // Cargo is told about the files where they are now, and its dep-info names this worktree.
    let said = String::from_utf8_lossy(&restored.stderr).into_owned();
    assert!(said.contains(&format!("{}/target/debug/deps/libdemo-0123.rmeta", two.display())), "{said}");
    let dep_info = String::from_utf8(artifact(&two, "demo-0123.d")).unwrap();
    assert!(
        dep_info.contains(&format!("{}/target/debug/deps/libdemo-0123.rlib", two.display())),
        "{dep_info}"
    );
    assert!(!dep_info.contains(&one.display().to_string()), "{dep_info}");
}

#[test]
fn a_changed_source_is_compiled_and_the_reason_is_said() {
    let bench = Bench::new("source");
    let (one, two) = (bench.tree("one"), bench.tree("two"));
    fs::write(one.join("ws/src/lib.rs"), LIB).unwrap();
    fs::write(two.join("ws/src/lib.rs"), LIB.replace("41", "42")).unwrap();
    bench.library(&one, "demo", "lib.rs", &[]);
    bench.library(&two, "demo", "lib.rs", &[]);
    let (kind, _, reason) = bench.last();
    assert_eq!(kind, "miss");
    assert!(reason.contains("the source changed") && reason.contains("src/lib.rs"), "{reason}");
    assert_ne!(artifact(&one, "libdemo-0123.rlib"), artifact(&two, "libdemo-0123.rlib"));
    // Each version is kept: going back to the first is a hit.
    fs::write(two.join("ws/src/lib.rs"), LIB).unwrap();
    bench.library(&two, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "hit");
    assert_eq!(artifact(&one, "libdemo-0123.rlib"), artifact(&two, "libdemo-0123.rlib"));
}

#[test]
fn a_changed_dependency_compiles_what_uses_it() {
    let bench = Bench::new("dependency");
    let (one, two) = (bench.tree("one"), bench.tree("two"));
    let user = "pub fn twice() -> u32 { demo::answer() * 2 }\n";
    for (tree, lib) in [(&one, LIB.to_owned()), (&two, LIB.replace("41", "42"))] {
        fs::write(tree.join("ws/src/lib.rs"), lib).unwrap();
        fs::write(tree.join("ws/src/user.rs"), user).unwrap();
        bench.library(tree, "demo", "lib.rs", &[]);
        let dependency = format!("demo={}/target/debug/deps/libdemo-0123.rmeta", tree.display());
        bench.library(tree, "user", "user.rs", &["--extern", &dependency]);
    }
    // The user's own source is the same in both, but what it was compiled against is not.
    let (kind, name, reason) = bench.last();
    assert_eq!((kind.as_str(), name.as_str()), ("miss", "user"), "{reason}");
    // And with the same dependency in a third worktree, both come from the cache.
    let three = bench.tree("three");
    fs::write(three.join("ws/src/lib.rs"), LIB).unwrap();
    fs::write(three.join("ws/src/user.rs"), user).unwrap();
    bench.library(&three, "demo", "lib.rs", &[]);
    let dependency = format!("demo={}/target/debug/deps/libdemo-0123.rmeta", three.display());
    bench.library(&three, "user", "user.rs", &["--extern", &dependency]);
    let kinds: Vec<String> = bench.log().iter().rev().take(2).map(|event| event.0.clone()).collect();
    assert_eq!(kinds, ["hit", "hit"]);
}

#[test]
fn generated_code_is_shared_but_an_embedded_path_is_not() {
    let bench = Bench::new("embedded");
    let (one, two) = (bench.tree("one"), bench.tree("two"));
    let includes = "include!(concat!(env!(\"OUT_DIR\"), \"/gen.rs\"));\n";
    let embeds =
        "include!(concat!(env!(\"OUT_DIR\"), \"/gen.rs\"));\npub const WHERE: &str = env!(\"OUT_DIR\");\n";
    for tree in [&one, &two] {
        fs::write(tree.join("target/debug/build/demo/out/gen.rs"), "pub fn generated() -> u32 { 7 }\n")
            .unwrap();
        fs::write(tree.join("ws/src/lib.rs"), includes).unwrap();
        fs::write(tree.join("ws/src/embeds.rs"), embeds).unwrap();
    }
    // Reading a generated file through OUT_DIR is the same crate wherever the build directory is.
    bench.library(&one, "demo", "lib.rs", &[]);
    bench.library(&two, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "hit", "{:?}", bench.last());
    // Keeping OUT_DIR as data is not: the artifact names the first build directory.
    bench.library(&one, "embeds", "embeds.rs", &[]);
    bench.library(&two, "embeds", "embeds.rs", &[]);
    let (kind, _, reason) = bench.last();
    assert_eq!(kind, "miss");
    assert!(reason.contains("embeds the paths"), "{reason}");
    let built = artifact(&two, "libembeds-0123.rlib");
    let own = format!("{}/target/debug/build/demo/out", two.display());
    assert!(built.windows(own.len()).any(|window| window == own.as_bytes()), "its own path is in it");
    // Under the same paths it is a hit.
    bench.library(&one, "embeds", "embeds.rs", &[]);
    assert_eq!(bench.last().0, "hit");
}

#[test]
fn what_links_or_compiles_incrementally_goes_to_rustc_with_the_reason() {
    let bench = Bench::new("past");
    let tree = bench.tree("one");
    fs::write(tree.join("ws/src/lib.rs"), LIB).unwrap();
    fs::write(tree.join("ws/src/main.rs"), "fn main() {}\n").unwrap();
    let binary = bench.compile(&tree, "app", "main.rs", &["--crate-type", "bin"]);
    assert!(binary.status.success(), "{}", String::from_utf8_lossy(&binary.stderr));
    let (kind, name, reason) = bench.last();
    assert_eq!((kind.as_str(), name.as_str()), ("pass", "app"));
    assert!(reason.contains("links"), "{reason}");
    assert!(tree.join("target/debug/deps/app-0123").is_file());

    let incremental = format!("incremental={}/target/debug/incremental", tree.display());
    bench.library(&tree, "demo", "lib.rs", &["-C", &incremental]);
    assert!(bench.last().2.contains("incrementally"), "{:?}", bench.last());

    // A compile error is rustc's own: its exit code and its message come through.
    fs::write(tree.join("ws/src/lib.rs"), "pub fn broken() -> u32 { \"no\" }\n").unwrap();
    let failed = bench.compile(&tree, "demo", "lib.rs", &["--crate-type", "lib"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("mismatched types"));
    assert_eq!(bench.last().0, "failed");
}

#[test]
fn a_kept_file_that_changed_on_disk_is_refused_and_compiled_again() {
    let bench = Bench::new("corrupt");
    let (one, two) = (bench.tree("one"), bench.tree("two"));
    for tree in [&one, &two] {
        fs::write(tree.join("ws/src/lib.rs"), LIB).unwrap();
    }
    bench.library(&one, "demo", "lib.rs", &[]);
    let good = artifact(&one, "libdemo-0123.rlib");
    // Damage every kept file of that size, whichever it is.
    let mut damaged = 0;
    for folder in fs::read_dir(bench.root.join("store/blobs")).unwrap().flatten() {
        for file in fs::read_dir(folder.path()).unwrap().flatten() {
            if file.metadata().unwrap().len() == good.len() as u64 {
                let mut bytes = fs::read(file.path()).unwrap();
                let last = bytes.len() - 1;
                bytes[last] ^= 0xff;
                fs::write(file.path(), bytes).unwrap();
                damaged += 1;
            }
        }
    }
    assert!(damaged > 0);
    bench.library(&two, "demo", "lib.rs", &[]);
    let (kind, _, reason) = bench.last();
    assert_eq!(kind, "miss");
    assert!(reason.contains("could not be restored"), "{reason}");
    assert_eq!(artifact(&two, "libdemo-0123.rlib"), good, "compiled again, not the damaged file");
    // What it compiled replaced the damaged file, so the next one is a hit.
    let three = bench.tree("three");
    fs::write(three.join("ws/src/lib.rs"), LIB).unwrap();
    bench.library(&three, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "hit");
}

#[test]
fn the_same_crate_compiled_twice_at_once_leaves_a_whole_cache() {
    let bench = Bench::new("together");
    let trees: Vec<PathBuf> = (0..4).map(|index| bench.tree(&format!("t{index}"))).collect();
    for tree in &trees {
        fs::write(tree.join("ws/src/lib.rs"), LIB).unwrap();
    }
    std::thread::scope(|scope| {
        for tree in &trees {
            scope.spawn(|| bench.library(tree, "demo", "lib.rs", &[]));
        }
    });
    let first = artifact(&trees[0], "libdemo-0123.rlib");
    for tree in &trees {
        assert_eq!(artifact(tree, "libdemo-0123.rlib"), first);
    }
    let fifth = bench.tree("t5");
    fs::write(fifth.join("ws/src/lib.rs"), LIB).unwrap();
    bench.library(&fifth, "demo", "lib.rs", &[]);
    assert_eq!(bench.last().0, "hit");
    assert_eq!(artifact(&fifth, "libdemo-0123.rlib"), first);
}
