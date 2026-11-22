use ravenrs_git::{CloneOptions, Credential, Git, GitError, Merge};

use crate::{commit, remote};

#[test]
fn a_shallow_clone_brings_only_the_last_commit() {
    let root = tempfile::tempdir().unwrap();
    let origin = remote(root.path());
    let clone = Git::clone(&origin.url, root.path().join("clone"), &CloneOptions::new().depth(1)).unwrap();
    assert_eq!(clone.run(&["rev-list", "--count", "HEAD"]).unwrap(), ["1"]);
    assert_eq!(clone.head().unwrap(), origin.work.head().unwrap());
    let whole =
        Git::clone(&origin.url, root.path().join("inteiro"), &CloneOptions::new().branch("main")).unwrap();
    assert_eq!(whole.run(&["rev-list", "--count", "HEAD"]).unwrap(), ["2"]);
}

#[test]
fn a_pull_request_is_fetched_and_merged_into_its_target() {
    let root = tempfile::tempdir().unwrap();
    let origin = remote(root.path());
    origin.work.run(&["checkout", "-q", "-b", "feature"]).unwrap();
    let pull = commit(&origin.work, "novo.txt", "do PR\n", "o PR");
    origin.work.run(&["push", "-q", "origin", "feature:refs/pull/7/head"]).unwrap();

    let clone = Git::clone(&origin.url, root.path().join("build"), &CloneOptions::new()).unwrap();
    let listed = clone.ls_remote("origin", &["refs/pull/*"]).unwrap();
    assert_eq!(listed, [(pull.clone(), "refs/pull/7/head".to_owned())]);
    clone.fetch_pull_request("origin", 7).unwrap();
    assert_eq!(clone.rev_parse("origin/pr/7").unwrap(), pull);

    let Merge::Merged(merged) = clone.merge("origin/pr/7", "Merge PR 7").unwrap() else {
        panic!("it merges")
    };
    assert_eq!(clone.head().unwrap(), merged);
    assert!(clone.dir().join("novo.txt").exists());
    assert_eq!(clone.run(&["log", "-1", "--format=%an <%ae>"]).unwrap(), ["RavenRS <ravenrs@localhost>"]);
}

#[test]
fn a_conflict_names_the_files_and_leaves_the_tree_clean() {
    let root = tempfile::tempdir().unwrap();
    let origin = remote(root.path());
    origin.work.run(&["checkout", "-q", "-b", "feature"]).unwrap();
    commit(&origin.work, "arquivo.txt", "linha 1\nmudada no PR\nlinha 3\n", "o PR");
    origin.work.run(&["push", "-q", "origin", "feature:refs/pull/8/head"]).unwrap();
    origin.work.run(&["checkout", "-q", "main"]).unwrap();
    commit(&origin.work, "arquivo.txt", "linha 1\nmudada no main\nlinha 3\n", "no main");
    origin.work.run(&["push", "-q", "origin", "main"]).unwrap();

    let clone = Git::clone(&origin.url, root.path().join("build"), &CloneOptions::new()).unwrap();
    let before = clone.head().unwrap();
    clone.fetch_pull_request("origin", 8).unwrap();
    assert_eq!(
        clone.merge("origin/pr/8", "Merge PR 8").unwrap(),
        Merge::Conflict(vec!["arquivo.txt".to_owned()])
    );
    assert!(clone.run(&["status", "--porcelain"]).unwrap().is_empty(), "nothing is left half merged");
    assert_eq!(clone.head().unwrap(), before);
    clone.checkout(&before).unwrap();
}

#[test]
fn the_credential_goes_in_the_environment_and_not_in_the_arguments() {
    let root = tempfile::tempdir().unwrap();
    let origin = remote(root.path());
    let token = "ghp_tokenQueNaoPodeAparecer";
    let git = Git::at(origin.work.dir()).with_credential(Credential::basic("x-access-token", token));
    let header = git.run(&["config", "--get", "http.extraHeader"]).unwrap();
    assert_eq!(header, ["Authorization: Basic eC1hY2Nlc3MtdG9rZW46Z2hwX3Rva2VuUXVlTmFvUG9kZUFwYXJlY2Vy"]);
    let config = std::fs::read_to_string(origin.work.dir().join(".git").join("config")).unwrap();
    assert!(!config.contains("extraHeader") && !config.contains(token), "nothing written to the repository");
}

#[test]
fn a_missing_git_and_a_failing_command_say_so() {
    let root = tempfile::tempdir().unwrap();
    let missing = Git::at(root.path()).with_program("git-que-nao-existe").run(&["status"]);
    assert!(matches!(missing, Err(GitError::NotInstalled(program)) if program == "git-que-nao-existe"));
    let failed = Git::at(root.path()).run(&["rev-parse", "--verify", "naoexiste"]);
    assert!(matches!(failed, Err(GitError::Failed { code: Some(_), .. })), "{failed:?}");
}
