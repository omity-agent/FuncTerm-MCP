use crate::runtime::config::Settings;
use serde::{Deserialize, Serialize};
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    PartialEq,
    Eq,
    Serialize,
    strum :: EnumString,
    strum :: IntoStaticStr,
    schemars :: JsonSchema,
    strum :: VariantArray,
    strum :: VariantNames,
)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum ShellChoice {
    #[serde(rename = "powershell")]
    PowerShell,
    #[serde(rename = "bash")]
    Bash,
    #[serde(rename = "nu")]
    #[strum(serialize = "nu")]
    NuShell,
    #[serde(rename = "zsh")]
    Zsh,
    #[serde(rename = "cmd")]
    Cmd,
    #[serde(rename = "bun")]
    Bun,
    #[serde(rename = "python")]
    Python,
}
impl ShellChoice {
    pub(crate) fn canonical_name(self) -> &'static str {
        let name: &'static str = self.into();
        name
    }
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Bash => "Bash",
            Self::NuShell => "NuShell",
            Self::Zsh => "Zsh",
            Self::Cmd => "Windows CMD",
            Self::Bun => "Bun",
            Self::Python => "Python",
        }
    }
    pub(crate) const fn shim_env_name(self) -> &'static str {
        match self {
            Self::PowerShell => "FUNCTERM_REAL_POWERSHELL",
            Self::Bash => "FUNCTERM_REAL_BASH",
            Self::NuShell => "FUNCTERM_REAL_NUSHELL",
            Self::Zsh => "FUNCTERM_REAL_ZSH",
            Self::Cmd => "FUNCTERM_REAL_CMD",
            Self::Bun => "FUNCTERM_REAL_BUN",
            Self::Python => "FUNCTERM_REAL_PYTHON",
        }
    }
    pub(crate) const fn shim_executable_names(self) -> &'static [&'static str] {
        match self {
            Self::PowerShell => &[
                "pwsh",
                "pwsh.exe",
                "powershell",
                "powershell.exe",
                "powershell_core",
                "windows_powershell",
            ],
            Self::Bash => &["bash", "bash.exe"],
            Self::NuShell => &["nu", "nu.exe", "nushell", "nushell.exe"],
            Self::Zsh => &["zsh"],
            Self::Cmd => &["cmd", "cmd.exe"],
            Self::Bun => &["bun", "bun.exe"],
            Self::Python => &[
                "python",
                "python.exe",
                "python3",
                "python3.exe",
                "pypy3",
                "pypy3.exe",
            ],
        }
    }
    pub(crate) fn executable_candidates(self, settings: &Settings) -> &[String] {
        match self {
            Self::PowerShell => &settings.powershell,
            Self::Bash => &settings.bash,
            Self::NuShell => &settings.nushell,
            Self::Zsh => &settings.zsh,
            Self::Cmd => &settings.cmd,
            Self::Bun => &settings.bun,
            Self::Python => &settings.python,
        }
    }
}
