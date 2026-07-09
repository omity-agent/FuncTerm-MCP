use anyhow::{Context as _, Result};
use serde::Serialize;
use std::fs;
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
    if done_path.exists() {
        return Ok(());
    }
    fs::create_dir_all(&state_dir).with_context(|| {
        format!(
            "failed to create command state directory {}",
            state_dir.display()
        )
    })?;
    let text = sonic_rs::to_string(done).context("failed to serialize done file")?;
    let temp_path = state_dir.join(crate::contract::DONE_TEMP_FILE);
    fs::write(&temp_path, text)
        .with_context(|| format!("failed to write done file {}", temp_path.display()))?;
    match fs::rename(&temp_path, &done_path) {
        Ok(()) => Ok(()),
        Err(_error) if done_path.exists() => {
            fs::remove_file(&temp_path).with_context(|| {
                format!(
                    "failed to remove obsolete done file {}",
                    temp_path.display()
                )
            })?;
            Ok(())
        }
        Err(error) => Err(error).context("failed to publish done file"),
    }
}
