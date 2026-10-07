use anyhow::Result;
#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<std::process::ExitCode> {
    functerm::run().await
}
