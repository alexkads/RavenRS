//! `raven-ci-ssh-falso`: an `ssh` for the tests of `agent-remote`. It takes the options and the
//! destination as `ssh` does, and runs on this machine the command line that would run on the
//! other one, read as a POSIX shell reads single quotes, or as `cmd.exe` reads double quotes when
//! the line starts with one (#906), with its input, output and errors.
//! What it was given goes, one argument a line, to the file of `RAVEN_CI_SSH_FALSO_LOG`.

/// The words of a line of `cmd.exe` (#906): double quotes, a `""` inside them is one `"`, and
/// `%USERPROFILE%` is the home of this machine.
fn words_cmd(line: &str) -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let line = line.replace("%USERPROFILE%", &home);
    let (mut words, mut word, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                word.push('"');
            }
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            other => word.push(other),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

fn words(line: &str) -> Vec<String> {
    let (mut words, mut word, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if !quoted => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            ' ' if !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            other => word.push(other),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(log) = std::env::var("RAVEN_CI_SSH_FALSO_LOG") {
        let _ = std::fs::write(log, args.join("\n"));
    }
    let Some(at) = args.iter().position(|a| a == "--") else {
        eprintln!("ssh falso: falta -- antes do comando");
        std::process::exit(255);
    };
    let line = args[at + 1..].join(" ");
    // A line that starts with a double quote is one for `cmd.exe`, as `sistema = "windows"` makes.
    let words = if line.starts_with('"') { words_cmd(&line) } else { words(&line) };
    let Some((program, rest)) = words.split_first() else {
        eprintln!("ssh falso: sem comando");
        std::process::exit(255);
    };
    match std::process::Command::new(program).args(rest).status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(255)),
        Err(e) => {
            eprintln!("ssh falso: {program}: {e}");
            std::process::exit(255);
        }
    }
}
