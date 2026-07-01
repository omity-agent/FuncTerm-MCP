use anyhow::{Context as _, Result};
use std::path::PathBuf;
const DAEMON_ROOT: &str = "shell-mcp-pty";
const ICEORYX_ROOT: &str = "shell-mcp-iceoryx2";
pub(crate) fn daemon_root() -> Result<PathBuf> {
    create_root(DAEMON_ROOT)
}
pub(crate) fn iceoryx_root() -> Result<PathBuf> {
    create_root(ICEORYX_ROOT)
}
fn create_root(name: &str) -> Result<PathBuf> {
    let root = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&root)
        .with_context(|| format!("failed to create temporary directory {}", root.display()))?;
    Ok(root)
}
