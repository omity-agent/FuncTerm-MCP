use crate::contract::{SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY};
use anyhow::{Context as _, Result};
use std::path::PathBuf;
const DAEMON_ROOT: &str = "functerm";
const GENERATIONS_DIRECTORY: &str = "generations";
const SERVICE_HASH_LENGTH: usize = 12;
const SERVICES_DIRECTORY: &str = "services";
const SHIMS_DIRECTORY: &str = "shims";
const TABS_DIRECTORY: &str = "tabs";
pub(crate) fn daemon_root() -> Result<PathBuf> {
    create_root(&daemon_root_name())
}
pub(crate) fn generation_root(
    root: &std::path::Path,
    service_name: &str,
    generation: &str,
) -> PathBuf {
    service_root(root, service_name)
        .join(GENERATIONS_DIRECTORY)
        .join(generation)
}
pub(crate) fn shim_directory(generation_root: &std::path::Path) -> PathBuf {
    generation_root.join(SHIMS_DIRECTORY)
}
pub(crate) fn tab_commands_directory(tab_root: &std::path::Path) -> PathBuf {
    tab_root.join(SESSION_COMMANDS_DIRECTORY)
}
pub(crate) fn tab_root(generation_root: &std::path::Path, tab_id: &str) -> PathBuf {
    generation_root.join(TABS_DIRECTORY).join(tab_id)
}
pub(crate) fn tab_state_directory(tab_root: &std::path::Path) -> PathBuf {
    tab_root.join(SESSION_STATE_DIRECTORY)
}
pub(crate) fn remove_stale_service_runtime(root: &std::path::Path, service_name: &str) {
    if let Err(error) = remove_directory_if_present(&service_root(root, service_name)) {
        eprintln!("{error:#}");
    }
}
fn service_root(root: &std::path::Path, service_name: &str) -> PathBuf {
    root.join(SERVICES_DIRECTORY)
        .join(service_directory_name(service_name))
}
fn service_directory_name(service_name: &str) -> String {
    let slug = service_slug(service_name);
    let hash = blake3::hash(service_name.as_bytes())
        .to_hex()
        .chars()
        .take(SERVICE_HASH_LENGTH)
        .collect::<String>();
    format!("{slug}-{hash}")
}
fn service_slug(service_name: &str) -> String {
    let mut slug = String::new();
    for character in service_name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "service".to_owned()
    } else {
        trimmed.to_owned()
    }
}
#[cfg(unix)]
fn daemon_root_name() -> String {
    let uid = unsafe { libc::geteuid() };
    format!("{DAEMON_ROOT}/{DAEMON_ROOT}-{uid}")
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
fn remove_directory_if_present(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to remove stale runtime directory {}",
                path.display()
            )
        }),
    }
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
