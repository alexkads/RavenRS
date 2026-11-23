//! The key a session of AI or a script reaches the server with, when `RAVEN_CI_TOKEN` does not
//! say it (#941): kept by `raven-ci ia instalar --chave -` in a file only its owner reads, and read
//! by every command that speaks to a server ([`crate::remote`]) and by the MCP ([`crate::mcp`]).
//! A leaf: it uses no other module of the program.

use std::io::Write;
use std::path::PathBuf;

/// Where the key is kept: `chave-do-servidor` in the folder of the program.
pub fn key_file() -> PathBuf {
    crate::versao::APP.home().join("chave-do-servidor")
}

/// The key in [`key_file`], when there is one.
pub fn saved_key() -> Option<String> {
    std::fs::read_to_string(key_file()).ok().map(|key| key.trim().to_owned()).filter(|key| !key.is_empty())
}

/// Keeps `key` in [`key_file`], readable only by its owner; one there before is replaced.
pub fn save_key(key: &str) -> std::io::Result<()> {
    let file = key_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = std::fs::remove_file(&file);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&file)?.write_all(key.as_bytes())
}
