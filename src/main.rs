use anyhow::Result;
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<std::process::ExitCode> {
    functerm::run().await
}
