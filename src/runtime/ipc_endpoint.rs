use anyhow::{Context as _, Result};
use std::path::PathBuf;
const DAEMON_ROOT: &str = "functerm";
const IPC_ROOT: &str = "ipc-channel";
const ENDPOINT_FILE: &str = "endpoint.txt";
pub(crate) fn publish(service_name: &str, endpoint_name: &str) -> Result<()> {
    let path = endpoint_file(service_name);
    let parent = path
        .parent()
        .with_context(|| format!("endpoint path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create IPC directory {}", parent.display()))?;
    let temporary_path = path.with_extension("tmp");
    std::fs::write(&temporary_path, endpoint_name)
        .with_context(|| format!("failed to write IPC endpoint {}", temporary_path.display()))?;
    std::fs::rename(&temporary_path, &path)
        .with_context(|| format!("failed to publish IPC endpoint {}", path.display()))?;
    Ok(())
}
pub(crate) fn read(service_name: &str) -> Result<String> {
    let path = endpoint_file(service_name);
    std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read IPC endpoint {}", path.display()))
        .map(|name| name.trim().to_owned())
}
pub(crate) fn endpoint_file(service_name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(DAEMON_ROOT).join(IPC_ROOT);
    root.join(hex::encode(service_name)).join(ENDPOINT_FILE)
}
