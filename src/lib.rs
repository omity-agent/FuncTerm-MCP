mod cli;
mod commands;
mod contract;
mod mcp;
mod runtime;
pub mod shell;
mod shim;
mod text;
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
