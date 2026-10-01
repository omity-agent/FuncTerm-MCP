use crate::runtime::protocol::KeyboardInput;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
pub(super) struct NewTabExec {
    pub(super) starting_directory: Option<String>,
    pub(super) starting_shell: ShellChoice,
}
impl NewTabExec {
    pub(super) fn starting_directory_path(&self) -> Option<&Path> {
        self.starting_directory.as_deref().map(Path::new)
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
pub(super) struct ManualWriteExec {
    pub(super) tab_id: String,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) bytes: Option<Vec<u8>>,
}
impl ManualWriteExec {
    pub(super) fn into_parts(self) -> Result<(String, KeyboardInput)> {
        let Self {
            tab_id,
            text,
            bytes,
        } = self;
        let input = match (text, bytes) {
            (Some(input_text), None) => KeyboardInput::Text(input_text),
            (None, Some(input_bytes)) => KeyboardInput::Bytes(input_bytes),
            (Some(_), Some(_)) => bail!("text and bytes cannot be provided together"),
            (None, None) => bail!("either text or bytes must be provided"),
        };
        Ok((tab_id, input))
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
pub(super) struct SendCommandExec {
    pub(super) tab_id: String,
    pub(super) command: String,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewRequest {
    #[schemars(length(min = 1))]
    pub(super) ids: Vec<String>,
    #[schemars(range(min = 0))]
    pub(super) wait_timeout: f64,
}
