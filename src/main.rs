use anyhow::Result;
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
#[tokio::main]
async fn main() -> Result<std::process::ExitCode> {
    functerm::run().await
}
