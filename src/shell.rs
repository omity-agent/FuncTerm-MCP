use crate::config::Settings;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShellChoice {
    PowerShell,
    Pwsh,
}
impl ShellChoice {
    pub(crate) fn executable(self, settings: &Settings) -> &str {
        match self {
            Self::PowerShell => &settings.windows_powershell,
            Self::Pwsh => &settings.pwsh,
        }
    }
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "powershell" | "windows_powershell" => Ok(Self::PowerShell),
            "pwsh" | "powershell_core" => Ok(Self::Pwsh),
            other => bail!("unsupported shell `{other}`; only powershell and pwsh are supported"),
        }
    }
}
