use anyhow::{Context as _, Result};
use serde::Deserialize;
const SETTINGS: &str = include_str!("../settings.toml");
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) daemon_address: String,
    pub(crate) terminal_rows: u16,
    pub(crate) terminal_cols: u16,
    pub(crate) windows_powershell: String,
    pub(crate) pwsh: String,
    pub(crate) bash: String,
    pub(crate) nushell: String,
}
pub(crate) fn load() -> Result<Settings> {
    toml::from_str(SETTINGS).context("failed to parse embedded settings")
}
