use anyhow::Result;use git2::Repository;use std::path::Path;
pub fn open(path:impl AsRef<Path>)->Result<Repository>{Ok(Repository::discover(path)?) }
pub fn status(path:impl AsRef<Path>)->Result<Vec<String>>{let r=open(path)?;let mut o=Vec::new();for e in r.statuses(None)?.iter(){if let Some(p)=e.path(){o.push(p.to_string())}}Ok(o)}
