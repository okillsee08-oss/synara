use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn confined(root: impl AsRef<Path>, candidate: impl AsRef<Path>) -> Result<PathBuf> {
    let root = root.as_ref().canonicalize()?;
    let path = if candidate.as_ref().is_absolute() {
        candidate.as_ref().to_path_buf()
    } else {
        root.join(candidate)
    };
    let resolved = if path.exists() {
        path.canonicalize()?
    } else {
        let parent = path.parent().ok_or_else(|| anyhow::anyhow!("path has no parent"))?;
        let parent = parent.canonicalize()?;
        parent.join(path.file_name().ok_or_else(|| anyhow::anyhow!("path has no filename"))?)
    };
    if !resolved.starts_with(&root) {
        bail!("path escapes workspace boundary")
    }
    Ok(resolved)
}

pub fn read_text(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(confined(root, path)?)?)
}

pub fn write_text(root: impl AsRef<Path>, path: impl AsRef<Path>, content: &str) -> Result<()> {
    let resolved = confined(root, path)?;
    fs::write(resolved, content)?;
    Ok(())
}
