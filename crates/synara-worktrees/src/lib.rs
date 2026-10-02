use anyhow::Result;
use git2::{Repository, WorktreeAddOptions, WorktreePruneOptions};
use std::path::{Path, PathBuf};

pub fn create(
    repo: impl AsRef<Path>,
    path: impl AsRef<Path>,
    branch: &str,
) -> Result<PathBuf> {
    let r = Repository::discover(repo)?;
    let branch_ref = format!("refs/heads/{branch}");
    let reference = r.find_reference(&branch_ref)?;
    let commit = reference.peel_to_commit()?;

    let name = path
        .as_ref()
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("worktree path has no final component"))?
        .to_string_lossy()
        .into_owned();

    let mut options = WorktreeAddOptions::new();
    options.reference(Some(&reference));
    let wt = r.worktree(&name, path.as_ref(), Some(&options))?;
    let _ = commit;
    Ok(wt.path().to_path_buf())
}

pub fn list(repo: impl AsRef<Path>) -> Result<Vec<(String, PathBuf)>> {
    let r = Repository::discover(repo)?;
    let names = r.worktrees()?;
    let mut out = Vec::new();
    for index in 0..names.len() {
        let Some(name) = names.get(index) else {
            continue;
        };
        let wt = r.find_worktree(name)?;
        out.push((name.to_string(), wt.path().to_path_buf()));
    }
    Ok(out)
}

pub fn remove(repo: impl AsRef<Path>, name: &str, force: bool) -> Result<()> {
    let r = Repository::discover(repo)?;
    let wt = r.find_worktree(name)?;
    let mut options = WorktreePruneOptions::new();
    if force {
        options.valid(false);
        options.working_tree(true);
        options.unlocked(true);
    }
    wt.prune(Some(&mut options))?;
    Ok(())
}
