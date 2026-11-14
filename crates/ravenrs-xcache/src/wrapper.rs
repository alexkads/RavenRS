//! Standing in front of rustc: look the compilation up, restore it, or compile and keep it.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::args::{Invocation, Kind, parse};
use crate::paths::Roots;
use crate::store::{Entry, Output, Store, Stored, entry_text, sha, sha_of_file};

/// Bump when what goes into a key changes: old entries then simply stop matching.
const KEY_VERSION: &str = "ravenrs-xcache key 1";

/// Variables of the build that rustc or a proc macro may read without saying so.
const ENVIRONMENT: &[&str] = &[
    "CARGO_CRATE_NAME",
    "CARGO_MANIFEST_DIR",
    "CARGO_MANIFEST_PATH",
    "CARGO_BIN_NAME",
    "OUT_DIR",
    "RUSTC_BOOTSTRAP",
    "MACOSX_DEPLOYMENT_TARGET",
    "IPHONEOS_DEPLOYMENT_TARGET",
    "SDKROOT",
    "SOURCE_DATE_EPOCH",
    "RUST_TARGET_PATH",
];

/// Milliseconds spent in each part of one request.
#[derive(Default)]
struct Times {
    key: u128,
    lookup: u128,
    restore: u128,
    compile: u128,
}

/// What became of one request, for the log.
struct Outcome<'a> {
    kind: &'a str,
    crate_name: &'a str,
    reason: &'a str,
    bytes: u64,
}

fn log(store: &Store, outcome: &Outcome<'_>, times: &Times) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    store.log(&format!(
        "{now}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        outcome.kind,
        outcome.crate_name,
        outcome.reason.replace(['\t', '\n'], " "),
        times.key,
        times.lookup,
        times.restore,
        times.compile,
        outcome.bytes
    ));
}

/// Runs rustc as it was asked, with nothing in between.
fn untouched(compiler: &OsStr, rest: &[OsString]) -> i32 {
    match Command::new(compiler).args(rest).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("ravenrs-xcache: cannot run {}: {error}", compiler.to_string_lossy());
            1
        }
    }
}

/// The wrapper: `ravenrs-xcache <rustc> <arguments>`.
pub(crate) fn run(compiler: &OsStr, rest: &[OsString]) -> i32 {
    let started = Instant::now();
    let off = std::env::var_os("RAVENRS_XCACHE_DISABLE").is_some_and(|v| v == "1" || v == "true");
    let Ok(store) = Store::open() else {
        return untouched(compiler, rest);
    };
    if off {
        return untouched(compiler, rest);
    }
    let pass = |crate_name: &str, reason: &str| {
        let code_started = Instant::now();
        let code = untouched(compiler, rest);
        let times = Times { compile: code_started.elapsed().as_millis(), ..Times::default() };
        let kind = if reason.contains("question") { "probe" } else { "pass" };
        log(&store, &Outcome { kind, crate_name, reason, bytes: 0 }, &times);
        code
    };
    let Some(args) = rest.iter().map(|arg| arg.to_str().map(str::to_owned)).collect::<Option<Vec<_>>>()
    else {
        return pass("?", "an argument is not UTF-8");
    };
    let name_of = |args: &[String]| {
        let at = args.iter().position(|a| a == "--crate-name");
        at.and_then(|at| args.get(at + 1)).cloned().unwrap_or_else(|| "?".to_owned())
    };
    if Path::new(compiler).file_stem().is_none_or(|stem| stem != "rustc") {
        // `ravenrs-xcache clippy-driver rustc …`: a workspace wrapper compiles, not rustc.
        return pass(&name_of(&args), "another tool compiles in rustc's place (clippy-driver)");
    }
    let invocation = match parse(&args) {
        Ok(invocation) => invocation,
        Err(reason) => return pass(&name_of(&args), &reason),
    };
    let Ok(cwd) = std::env::current_dir() else {
        return pass(&invocation.crate_name, "the working directory cannot be read");
    };
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    let roots = Roots::find(&invocation.out_dir, invocation.target.as_deref(), &cwd, cargo_home.as_deref());
    let remaps = match roots.remap_args() {
        Ok(remaps) => remaps,
        Err(reason) => return pass(&invocation.crate_name, &reason),
    };
    let request = match request_key(&store, compiler, &invocation, &roots, &cwd) {
        Ok(request) => request,
        Err(reason) => return pass(&invocation.crate_name, &reason),
    };
    let mut times = Times { key: started.elapsed().as_millis(), ..Times::default() };
    let job = Job {
        store: &store,
        compiler,
        rest,
        invocation: &invocation,
        roots: &roots,
        remaps: &remaps,
        cwd: &cwd,
    };

    if invocation.kind == Kind::ProcMacro {
        return job.proc_macro(&request, &mut times);
    }
    let looking = Instant::now();
    let found = job.look_up(&request);
    times.lookup = looking.elapsed().as_millis();
    let reason = match found {
        Ok(result) => {
            let restoring = Instant::now();
            match job.restore(&result) {
                Ok(bytes) => {
                    times.restore = restoring.elapsed().as_millis();
                    store.touch(&result);
                    let outcome =
                        Outcome { kind: "hit", crate_name: &invocation.crate_name, reason: "", bytes };
                    log(&store, &outcome, &times);
                    return 0;
                }
                Err(error) => {
                    store.drop_stored(&result);
                    format!("the kept result could not be restored ({error}); compiled again")
                }
            }
        }
        Err(reason) => reason,
    };
    job.compile_and_keep(&request, &reason, &mut times)
}

/// What identifies the compiler: what `rustc -vV` says, read once for each binary.
fn compiler_identity(store: &Store, compiler: &OsStr) -> Result<String, String> {
    let meta = fs::metadata(compiler).map_err(|e| format!("rustc cannot be read: {e}"))?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos());
    let toolchain = std::env::var("RUSTUP_TOOLCHAIN").unwrap_or_default();
    let which =
        sha(format!("{}|{}|{modified:?}|{toolchain}", compiler.to_string_lossy(), meta.len()).as_bytes());
    let record = store.root.join("compilers").join(which);
    if let Ok(known) = fs::read_to_string(&record) {
        return Ok(known);
    }
    let output = Command::new(compiler).arg("-vV").output().map_err(|e| format!("rustc -vV failed: {e}"))?;
    if !output.status.success() {
        return Err("rustc -vV failed".to_owned());
    }
    let version = String::from_utf8_lossy(&output.stdout).into_owned();
    let _ = Store::write_atomic(&record, version.as_bytes());
    Ok(version)
}

/// The key of a command line: everything known before compiling that can change the result.
fn request_key(
    store: &Store,
    compiler: &OsStr,
    invocation: &Invocation,
    roots: &Roots,
    cwd: &Path,
) -> Result<String, String> {
    let mut text = format!("{KEY_VERSION}\n{}\n", compiler_identity(store, compiler)?);
    text.push_str(&format!("kind {:?}\ncwd {}\n", invocation.kind, roots.norm(&cwd.to_string_lossy())));
    for arg in &invocation.key_args {
        text.push_str(&format!("arg {}\n", roots.norm(arg)));
    }
    for (spec, path) in &invocation.externs {
        let identity = match path {
            Some(path) => store
                .identity(&cwd.join(path))
                .map_err(|e| format!("the dependency {path} cannot be read: {e}"))?,
            None => String::new(),
        };
        text.push_str(&format!("extern {spec} {identity}\n"));
    }
    let mut environment: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, _)| name.starts_with("CARGO_PKG_") || ENVIRONMENT.contains(&name.as_str()))
        .collect();
    environment.sort();
    for (name, value) in environment {
        text.push_str(&format!("env {name}={}\n", roots.norm(&value)));
    }
    let key = sha(text.as_bytes());
    // RAVENRS_XCACHE_TRACE=<folder>: what each key was made of, to compare two builds that should
    // have shared a crate and did not.
    if let Some(folder) = std::env::var_os("RAVENRS_XCACHE_TRACE") {
        let file = Path::new(&folder).join(format!("{}-{}.key", invocation.crate_name, &key[..12]));
        let _ = Store::write_atomic(&file, text.as_bytes());
    }
    Ok(key)
}

/// One library or proc macro to produce.
struct Job<'a> {
    store: &'a Store,
    compiler: &'a OsStr,
    rest: &'a [OsString],
    invocation: &'a Invocation,
    roots: &'a Roots,
    remaps: &'a [String],
    cwd: &'a Path,
}

/// What rustc did: its exit code, and what it wrote to stderr.
struct Compiled {
    code: i32,
    lines: Vec<String>,
    began: SystemTime,
}

impl Job<'_> {
    /// The result kept for this command line whose recorded inputs are all as they are now, or why
    /// there is none.
    fn look_up(&self, request: &str) -> Result<String, String> {
        let entries = self.store.entries(request);
        if entries.is_empty() {
            return Err("first time: the compiler, the flags or a dependency are new to the cache".to_owned());
        }
        let mut why = String::new();
        for entry in &entries {
            match self.still_holds(entry) {
                Ok(()) if self.store.stored(&entry.result).is_some() => return Ok(entry.result.clone()),
                Ok(()) => why = "its entry was cleaned from the cache".to_owned(),
                Err(reason) if why.is_empty() => why = reason,
                Err(_) => {}
            }
        }
        Err(why)
    }

    fn still_holds(&self, entry: &Entry) -> Result<(), String> {
        if let Some(pinned) = &entry.pinned
            && *pinned != self.roots.raw()
        {
            return Err("it embeds the paths it was built under, and these are other paths".to_owned());
        }
        for (name, value) in &entry.env {
            let now = std::env::var(name).ok().map(|value| self.roots.norm(&value));
            if now != *value {
                return Err(format!("the environment variable {name} changed"));
            }
        }
        for (known, path) in &entry.files {
            let real = self.cwd.join(self.roots.denorm(path));
            match self.store.identity(&real) {
                Ok(now) if now == *known => {}
                Ok(_) => return Err(format!("the source changed: {path}")),
                Err(_) => return Err(format!("the source is gone: {path}")),
            }
        }
        Ok(())
    }

    /// Puts the kept files in the output directory and says to Cargo what rustc said. Returns the
    /// bytes restored.
    fn restore(&self, result: &str) -> Result<u64, String> {
        let stored = self.store.stored(result).ok_or("the result is gone")?;
        let verify = std::env::var_os("RAVENRS_XCACHE_VERIFY").is_none_or(|v| v != "0");
        let mut bytes = 0;
        for output in &stored.outputs {
            let blob = self.store.blob(&output.sha);
            let size = fs::metadata(&blob).map_err(|e| format!("{}: {e}", output.name))?.len();
            if size != output.size || (verify && sha_of_file(&blob).ok().as_deref() != Some(&output.sha)) {
                let _ = fs::remove_file(&blob);
                return Err(format!("{} is not what was kept", output.name));
            }
            let to = self.invocation.out_dir.join(&output.name);
            let _ = fs::remove_file(&to);
            if output.emit == "dep-info" {
                // Cargo reads this file to know what to watch: it has to name this checkout's files.
                let text = fs::read_to_string(&blob).map_err(|e| e.to_string())?;
                fs::write(&to, self.roots.denorm(&text)).map_err(|e| e.to_string())?;
            } else {
                fs::copy(&blob, &to).map_err(|e| format!("{}: {e}", output.name))?;
                self.store.remember(&to, &output.sha);
            }
            bytes += size;
        }
        let mut stderr = std::io::stderr().lock();
        for line in &stored.stderr {
            let text = match line.split_once(' ') {
                Some(("A", rest)) => {
                    let (emit, name) = rest.split_once(' ').unwrap_or((rest, ""));
                    artifact_line(&self.invocation.out_dir.join(name), emit)
                }
                Some(("D", rest)) => rest.to_owned(),
                _ => continue,
            };
            let _ = writeln!(stderr, "{text}");
        }
        Ok(bytes)
    }

    /// Runs rustc with the roots remapped, passing on what it says as it says it.
    fn compile(&self) -> Compiled {
        let began = SystemTime::now();
        let mut command = Command::new(self.compiler);
        command.args(self.rest).args(self.remaps).stderr(Stdio::piped());
        let mut lines = Vec::new();
        let code = match command.spawn() {
            Ok(mut child) => {
                if let Some(pipe) = child.stderr.take() {
                    let mut stderr = std::io::stderr();
                    for line in BufReader::new(pipe).split(b'\n').map_while(Result::ok) {
                        let _ = stderr.write_all(&line);
                        let _ = stderr.write_all(b"\n");
                        lines.push(String::from_utf8_lossy(&line).into_owned());
                    }
                }
                child.wait().ok().and_then(|status| status.code()).unwrap_or(1)
            }
            Err(error) => {
                eprintln!("ravenrs-xcache: cannot run {}: {error}", self.compiler.to_string_lossy());
                1
            }
        };
        Compiled { code, lines, began }
    }

    /// The files rustc reported, each inside the output directory: `(emit, file name)`.
    fn reported(&self, lines: &[String]) -> Result<Vec<(String, String)>, String> {
        let mut outputs = Vec::new();
        for line in lines {
            let Some((path, emit)) = artifact_of(line) else { continue };
            let path = PathBuf::from(path);
            let inside = path.parent() == Some(self.invocation.out_dir.as_path());
            match path.file_name().and_then(|name| name.to_str()) {
                Some(name) if inside => outputs.push((emit, name.to_owned())),
                _ => return Err("rustc wrote a file outside the output directory".to_owned()),
            }
        }
        Ok(outputs)
    }

    /// What the compilation read, from the dep-info rustc wrote: the entry without its result.
    fn inputs(&self, outputs: &[(String, String)], began: SystemTime) -> Result<Entry, String> {
        let (_, name) = outputs
            .iter()
            .find(|(emit, _)| emit == "dep-info")
            .ok_or("rustc wrote no dep-info, so its inputs are not known")?;
        let text = fs::read_to_string(self.invocation.out_dir.join(name)).map_err(|e| e.to_string())?;
        let (files, variables) = dep_info(&text);
        let mut entry = Entry::default();
        for file in files {
            let real = self.cwd.join(&file);
            let written =
                fs::metadata(&real).and_then(|meta| meta.modified()).map_err(|e| format!("{file}: {e}"))?;
            if written >= began {
                return Err(format!("{file} was written while it compiled"));
            }
            let known = self.store.identity(&real).map_err(|e| format!("{file}: {e}"))?;
            entry.files.push((known, self.roots.norm(&file)));
        }
        for name in variables {
            let value = std::env::var(&name).ok().map(|value| self.roots.norm(&value));
            entry.env.push((name, value));
        }
        Ok(entry)
    }

    fn compile_and_keep(&self, request: &str, reason: &str, times: &mut Times) -> i32 {
        let compiling = Instant::now();
        let compiled = self.compile();
        times.compile = compiling.elapsed().as_millis();
        let crate_name = self.invocation.crate_name.as_str();
        if compiled.code != 0 {
            log(self.store, &Outcome { kind: "failed", crate_name, reason, bytes: 0 }, times);
            return compiled.code;
        }
        match self.keep(request, &compiled) {
            Ok(bytes) => log(self.store, &Outcome { kind: "miss", crate_name, reason, bytes }, times),
            Err(why) => {
                let reason = format!("{reason}; not kept: {why}");
                log(self.store, &Outcome { kind: "miss", crate_name, reason: &reason, bytes: 0 }, times);
            }
        }
        compiled.code
    }

    /// Keeps what a compilation produced. Returns the bytes kept.
    fn keep(&self, request: &str, compiled: &Compiled) -> Result<u64, String> {
        let outputs = self.reported(&compiled.lines)?;
        let mut entry = self.inputs(&outputs, compiled.began)?;
        let mut stored = Stored::default();
        let mut embeds = !self.invocation.plain_objects;
        let mut bytes = 0;
        for (emit, name) in &outputs {
            let path = self.invocation.out_dir.join(name);
            let content = fs::read(&path).map_err(|e| format!("{name}: {e}"))?;
            let hash = if emit == "dep-info" {
                let text = String::from_utf8(content).map_err(|_| "the dep-info is not text")?;
                self.store.keep_bytes(self.roots.norm(&text).as_bytes()).map_err(|e| e.to_string())?
            } else {
                // The proof: with the roots remapped, a real path still in the bytes is data the
                // crate embedded, and the artifact only serves where it was built.
                embeds = embeds || self.roots.leaked_in(&content).is_some();
                let hash = sha(&content);
                self.store.keep_file(&path, &hash).map_err(|e| e.to_string())?;
                self.store.remember(&path, &hash);
                hash
            };
            let size =
                fs::metadata(self.store.blob(&hash)).map(|meta| meta.len()).map_err(|e| e.to_string())?;
            bytes += size;
            stored.outputs.push(Output { emit: emit.clone(), sha: hash, size, name: name.clone() });
        }
        for line in &compiled.lines {
            match artifact_of(line) {
                Some((path, emit)) => {
                    let name =
                        Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
                    stored.stderr.push(format!("A {emit} {name}"));
                }
                None => stored.stderr.push(format!("D {line}")),
            }
        }
        if embeds {
            entry.pinned = Some(self.roots.raw());
        }
        entry.result = sha(format!("{request}\n{}", entry_text(&entry)).as_bytes());
        self.store.keep_stored(&entry.result, &stored).map_err(|e| e.to_string())?;
        self.store.add_entry(request, &entry).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    /// A proc macro links, so it is compiled here every time. Compiled with the roots remapped it is
    /// the same macro in every worktree, so the crates that use it are told apart by what it was
    /// built from, not by the bytes of a library the linker stamped with this build directory.
    fn proc_macro(&self, request: &str, times: &mut Times) -> i32 {
        let compiling = Instant::now();
        let compiled = self.compile();
        times.compile = compiling.elapsed().as_millis();
        let crate_name = self.invocation.crate_name.as_str();
        let mut reason = "a proc macro links; compiled with the paths remapped".to_owned();
        if compiled.code == 0 {
            let known = self.reported(&compiled.lines).and_then(|outputs| {
                let entry = self.inputs(&outputs, compiled.began)?;
                Ok((outputs, sha(format!("proc-macro {request}\n{}", entry_text(&entry)).as_bytes())))
            });
            match known {
                Ok((outputs, identity)) => {
                    for (emit, name) in &outputs {
                        if emit == "link" {
                            self.store.remember(&self.invocation.out_dir.join(name), &identity);
                        }
                    }
                }
                Err(why) => reason = format!("{reason}; its users are told apart by its bytes: {why}"),
            }
        }
        log(self.store, &Outcome { kind: "pass", crate_name, reason: &reason, bytes: 0 }, times);
        compiled.code
    }
}

/// The path and kind of an output file, from the line rustc writes for it with `--json=artifacts`.
fn artifact_of(line: &str) -> Option<(String, String)> {
    if !line.starts_with("{\"$message_type\":\"artifact\"") {
        return None;
    }
    Some((json_string(line, "artifact")?, json_string(line, "emit")?))
}

/// The string value of a key in a line of JSON written by rustc.
fn json_string(line: &str, key: &str) -> Option<String> {
    let start = line.find(&format!("\"{key}\":\""))? + key.len() + 4;
    let mut value = String::new();
    let mut characters = line[start..].chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Some(value),
            '\\' => value.push(characters.next()?),
            other => value.push(other),
        }
    }
    None
}

/// The line rustc writes for an output file.
fn artifact_line(path: &Path, emit: &str) -> String {
    let path = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    format!("{{\"$message_type\":\"artifact\",\"artifact\":\"{path}\",\"emit\":\"{emit}\"}}")
}

/// The source files and the environment variables in a dep-info file, as rustc writes it: Makefile
/// rules with spaces escaped, then `# env-dep:NAME=VALUE` comments.
fn dep_info(text: &str) -> (Vec<String>, Vec<String>) {
    let mut files: Vec<String> = Vec::new();
    let mut variables: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(variable) = line.strip_prefix("# env-dep:") {
            let name = variable.split('=').next().unwrap_or(variable).to_owned();
            if !variables.contains(&name) {
                variables.push(name);
            }
            continue;
        }
        let Some((target, sources)) = line.split_once(": ") else { continue };
        if target.starts_with('#') {
            continue;
        }
        let mut current = String::new();
        let mut characters = sources.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '\\' if characters.peek() == Some(&' ') => {
                    current.push(' ');
                    characters.next();
                }
                ' ' => {
                    if !current.is_empty() && !files.contains(&current) {
                        files.push(std::mem::take(&mut current));
                    }
                    current.clear();
                }
                other => current.push(other),
            }
        }
        if !current.is_empty() && !files.contains(&current) {
            files.push(current);
        }
    }
    (files, variables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_info_gives_the_sources_once_and_the_variables_read() {
        let text = "/t/debug/deps/d-1.d: crates/d/src/lib.rs /t/debug/build/d-1/out/gen.rs src/with\\ space.rs\n\n\
                    /t/debug/deps/libd-1.rmeta: crates/d/src/lib.rs /t/debug/build/d-1/out/gen.rs\n\n\
                    crates/d/src/lib.rs:\n/t/debug/build/d-1/out/gen.rs:\n\n\
                    # env-dep:OUT_DIR=/t/debug/build/d-1/out\n# env-dep:NOT_SET\n";
        let (files, variables) = dep_info(text);
        assert_eq!(files, ["crates/d/src/lib.rs", "/t/debug/build/d-1/out/gen.rs", "src/with space.rs"]);
        assert_eq!(variables, ["OUT_DIR", "NOT_SET"]);
    }

    #[test]
    fn an_artifact_line_is_read_and_written_the_same() {
        let path = Path::new("/t/de\"bug/deps/libd-1.rmeta");
        let line = artifact_line(path, "metadata");
        assert_eq!(artifact_of(&line), Some((path.to_string_lossy().into_owned(), "metadata".to_owned())));
        assert_eq!(artifact_of("{\"$message_type\":\"diagnostic\",\"message\":\"x\"}"), None);
    }
}
