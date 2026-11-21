pub(crate) mod mcp;
pub(crate) mod privileges;
pub(crate) mod runtime;
pub(crate) mod shim;
pub(crate) fn init_diagnostics(filter: &str) -> anyhow::Result<()> {
    use anyhow::Context as _;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(filter).context("invalid diagnostics_filter")?,
        )
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize diagnostics: {error}"))
}
