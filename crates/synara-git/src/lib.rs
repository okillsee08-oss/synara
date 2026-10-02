use anyhow::{Context, Result};
use git2::{Repository, StatusOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Change {
    pub path: PathBuf,
    pub staged: bool,
    pub worktree: bool,
}

pub fn open(path: impl AsRef<Path>) -> Result<Repository> {
    Ok(Repository::discover(path)?)
}

pub fn status(path: impl AsRef<Path>) -> Result<Vec<Change>> {
    let repo = open(path)?;
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options))?;
    Ok(statuses
        .iter()
        .filter_map(|entry| {
            let path = entry.path().map(PathBuf::from)?;
            let flags = entry.status();
            Some(Change {
                path,
                staged: flags.is_index_new()
                    || flags.is_index_modified()
                    || flags.is_index_deleted()
                    || flags.is_index_renamed()
                    || flags.is_index_typechange(),
                worktree: flags.is_wt_new()
                    || flags.is_wt_modified()
                    || flags.is_wt_deleted()
                    || flags.is_wt_renamed()
                    || flags.is_wt_typechange(),
            })
        })
        .collect())
}

pub fn branches(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let repo = open(path)?;
    Ok(repo
        .branches(None)?
        .filter_map(|item| {
            let (branch, _) = item.ok()?;
            branch.name().ok().flatten().map(str::to_owned)
        })
        .collect())
}

pub fn create_branch(path: impl AsRef<Path>, name: &str) -> Result<()> {
    let repo = open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    repo.branch(name, &head, false)?;
    Ok(())
}

pub fn checkout(path: impl AsRef<Path>, name: &str) -> Result<()> {
    let repo = open(path)?;
    let reference = repo.find_reference(&format!("refs/heads/{name}"))?;
    let object = reference.peel(git2::ObjectType::Commit)?;
    repo.checkout_tree(&object, None)?;
    repo.set_head(&format!("refs/heads/{name}"))?;
    Ok(())
}

pub fn stage_all(path: impl AsRef<Path>) -> Result<()> {
    let repo = open(path)?;
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    Ok(())
}

pub fn commit(path: impl AsRef<Path>, message: &str) -> Result<String> {
    let repo = open(path)?;
    let signature = repo.signature().context("Git identity is not configured")?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let parent = repo.head()?.peel_to_commit()?;
    let commit = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent],
    )?;
    Ok(commit.to_string())
}


#[derive(Debug, Clone)]
pub struct CommitSummary {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

pub fn current_branch(path: impl AsRef<Path>) -> Result<Option<String>> {
    let repo = open(path)?;
    Ok(repo.head()?.shorthand().map(str::to_owned))
}

pub fn head_commit(path: impl AsRef<Path>) -> Result<String> {
    let repo = open(path)?;
    Ok(repo.head()?.peel_to_commit()?.id().to_string())
}

pub fn recent_commits(path: impl AsRef<Path>, limit: usize) -> Result<Vec<CommitSummary>> {
    let repo = open(path)?;
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TIME)?;
    let mut commits = Vec::new();
    for oid in walk.take(limit) {
        let commit = repo.find_commit(oid?)?;
        commits.push(CommitSummary {
            id: commit.id().to_string(),
            message: commit.summary().unwrap_or_default().to_string(),
            author: commit.author().name().unwrap_or_default().to_string(),
            timestamp: commit.time().seconds(),
        });
    }
    Ok(commits)
}

pub fn diff(path: impl AsRef<Path>) -> Result<String> {
    let repo = open(path)?;
    let mut options = git2::DiffOptions::new();
    options.include_untracked(true);
    let diff = repo.diff_index_to_workdir(None, Some(&mut options))?;
    let mut output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        output.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })?;
    Ok(output)
}
