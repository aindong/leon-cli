//! A git worktree and branch per worker (`leon agent start --worktree`), and removing them
//! (`leon agent close`) only when that loses nothing, unless forced. git runs here, as the user.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::client::Fail;

/// `git -C dir ARGS`, given `input` on stdin: its stdout, or why it failed.
fn git_with(dir: &Path, args: &[&str], input: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("can't run git: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped");
    let _ = stdin.write_all(input.as_bytes()); // git may stop reading early
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_with(dir, args, "")
}

/// A branch name made from `text`: letters, digits, `.`, `_`, `-` and `/` kept, anything else `-`.
pub fn branch_name(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || "._-/".contains(c) { c } else { '-' }).collect()
}

/// Where leon puts the worktree for `branch` of the checkout `top`.
fn place(top: &Path, branch: &str) -> PathBuf {
    let name = top.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    top.with_file_name(format!("{name}-worktrees")).join(branch.replace('/', "-"))
}

/// Make branch `branch` at `dir`'s checkout's HEAD, checked out in a new worktree beside the
/// checkout: `<toplevel>-worktrees/<branch, / as ->`. Returns the worktree as `agent.link` takes it.
pub fn create(dir: &Path, branch: &str) -> Result<Value, Fail> {
    let top = git(dir, &["rev-parse", "--show-toplevel"]).map_err(|e| match e.starts_with("can't run git") {
        true => Fail::Other(e),
        false => Fail::Other(format!("{} is not in a git repository: --worktree needs one ({e})", dir.display())),
    })?;
    let top = PathBuf::from(top.trim());
    let p = place(&top, branch).to_string_lossy().into_owned();
    git(&top, &["worktree", "add", "-q", "-b", branch, "--", &p])
        .map_err(|e| Fail::Other(format!("git worktree add: {e}")))?;
    // the main checkout, not `top`: `top` may be another worktree, removed before this one
    let common = git(&top, &["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .map_err(|e| Fail::Other(format!("git rev-parse: {e}")))?;
    let common = PathBuf::from(common.trim());
    let repo =
        if common.ends_with(".git") { common.parent().map_or(common.clone(), Path::to_path_buf) } else { common };
    Ok(json!({ "path": p, "branch": branch, "repo": repo }))
}

fn fields(tree: &Value) -> (PathBuf, String, PathBuf) {
    let field = |k: &str| tree[k].as_str().unwrap_or("").to_string();
    (PathBuf::from(field("path")), field("branch"), PathBuf::from(field("repo")))
}

/// Check that `tree` is a worktree leon made: git lists it in `repo` with `branch` checked out, at
/// the place `create` puts one. The record is server state any client could have written, so
/// nothing is removed on its word alone.
pub fn check_ours(tree: &Value) -> Result<(), Fail> {
    let (path, branch, repo) = fields(tree);
    let refuse = |why: &str| {
        Fail::Other(format!(
            "leon won't remove {} (branch {branch}): {why}; nothing was closed. Remove it yourself if you mean to.",
            path.display()
        ))
    };
    let shaped = path.file_name().is_some_and(|n| n.to_string_lossy() == branch.replace('/', "-"))
        && path.parent().and_then(Path::file_name).is_some_and(|n| n.to_string_lossy().ends_with("-worktrees"));
    if branch.is_empty() || !shaped {
        return Err(refuse("it isn't where leon puts the worktrees it makes"));
    }
    let list = git(&repo, &["worktree", "list", "--porcelain"]).map_err(|e| refuse(&e))?;
    let entry = format!("worktree {}\n", path.display());
    let listed = list.split("\n\n").map(|e| format!("{e}\n")).find(|e| e.starts_with(&entry));
    if !listed.is_some_and(|e| e.lines().any(|l| l == format!("branch refs/heads/{branch}"))) {
        return Err(refuse("git doesn't list it as a worktree of that repository on that branch"));
    }
    Ok(())
}

/// What removing worktree `tree` and its branch would lose: changes not committed (untracked and
/// ignored files too, whatever `status.showUntrackedFiles` says), commits on its HEAD or branch
/// that no other branch and no remote holds, and a lock someone put on it. The branch is checked
/// from the repository, so a worktree directory already gone still guards its commits.
pub fn would_lose(tree: &Value) -> Result<Vec<String>, Fail> {
    let (path, branch, repo) = fields(tree);
    let fail = |e: String| Fail::Other(format!("{}: {e}", repo.display()));
    let mut lost = vec![];
    let mut heads = vec![];
    if path.is_dir() {
        let args = ["status", "--porcelain", "--untracked-files=all", "--ignored=matching", "--ignore-submodules=none"];
        for line in git(&path, &args).map_err(fail)?.lines() {
            match line.strip_prefix("!! ") {
                Some(ignored) => lost.push(format!("ignored by git (build output?): {ignored}")),
                None => lost.push(format!("not committed: {}", line.trim())),
            }
        }
        heads.push(git(&path, &["rev-parse", "HEAD"]).map_err(fail)?.trim().to_string());
    }
    let list = git(&repo, &["worktree", "list", "--porcelain"]).map_err(fail)?;
    let entry = format!("worktree {}\n", path.display());
    let mine = list.split("\n\n").map(|e| format!("{e}\n")).find(|e| e.starts_with(&entry));
    if mine.is_some_and(|e| e.lines().any(|l| l.starts_with("locked"))) {
        lost.push(format!("locked: {} (git worktree unlock it first)", path.display()));
    }
    let own = format!("refs/heads/{branch}");
    let refs = git(&repo, &["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes"]).map_err(fail)?;
    if refs.lines().any(|r| r == own) {
        heads.push(own.clone());
    }
    if heads.is_empty() {
        return Ok(lost);
    }
    // every other branch and remote, on stdin: a repo can have more than a command line holds
    let others: String = refs.lines().filter(|r| *r != own).map(|r| format!("^{r}\n")).collect();
    let mut args = vec!["rev-list", "--oneline"];
    args.extend(heads.iter().map(String::as_str));
    args.push("--stdin");
    let only = git_with(&repo, &args, &others).map_err(fail)?;
    lost.extend(only.lines().map(|l| format!("not on any other branch or remote: {l}")));
    Ok(lost)
}

/// How to finish removing `tree` by hand.
pub fn by_hand(tree: &Value) -> String {
    let (path, branch, repo) = fields(tree);
    let (path, repo) = (path.display(), repo.display());
    format!("git -C '{repo}' worktree remove --force '{path}'; git -C '{repo}' branch -D '{branch}'")
}

/// Remove worktree `tree` and its branch (if it still has that name); `force` also when they
/// hold work.
pub fn remove(tree: &Value, force: bool) -> Result<(), Fail> {
    let (path, branch, repo) = fields(tree);
    let p = path.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.extend(["--", &p]);
    if path.exists() {
        git(&repo, &args).map_err(|e| Fail::Other(format!("git worktree remove: {e}")))?;
    } else {
        let _ = git(&repo, &["worktree", "prune"]);
    }
    let own = format!("refs/heads/{branch}");
    if git(&repo, &["show-ref", "--verify", "--quiet", &own]).is_ok() {
        git(&repo, &["branch", "-D", "--", &branch])
            .map_err(|e| Fail::Other(format!("git branch -D {branch}: {e}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_names_keep_only_safe_characters() {
        assert_eq!(branch_name("agent/fix tests!"), "agent/fix-tests-");
        assert_eq!(branch_name("feat-x.2_y"), "feat-x.2_y");
    }

    #[test]
    fn a_worktree_goes_beside_its_checkout() {
        assert_eq!(place(Path::new("/src/proj"), "agent/docs"), Path::new("/src/proj-worktrees/agent-docs"));
    }
}
