mod cli;
mod commands;
mod mcp;
mod runtime;
mod shell;
mod shim;
extern crate alloc;
use anyhow::Result;
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
#[tokio::main]
async fn main() -> Result<()> {
    if let Some(code) = shim::run_if_requested()? {
        std::process::exit(code);
    }
    cli::run().await
}
