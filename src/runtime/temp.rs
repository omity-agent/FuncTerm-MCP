use anyhow::{Context as _, Result};
use std::path::PathBuf;
const DAEMON_ROOT: &str = "functerm";
pub(crate) fn daemon_root() -> Result<PathBuf> {
    create_root(&daemon_root_name())
}
#[cfg(unix)]
fn daemon_root_name() -> String {
    let uid = unsafe { libc::geteuid() };
    format!("{DAEMON_ROOT}-{uid}")
}
#[cfg(not(unix))]
fn daemon_root_name() -> String {
    DAEMON_ROOT.to_owned()
}
fn create_root(name: &str) -> Result<PathBuf> {
    let root = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&root)
        .with_context(|| format!("failed to create temporary directory {}", root.display()))?;
    #[cfg(unix)]
    secure_root(&root)?;
    Ok(root)
}
#[cfg(unix)]
fn secure_root(root: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let metadata = std::fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect temporary directory {}", root.display()))?;
    if !metadata.file_type().is_dir() {
        anyhow::bail!("temporary root is not a directory: {}", root.display());
    }
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid {
        anyhow::bail!(
            "temporary root is not owned by current user: {}",
            root.display()
        );
    }
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to secure temporary directory {}", root.display()))?;
    let secured = std::fs::metadata(root).with_context(|| {
        format!(
            "failed to inspect secured temporary directory {}",
            root.display()
        )
    })?;
    if secured.mode() & 0o777 != 0o700 {
        anyhow::bail!(
            "temporary root permissions are not 0700: {}",
            root.display()
        );
    }
    Ok(())
}
