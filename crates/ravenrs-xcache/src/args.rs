//! What a rustc command line asks for, and whether its result can be kept.

use std::path::PathBuf;

/// What the wrapper does with a command line it understood.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// A library: compiled once, kept, and restored wherever the same inputs come up.
    Library,
    /// A proc macro: it links, so it is compiled every time, but with the same path remapping, and
    /// what it was compiled from is remembered so that the crates using it can still be shared.
    ProcMacro,
}

/// A rustc command line, read.
#[derive(Debug)]
pub(crate) struct Invocation {
    pub kind: Kind,
    pub crate_name: String,
    pub out_dir: PathBuf,
    /// The `--target` triple, when the build is for another platform.
    pub target: Option<String>,
    /// `--extern` entries: the name with its options, and the file when there is one.
    pub externs: Vec<(String, Option<String>)>,
    /// Every argument that can change what is compiled, in order, without the ones that only say
    /// where files go.
    pub key_args: Vec<String>,
    /// Whether the objects are machine code only. With bitcode in them (LTO), looking for a path in
    /// the bytes proves nothing, so the result is kept for the paths it was built under.
    pub plain_objects: bool,
}

/// Long options that take a value, as the next argument or after `=`.
const WITH_VALUE: &[&str] = &[
    "crate-name",
    "edition",
    "crate-type",
    "emit",
    "cfg",
    "check-cfg",
    "out-dir",
    "extern",
    "target",
    "cap-lints",
    "error-format",
    "json",
    "color",
    "diagnostic-width",
    "remap-path-prefix",
    "sysroot",
    "explain",
    "print",
    "env-set",
    "codegen",
    "warn",
    "allow",
    "deny",
    "forbid",
    "force-warn",
    "pretty",
    "unpretty",
];

/// Short options that take a value, glued or as the next argument.
const SHORT_WITH_VALUE: &[char] = &['C', 'L', 'l', 'A', 'W', 'D', 'F', 'Z', 'o'];

/// Reads a rustc command line. `Err` is why the wrapper leaves it to rustc untouched.
pub(crate) fn parse(args: &[String]) -> Result<Invocation, String> {
    let mut crate_name = None;
    let mut crate_types: Vec<String> = Vec::new();
    let mut emit: Option<String> = None;
    let mut out_dir = None;
    let mut target = None;
    let mut inputs: Vec<&str> = Vec::new();
    let mut externs = Vec::new();
    let mut key_args = Vec::new();
    let mut json = String::new();
    let mut embeds_bitcode = true;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        index += 1;
        let (name, value): (String, Option<String>) = if let Some(long) = arg.strip_prefix("--") {
            match long.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None if WITH_VALUE.contains(&long) => {
                    let value = args.get(index).cloned();
                    index += 1;
                    (long.to_owned(), Some(value.ok_or_else(|| format!("--{long} has no value"))?))
                }
                None => (long.to_owned(), None),
            }
        } else if arg == "-" {
            return Err("it compiles standard input".to_owned());
        } else if let Some(short) = arg.strip_prefix('-') {
            let letter = short.chars().next().unwrap_or(' ');
            if SHORT_WITH_VALUE.contains(&letter) {
                let glued = &short[letter.len_utf8()..];
                let value = if glued.is_empty() {
                    let value = args.get(index).cloned();
                    index += 1;
                    value.ok_or_else(|| format!("-{letter} has no value"))?
                } else {
                    glued.to_owned()
                };
                (letter.to_string(), Some(value))
            } else {
                (short.to_owned(), None)
            }
        } else if arg.starts_with('@') {
            return Err("its arguments come from a response file".to_owned());
        } else {
            inputs.push(arg);
            key_args.push(arg.to_owned());
            continue;
        };
        let value_text = value.clone().unwrap_or_default();
        match name.as_str() {
            "crate-name" => crate_name = value.clone(),
            "crate-type" => crate_types.extend(value_text.split(',').map(str::to_owned)),
            "emit" => {
                if emit.is_some() {
                    return Err("--emit is given more than once".to_owned());
                }
                emit = value.clone();
            }
            "out-dir" => {
                out_dir = value.map(PathBuf::from);
                continue;
            }
            "extern" => {
                match value_text.split_once('=') {
                    Some((spec, path)) => externs.push((spec.to_owned(), Some(path.to_owned()))),
                    None => externs.push((value_text, None)),
                }
                continue;
            }
            "diagnostic-width" => continue,
            "json" => json = value_text.clone(),
            "print" | "explain" | "version" | "V" | "vV" | "help" | "h" => {
                return Err("it asks rustc a question, it does not compile".to_owned());
            }
            "test" => return Err("it builds a test binary, which links".to_owned()),
            "o" => return Err("-o names the output file".to_owned()),
            "sysroot" => return Err("--sysroot points at another standard library".to_owned()),
            "remap-path-prefix" => return Err("it remaps paths itself".to_owned()),
            "target" if value_text.ends_with(".json") => {
                return Err("the target is a spec file".to_owned());
            }
            "target" => target = value.clone(),
            "L" if value_text.starts_with("dependency=") => continue,
            "C" | "codegen" => {
                let option = value_text.split('=').next().unwrap_or_default();
                match option {
                    "incremental" => return Err("it compiles incrementally".to_owned()),
                    "profile-generate" | "profile-use" => {
                        return Err("it uses profile data".to_owned());
                    }
                    "embed-bitcode" if value_text == "embed-bitcode=no" => embeds_bitcode = false,
                    "lto" | "linker-plugin-lto" => embeds_bitcode = true,
                    _ => {}
                }
            }
            _ => {}
        }
        match value {
            Some(value) => key_args.push(format!("--{name}={value}")),
            None => key_args.push(format!("--{name}")),
        }
    }

    let [_input] = inputs.as_slice() else {
        return Err(if inputs.is_empty() {
            "it asks rustc a question, it does not compile".to_owned()
        } else {
            "it has more than one input file".to_owned()
        });
    };
    let kind = match crate_types.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["lib"] | ["rlib"] | ["lib", "rlib"] | ["rlib", "lib"] => Kind::Library,
        ["proc-macro"] => Kind::ProcMacro,
        [] => return Err("it builds an executable, which links".to_owned()),
        other => return Err(format!("crate type {} links", other.join(","))),
    };
    let emit = emit.ok_or_else(|| "it has no --emit".to_owned())?;
    for kind in emit.split(',') {
        if !matches!(kind, "dep-info" | "metadata" | "link") {
            return Err(format!("--emit={kind} is not kept"));
        }
    }
    if !json.split(',').any(|part| part == "artifacts") {
        return Err("rustc does not report its output files (--json=artifacts)".to_owned());
    }
    Ok(Invocation {
        kind,
        crate_name: crate_name.ok_or_else(|| "it has no --crate-name".to_owned())?,
        out_dir: out_dir.ok_or_else(|| "it has no --out-dir".to_owned())?,
        target,
        externs,
        key_args,
        plain_objects: !embeds_bitcode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_like(extra: &[&str]) -> Vec<String> {
        let mut args: Vec<String> = [
            "--crate-name",
            "demo",
            "--edition=2024",
            "src/lib.rs",
            "--error-format=json",
            "--json=diagnostic-rendered-ansi,artifacts,future-incompat",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata,link",
            "-C",
            "embed-bitcode=no",
            "-C",
            "metadata=abc",
            "--out-dir",
            "/t/debug/deps",
            "-L",
            "dependency=/t/debug/deps",
            "--extern",
            "serde=/t/debug/deps/libserde-1.rmeta",
            "--extern",
            "proc_macro",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        args.extend(extra.iter().map(|s| (*s).to_owned()));
        args
    }

    #[test]
    fn a_library_as_cargo_asks_for_it_is_kept() {
        let it = parse(&cargo_like(&[])).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(it.kind, Kind::Library);
        assert_eq!(it.crate_name, "demo");
        assert_eq!(it.out_dir, PathBuf::from("/t/debug/deps"));
        assert_eq!(it.externs.len(), 2);
        assert_eq!(it.externs[0], ("serde".to_owned(), Some("/t/debug/deps/libserde-1.rmeta".to_owned())));
        assert!(it.plain_objects);
        // Where files go is not what is compiled.
        assert!(!it.key_args.iter().any(|a| a.contains("/t/debug")), "{:?}", it.key_args);
        assert!(it.key_args.contains(&"--C=metadata=abc".to_owned()));
        assert!(it.key_args.contains(&"src/lib.rs".to_owned()));
    }

    #[test]
    fn what_links_or_only_asks_is_left_to_rustc() {
        for (extra, why) in [
            (vec!["--test"], "test binary"),
            (vec!["-C", "incremental=/t/inc"], "incrementally"),
            (vec!["--print=cfg"], "question"),
            (vec!["-o", "x"], "-o"),
            (vec!["--remap-path-prefix=/a=/b"], "remaps"),
            (vec!["other.rs"], "more than one input"),
        ] {
            let error = parse(&cargo_like(&extra)).err().unwrap_or_default();
            assert!(error.contains(why), "{extra:?}: {error}");
        }
        let query: Vec<String> = vec!["-vV".to_owned()];
        assert!(parse(&query).is_err());
        let binary: Vec<String> =
            cargo_like(&[]).into_iter().filter(|a| a != "--crate-type" && a != "lib").collect();
        assert!(parse(&binary).err().unwrap_or_default().contains("executable"));
    }

    #[test]
    fn a_proc_macro_is_told_apart_and_lto_objects_are_not_plain() {
        let mut args = cargo_like(&[]);
        let at = args.iter().position(|a| a == "lib").unwrap_or(0);
        args[at] = "proc-macro".to_owned();
        assert_eq!(parse(&args).map(|i| i.kind).ok(), Some(Kind::ProcMacro));
        let lto = parse(&cargo_like(&["-C", "linker-plugin-lto"])).unwrap_or_else(|why| panic!("{why}"));
        assert!(!lto.plain_objects);
    }
}
