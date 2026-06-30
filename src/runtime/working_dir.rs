use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
pub(crate) fn resolve(input: Option<&Path>) -> Result<PathBuf> {
    let base = std::env::current_dir().context("failed to locate program working directory")?;
    Ok(match input {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => base.join(path),
        None => base,
    })
}
