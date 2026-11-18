//! A git worktree and branch per worker (`leon agent start --worktree`), and removing them
//! (`leon agent close`) only when that loses nothing, unless forced. git runs here, as the user.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::client::Fail;

/// `git -C dir ARGS`'s stdout, or its stderr as the failure.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|e| format!("can't run git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A branch name made from `text`: letters, digits, `.`, `_`, `-` and `/` kept, anything else `-`.
pub fn branch_name(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || "._-/".contains(c) { c } else { '-' }).collect()
}

/// Make branch `branch` at `dir`'s checkout's HEAD, checked out in a new worktree beside the
/// checkout: `<toplevel>-worktrees/<branch, / as ->`. Returns the worktree as `agent.link` takes it.
pub fn create(dir: &Path, branch: &str) -> Result<Value, Fail> {
    let top = git(dir, &["rev-parse", "--show-toplevel"])
        .map_err(|_| Fail::Other(format!("{} is not in a git repository: --worktree needs one", dir.display())))?;
    let top = PathBuf::from(top.trim());
    let name = top.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    let path = top.with_file_name(format!("{name}-worktrees")).join(branch.replace('/', "-"));
    let p = path.to_string_lossy().into_owned();
    git(&top, &["worktree", "add", "-q", "-b", branch, &p])
        .map_err(|e| Fail::Other(format!("git worktree add: {e}")))?;
    Ok(json!({ "path": p, "branch": branch, "repo": top }))
}

/// What removing worktree `tree` and its branch would lose: uncommitted changes (untracked files
/// too; ignored ones, such as build output, go without asking, as with `git worktree remove`), and
/// commits on its HEAD or its branch that no other branch and no remote holds. The branch is
/// checked from the repository, so a worktree directory already gone still guards its commits.
pub fn would_lose(tree: &Value) -> Result<Vec<String>, Fail> {
    let field = |k: &str| tree[k].as_str().unwrap_or("").to_string();
    let (path, branch, repo) = (PathBuf::from(field("path")), field("branch"), PathBuf::from(field("repo")));
    let fail = |e: String| Fail::Other(format!("{}: {e}", repo.display()));
    let mut lost = vec![];
    let mut heads = vec![];
    if path.is_dir() {
        let status = git(&path, &["status", "--porcelain"]).map_err(fail)?;
        lost.extend(status.lines().map(|l| format!("uncommitted: {}", l.trim())));
        heads.push(git(&path, &["rev-parse", "HEAD"]).map_err(fail)?.trim().to_string());
    }
    let own = format!("refs/heads/{branch}");
    let refs = git(&repo, &["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes"]).map_err(fail)?;
    if refs.lines().any(|r| r == own) {
        heads.push(own.clone());
    }
    if heads.is_empty() {
        return Ok(lost);
    }
    let mut args = vec!["rev-list".to_string(), "--oneline".into()];
    args.extend(heads);
    args.push("--not".into());
    args.extend(refs.lines().filter(|r| *r != own).map(String::from));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let only = git(&repo, &args).map_err(fail)?;
    lost.extend(only.lines().map(|l| format!("not on any other branch or remote: {l}")));
    Ok(lost)
}

/// Remove worktree `tree` and its branch; `force` also when they hold work.
pub fn remove(tree: &Value, force: bool) -> Result<(), Fail> {
    let field = |k: &str| tree[k].as_str().unwrap_or("").to_string();
    let (path, branch, main) = (field("path"), field("branch"), PathBuf::from(field("repo")));
    let path = path.as_str();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path);
    if Path::new(path).exists() {
        git(&main, &args).map_err(|e| Fail::Other(format!("git worktree remove: {e}")))?;
    } else {
        let _ = git(&main, &["worktree", "prune"]);
    }
    git(&main, &["branch", "-D", &branch]).map_err(|e| Fail::Other(format!("git branch -D {branch}: {e}")))?;
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
}
