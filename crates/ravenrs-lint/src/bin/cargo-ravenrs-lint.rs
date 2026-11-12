//! `cargo ravenrs-lint`: checks the packages of a workspace that depend on RavenRS.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use ravenrs_lint::{Language, Level, Lint};

const USAGE: &str = "\
Checks applications built with RavenRS.

Usage: cargo ravenrs-lint [options]

Options:
  --manifest-path <path>  The Cargo.toml to start from (default: ./Cargo.toml)
  -p, --package <name>    Check only this package
  --message-format json   One JSON object per diagnostic
  --gate                  Run cargo fmt --check, cargo clippy -D warnings, these rules and cargo test,
                          stopping at the first that fails. Warnings fail too.
  --init                  Write the strict lints, clippy.toml, the test and the agents' instructions
  --language <en|pt-BR>   The interface's language, over the configuration (default: en)
  --list                  List the rules
  --print-lints           Print the [lints] tables --init writes
  --print-clippy          Print the clippy.toml --init writes
  -h, --help              Show this
";

struct Options {
    manifest: PathBuf,
    package: Option<String>,
    json: bool,
    gate: bool,
    init: bool,
    language: Option<Language>,
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Cargo runs `cargo-ravenrs-lint ravenrs-lint ...`.
    if args.first().is_some_and(|a| a == "ravenrs-lint") {
        args.remove(0);
    }
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut options = Options {
        manifest: PathBuf::from("Cargo.toml"),
        package: None,
        json: false,
        gate: false,
        init: false,
        language: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            "--list" => {
                for lint in Lint::ALL {
                    println!("ravenrs::{:<28} {}", lint.name(), lint.summary());
                }
                return Ok(ExitCode::SUCCESS);
            }
            "--print-lints" => {
                print!("{}", ravenrs_lint::recommended_lints());
                return Ok(ExitCode::SUCCESS);
            }
            "--print-clippy" => {
                print!("{}", ravenrs_lint::recommended_clippy());
                return Ok(ExitCode::SUCCESS);
            }
            "--manifest-path" => options.manifest = PathBuf::from(value("--manifest-path")?),
            "-p" | "--package" => options.package = Some(value("--package")?),
            "--message-format" => options.json = value("--message-format")? == "json",
            "--gate" => options.gate = true,
            "--init" => options.init = true,
            "--language" => {
                options.language = Some(match value("--language")?.as_str() {
                    "en" => Language::English,
                    "pt-BR" | "pt" => Language::PortugueseBrazil,
                    other => return Err(format!("unknown language `{other}`; use en or pt-BR")),
                });
            }
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        }
    }

    if options.init {
        let dir = options.manifest.parent().map(PathBuf::from).unwrap_or_default();
        let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
        let report =
            ravenrs_lint::init(&dir, options.language.unwrap_or_default()).map_err(|e| e.to_string())?;
        for path in &report.changed {
            println!("wrote {}", path.display());
        }
        for note in &report.notes {
            println!("note: {note}");
        }
        println!("next: cargo ravenrs-lint --gate");
        return Ok(ExitCode::SUCCESS);
    }

    if options.gate {
        let steps: [&[&str]; 2] =
            [&["fmt", "--all", "--check"], &["clippy", "--all-targets", "--", "-D", "warnings"]];
        for step in steps {
            cargo(&options, step)?;
        }
    }

    let packages = ravenrs_lint::packages(&options.manifest).map_err(|e| e.to_string())?;
    let mut chosen: Vec<_> = packages
        .into_iter()
        .filter(|p| match &options.package {
            Some(name) => &p.name == name,
            None => p.uses_ravenrs,
        })
        .collect();
    if chosen.is_empty() {
        return Err(match &options.package {
            Some(name) => format!("no package `{name}` in this workspace"),
            None => "no package in this workspace depends on RavenRS".into(),
        });
    }
    let mut diagnostics = Vec::new();
    let many = chosen.len() > 1;
    for package in &mut chosen {
        if let Some(language) = options.language {
            package.config.language = language;
        }
        let found = ravenrs_lint::check_package(package).map_err(|e| e.to_string())?;
        diagnostics.extend(found.into_iter().map(|mut d| {
            if many {
                d.file = PathBuf::from(&package.name).join(&d.file);
            }
            d
        }));
    }
    if options.json {
        print!("{}", ravenrs_lint::render_json(&diagnostics));
    } else {
        eprint!("{}", ravenrs_lint::render(&diagnostics, true));
    }
    let failed = diagnostics.iter().any(|d| d.level == Level::Deny || options.gate && d.level == Level::Warn);
    if failed {
        return Ok(ExitCode::FAILURE);
    }
    if options.gate {
        cargo(&options, &["test", "--all-targets"])?;
        eprintln!("ravenrs-lint gate: everything passed");
    }
    Ok(ExitCode::SUCCESS)
}

fn cargo(options: &Options, args: &[&str]) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.arg(args[0]).arg("--manifest-path").arg(&options.manifest);
    if let Some(name) = &options.package {
        command.args(["-p", name]);
    }
    command.args(&args[1..]);
    let status = command.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed; fix it and run the gate again", args.join(" ")))
    }
}
