mod cli;
mod mcp;
mod runtime;
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
