#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]
mod app;
mod engine;
mod file_publish;
pub(crate) use app::{cli, commands, contract, path_text as text};
pub(crate) use engine::{mcp, runtime, shim};
pub mod shell;
#[cfg(test)]
#[path = "../tests/support/temp.rs"]
pub(crate) mod test_fs;
extern crate alloc;
use anyhow::{Context as _, Result};
#[inline]
pub async fn run() -> Result<std::process::ExitCode> {
    if let Some(code) = shim::run_if_requested()? {
        let exit_code = u8::try_from(code).context("shim exit code is outside 0..=255")?;
        return Ok(std::process::ExitCode::from(exit_code));
    }
    cli::run().await?;
    Ok(std::process::ExitCode::SUCCESS)
}
