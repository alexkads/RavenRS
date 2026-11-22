# ravenrs-git

Git for RavenRS services, over the `git` installed on the machine and run with `ravenrs-process`: arguments with no shell, only the environment it needs, a time limit. It is what the Raven CI plugins `git`, `github` and `azure-devops` build on.

## What it does

| Piece | What it does |
|---|---|
| `Git::clone(url, dir, CloneOptions::new().depth(1).branch("main"))` | A shallow clone, without tags. |
| `Git::at(dir)` | A folder that already is a repository. |
| `fetch(remote, refspecs)`, `fetch_pull_request(remote, n)` | Fetches refs. A pull request lands as `<remote>/pr/N`, from `refs/pull/N/head` (GitHub and Azure DevOps). |
| `checkout(rev)`, `rev_parse(rev)`, `head()`, `ls_remote(remote, patterns)` | Checks out with no branch; names commits; lists the remote's refs as (commit, ref). |
| `merge(rev, message)` | `Merge::Merged(commit)`, or `Merge::Conflict(files)` with the merge undone, so the tree is as before. |
| `Credential::basic(user, token)`, `Credential::bearer(token)` | Goes as `http.extraHeader` in the child's environment (`GIT_CONFIG_COUNT`). It is never an argument, which anyone on the machine can read with `ps`, nor part of the remote's address, which `git` writes to `.git/config`. It shows as `Credential(•••)`. |
| `GitError` | `NotInstalled`, `Failed { command, code, error }` with the last lines `git` wrote, or `TimedOut`. |

Every command runs without the user's or the system's git configuration (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`), so a signing key or a hook set up for someone's own work does not change what a build does. `GIT_TERMINAL_PROMPT=0` keeps it from ever waiting for a password, and `SSH_AUTH_SOCK` passes for SSH remotes. Merges are recorded as `RavenRS <ravenrs@localhost>` unless `with_identity` says otherwise.

## Example

```rust,no_run
use ravenrs_git::{CloneOptions, Credential, Git, Merge};

let git = Git::at("/srv/ci/build-42")
    .with_credential(Credential::basic("x-access-token", "ghp_..."))
    .clone_with("https://github.com/owner/repo.git", &CloneOptions::new().depth(50).branch("master"))?;
git.fetch_pull_request("origin", 803)?;
match git.merge("origin/pr/803", "Merge PR 803 into master")? {
    Merge::Merged(commit) => println!("building {commit}"),
    Merge::Conflict(files) => println!("does not merge: {files:?}"),
}
# Ok::<(), ravenrs_git::GitError>(())
```
