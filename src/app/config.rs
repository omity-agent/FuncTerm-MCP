use alloc::collections::BTreeMap;
use anyhow::{Context as _, Result};
use serde::Deserialize;
const SETTINGS: &str = include_str!("../../settings.toml");
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) daemon_service_name: String,
    pub(crate) terminal_rows: u16,
    pub(crate) terminal_cols: u16,
    pub(crate) terminal_model_title: String,
    pub(crate) shell_startup_timeout_seconds: f64,
    pub(crate) powershell: Vec<String>,
    pub(crate) bash: String,
    pub(crate) nushell: String,
    pub(crate) zsh: String,
    pub(crate) cmd: String,
    pub(crate) bun: String,
    pub(crate) python: Vec<String>,
    pub(crate) mcp: McpSettings,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct McpSettings {
    pub(crate) new_tab: ToolDescription,
    pub(crate) manual_write: ToolDescription,
    pub(crate) send_command: ToolDescription,
    pub(crate) view: ToolDescription,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct ToolDescription {
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) parameters: BTreeMap<String, String>,
}
pub(crate) fn load() -> Result<Settings> {
    let mut settings =
        toml::from_str::<Settings>(SETTINGS).context("failed to parse embedded settings")?;
    apply_string_override(
        "FUNCTERM_DAEMON_SERVICE_NAME",
        &mut settings.daemon_service_name,
    );
    apply_list_override("FUNCTERM_POWERSHELL", &mut settings.powershell);
    apply_string_override("FUNCTERM_BASH", &mut settings.bash);
    apply_string_override("FUNCTERM_NUSHELL", &mut settings.nushell);
    apply_string_override("FUNCTERM_ZSH", &mut settings.zsh);
    apply_string_override("FUNCTERM_CMD", &mut settings.cmd);
    apply_string_override("FUNCTERM_BUN", &mut settings.bun);
    apply_list_override("FUNCTERM_PYTHON", &mut settings.python);
    Ok(settings)
}
fn apply_list_override(name: &str, value: &mut Vec<String>) {
    if let Ok(override_value) = std::env::var(name) {
        *value = override_value
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
    }
}
fn apply_string_override(name: &str, value: &mut String) {
    if let Ok(override_value) = std::env::var(name) {
        *value = override_value;
    }
}
