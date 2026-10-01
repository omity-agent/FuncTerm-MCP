use anyhow::Result;
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<std::process::ExitCode> {
    functerm::run().await
}
