#![cfg_attr(unix, feature(process_setsid))]
#![feature(once_cell_try)]
mod app;
mod engine;
pub(crate) use app::filesystem as publication;
pub(crate) use app::{cli, contract, filesystem as text};
pub(crate) use engine::{mcp, runtime, shim};
pub mod shell;
extern crate alloc;
use anyhow::Result;
#[inline]
pub async fn run() -> Result<std::process::ExitCode> {
    if let Some(code) = shim::run_if_requested()? {
        return Ok(shim::process_exit(code));
    }
    cli::run().await?;
    Ok(std::process::ExitCode::SUCCESS)
}
