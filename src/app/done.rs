use anyhow::{Context as _, Result};
use serde::Serialize;
use std::path::Path;
#[derive(Serialize)]
pub(crate) struct DoneOutput<'value> {
    pub(crate) command_id: &'value str,
    pub(crate) exit_code: i32,
    pub(crate) time_consumption: &'value str,
    pub(crate) cwd: &'value str,
}
pub(crate) fn write(done: &DoneOutput<'_>, directory: &Path) -> Result<()> {
    let state_dir = directory.join(crate::contract::COMMAND_STATE_DIRECTORY);
    let done_path = state_dir.join(crate::contract::DONE_FILE);
    let text = sonic_rs::to_string(done).context("failed to serialize done file")?;
    crate::file_publish::write_once(&done_path, text).context("failed to publish done file")
}
