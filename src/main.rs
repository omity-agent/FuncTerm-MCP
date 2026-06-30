mod cli;
mod client;
mod config;
mod daemon;
mod ipc;
mod mcp;
mod session;
mod shell;
extern crate alloc;
use anyhow::Result;
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
