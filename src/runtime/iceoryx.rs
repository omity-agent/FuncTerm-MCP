use anyhow::{Context as _, Result};
use iceoryx2::config::Config;
use iceoryx2_bb_system_types::path::{Path, SemanticString as _};
pub(crate) fn config() -> Result<Config> {
    let root = std::env::temp_dir()
        .join("agent")
        .join("shell-mcp-iceoryx2");
    std::fs::create_dir_all(&root).context("failed to create iceoryx2 root directory")?;
    let root_text = root.to_string_lossy();
    let root_path = Path::new(root_text.as_bytes()).context("invalid iceoryx2 root path")?;
    let mut config = Config::default();
    config.global.set_root_path(&root_path);
    Ok(config)
}
