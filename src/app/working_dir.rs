use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
pub(crate) fn resolve(input: Option<&Path>) -> Result<PathBuf> {
    let base = std::env::current_dir().context("failed to locate program working directory")?;
    let Some(raw_path) = input else {
        return Ok(base);
    };
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }
    Ok(base.join(raw_path))
}
