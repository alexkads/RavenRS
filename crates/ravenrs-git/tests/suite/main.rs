//! The tests of ravenrs-git, in one binary, over repositories each test makes with the git installed.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod commands;

use std::path::Path;

use ravenrs_git::Git;

/// A remote in `root`/origin.git with `main` holding one file, and the folder where it was made.
#[derive(Debug)]
pub struct Remote {
    /// The folder of the work the remote was made from.
    pub work: Git,
    /// Its address, as `file://…`, so a clone can be shallow.
    pub url: String,
}

/// Writes `text` to `file` in `git`'s folder and commits it.
pub fn commit(git: &Git, file: &str, text: &str, message: &str) -> String {
    std::fs::write(git.dir().join(file), text).unwrap();
    git.run(&["add", "--", file]).unwrap();
    git.run(&["commit", "-q", "-m", message]).unwrap();
    git.head().unwrap()
}

/// A remote with two commits on `main`.
pub fn remote(root: &Path) -> Remote {
    let work = Git::at(root.join("work"));
    std::fs::create_dir_all(work.dir()).unwrap();
    work.run(&["init", "-q", "-b", "main"]).unwrap();
    commit(&work, "arquivo.txt", "linha 1\nlinha 2\n", "primeiro");
    commit(&work, "arquivo.txt", "linha 1\nlinha 2\nlinha 3\n", "segundo");
    let bare = root.join("origin.git");
    Git::at(root).run(&["clone", "-q", "--bare", "work", "origin.git"]).unwrap();
    work.run(&["remote", "add", "origin", bare.to_str().unwrap()]).unwrap();
    // `file:///C:/…` on Windows, `file:///tmp/…` elsewhere.
    let path = bare.to_str().unwrap().replace('\\', "/");
    let url = if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") };
    Remote { work, url }
}
