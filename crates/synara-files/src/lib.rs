use anyhow::{bail,Result};use std::path::{Path,PathBuf};
pub fn confined(root:impl AsRef<Path>,candidate:impl AsRef<Path>)->Result<PathBuf>{let root=root.as_ref().canonicalize()?;let path=if candidate.as_ref().is_absolute(){candidate.as_ref().to_path_buf()}else{root.join(candidate)}.canonicalize()?;if !path.starts_with(&root){bail!("path escapes workspace boundary")}Ok(path)}
pub fn read_text(root:impl AsRef<Path>,path:impl AsRef<Path>)->Result<String>{Ok(std::fs::read_to_string(confined(root,path)?)?)}
