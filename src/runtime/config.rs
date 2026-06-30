use anyhow::{Context as _, Result};
use serde::Deserialize;
const SETTINGS: &str = include_str!("../../settings.toml");
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
    let mut settings =
        toml::from_str::<Settings>(SETTINGS).context("failed to parse embedded settings")?;
    apply_string_override("SHELL_MCP_PTY_DAEMON_ADDRESS", &mut settings.daemon_address);
    apply_string_override(
        "SHELL_MCP_PTY_WINDOWS_POWERSHELL",
        &mut settings.windows_powershell,
    );
    apply_string_override("SHELL_MCP_PTY_PWSH", &mut settings.pwsh);
    apply_string_override("SHELL_MCP_PTY_BASH", &mut settings.bash);
    apply_string_override("SHELL_MCP_PTY_NUSHELL", &mut settings.nushell);
    Ok(settings)
}
fn apply_string_override(name: &str, value: &mut String) {
    if let Ok(override_value) = std::env::var(name) {
        *value = override_value;
    }
}
