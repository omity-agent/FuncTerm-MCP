use anyhow::{Context as _, Result};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
pub(crate) fn write_once(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    publish_once(destination, |file| file.write_all(contents.as_ref()))
}
#[cfg(windows)]
pub(crate) fn copy_once(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        return Ok(());
    }
    let mut source_file = std::fs::File::open(source).with_context(|| {
        format!(
            "failed to open executable snapshot source {}",
            source.display()
        )
    })?;
    let permissions = source_file.metadata()?.permissions();
    publish_once(destination, |destination_file| {
        std::io::copy(&mut source_file, destination_file)?;
        destination_file.set_permissions(permissions)
    })
}
pub(crate) fn write_replace(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let mut temporary = temporary_sibling(destination)?;
    temporary.write_all(contents.as_ref())?;
    temporary.persist(destination).with_context(|| {
        format!(
            "failed to atomically replace file {}",
            destination.display()
        )
    })?;
    Ok(())
}
fn publish_once(
    destination: &Path,
    operation: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<()> {
    let mut temporary = temporary_sibling(destination)?;
    operation(temporary.as_file_mut())?;
    match temporary.persist_noclobber(destination) {
        Ok(_file) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            error
                .file
                .close()
                .context("failed to remove unpublished temporary file")?;
            Ok(())
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to atomically publish file {}",
                destination.display()
            )
        }),
    }
}
fn temporary_sibling(destination: &Path) -> Result<NamedTempFile> {
    let parent = destination
        .parent()
        .context("published file has no parent")?;
    fs_err::create_dir_all(parent)?;
    NamedTempFile::new_in(parent).context("failed to create atomic publication file")
}
pub(crate) fn path_text(path: &Path, role: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{role} is not valid UTF-8: {}", path.display()))
}
pub(crate) fn resolve(input: Option<&Path>) -> Result<PathBuf> {
    let base = std::env::current_dir().context("failed to locate program working directory")?;
    let Some(raw_path) = input else {
        return Ok(base);
    };
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }
    Ok(base.join(raw_path))
}
