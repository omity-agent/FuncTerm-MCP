use anyhow::{Context as _, Result};
use iceoryx2::config::Config;
use iceoryx2_bb_system_types::path::{Path, SemanticString as _};
use std::path::PathBuf;
const ICEORYX_ROOT: &str = "shell-mcp-iceoryx2";
pub(crate) fn config() -> Result<Config> {
    let root = root()?;
    let root_text = root.to_string_lossy();
    let root_path = Path::new(root_text.as_bytes()).context("invalid iceoryx2 root path")?;
    let mut config = Config::default();
    config.global.set_root_path(&root_path);
    Ok(config)
}
fn root() -> Result<PathBuf> {
    let root = std::env::temp_dir().join(ICEORYX_ROOT);
    std::fs::create_dir_all(&root)
        .with_context(|| format!("failed to create temporary directory {}", root.display()))?;
    Ok(root)
}
