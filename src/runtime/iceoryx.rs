use anyhow::{Context as _, Result};
use iceoryx2::config::Config;
use iceoryx2_bb_system_types::path::{Path, SemanticString as _};
pub(crate) fn config() -> Result<Config> {
    let root = crate::runtime::temp::iceoryx_root()?;
    let root_text = root.to_string_lossy();
    let root_path = Path::new(root_text.as_bytes()).context("invalid iceoryx2 root path")?;
    let mut config = Config::default();
    config.global.set_root_path(&root_path);
    Ok(config)
}
