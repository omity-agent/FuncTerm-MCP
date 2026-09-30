use crate::runtime::protocol::KeyboardInput;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabRequest {
    pub(super) starting_directory: Option<String>,
    pub(super) starting_shell: ShellChoice,
}
impl NewTabRequest {
    pub(super) fn starting_directory_path(&self) -> Option<&Path> {
        self.starting_directory.as_deref().map(Path::new)
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteRequest {
    pub(super) tab_id: String,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) bytes: Option<Vec<u8>>,
    pub(super) wait_timeout: f64,
}
impl ManualWriteRequest {
    pub(super) fn into_parts(self) -> Result<(String, KeyboardInput, f64)> {
        let Self {
            tab_id,
            text,
            bytes,
            wait_timeout,
        } = self;
        let input = match (text, bytes) {
            (Some(input_text), None) => KeyboardInput::Text(input_text),
            (None, Some(input_bytes)) => KeyboardInput::Bytes(input_bytes),
            (Some(_), Some(_)) => bail!("text and bytes cannot be provided together"),
            (None, None) => bail!("either text or bytes must be provided"),
        };
        Ok((tab_id, input, wait_timeout))
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandRequest {
    pub(super) tab_id: String,
    pub(super) command: String,
    pub(super) wait_timeout: f64,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ViewRequest {
    pub(super) id: String,
    pub(super) wait_timeout: f64,
}
#[cfg(test)]
#[path = "../../../tests/unit/app/tool_inputs.rs"]
mod tests;
