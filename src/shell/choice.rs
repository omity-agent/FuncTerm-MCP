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
}
