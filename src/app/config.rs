use alloc::collections::BTreeMap;
use anyhow::{Context as _, Result, ensure};
use core::time::Duration;
use serde::Deserialize;
const SETTINGS: &str = include_str!("../../settings.toml");
pub(crate) const DAEMON_SERVICE_NAME_ENV: &str = "FUNCTERM_DAEMON_SERVICE_NAME";
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) daemon_service_name: String,
    pub(crate) diagnostics_filter: String,
    pub(crate) terminal_rows: u16,
    pub(crate) terminal_cols: u16,
    pub(crate) terminal_model_title: String,
    pub(crate) shell_startup_timeout_seconds: f64,
    pub(crate) shell_load_profile: bool,
    pub(crate) powershell: Vec<String>,
    pub(crate) bash: Vec<String>,
    pub(crate) nushell: Vec<String>,
    pub(crate) zsh: Vec<String>,
    pub(crate) cmd: Vec<String>,
    pub(crate) bun: Vec<String>,
    pub(crate) python: Vec<String>,
    pub(crate) ipc: IpcSettings,
    pub(crate) mcp: McpSettings,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IpcSettings {
    pub(crate) setup_timeout_seconds: f64,
    pub(crate) max_frame_bytes: usize,
    pub(crate) blocking_concurrency: usize,
}
impl IpcSettings {
    pub(crate) fn setup_timeout(&self) -> Result<Duration> {
        Duration::try_from_secs_f64(self.setup_timeout_seconds)
            .context("ipc.setup_timeout_seconds must be finite and non-negative")
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.setup_timeout()?.is_zero(),
            "IPC setup timeout is zero"
        );
        ensure!(
            self.max_frame_bytes > 0 && u32::try_from(self.max_frame_bytes).is_ok(),
            "ipc.max_frame_bytes must be in 1..=4294967295"
        );
        ensure!(
            self.blocking_concurrency > 0
                && self.blocking_concurrency <= tokio::sync::Semaphore::MAX_PERMITS,
            "ipc.blocking_concurrency is outside the supported semaphore range"
        );
        Ok(())
    }
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
    #[serde(default)]
    pub(crate) exec_parameters: BTreeMap<String, String>,
}
pub(crate) fn load() -> Result<Settings> {
    let mut settings =
        toml::from_str::<Settings>(SETTINGS).context("failed to parse embedded settings")?;
    settings.ipc.validate()?;
    apply_string_override(DAEMON_SERVICE_NAME_ENV, &mut settings.daemon_service_name);
    for (name, candidates) in [
        ("FUNCTERM_POWERSHELL", &mut settings.powershell),
        ("FUNCTERM_BASH", &mut settings.bash),
        ("FUNCTERM_NUSHELL", &mut settings.nushell),
        ("FUNCTERM_ZSH", &mut settings.zsh),
        ("FUNCTERM_CMD", &mut settings.cmd),
        ("FUNCTERM_BUN", &mut settings.bun),
        ("FUNCTERM_PYTHON", &mut settings.python),
    ] {
        apply_list_override(name, candidates);
    }
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
