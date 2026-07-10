use anyhow::{Context as _, Result, anyhow};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
pub(crate) fn write_once(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if destination.exists() {
        return Ok(());
    }
    let staged = stage(destination, |path| {
        fs::write(path, contents.as_ref())
            .with_context(|| format!("failed to stage file {}", path.display()))
    })?;
    publish_once(&staged, destination)
}
pub(crate) fn copy_once(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        return Ok(());
    }
    let staged = stage(destination, |path| {
        fs::copy(source, path)
            .with_context(|| {
                format!(
                    "failed to stage file copy from {} to {}",
                    source.display(),
                    path.display()
                )
            })
            .map(|_| ())
    })?;
    publish_once(&staged, destination)
}
pub(crate) fn write_replace(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let staged = stage(destination, |path| {
        fs::write(path, contents.as_ref())
            .with_context(|| format!("failed to stage replacement {}", path.display()))
    })?;
    if let Err(error) = replace_file(&staged, destination) {
        return Err(cleanup_error(
            &staged,
            error.context(format!("failed to replace file {}", destination.display())),
        ));
    }
    Ok(())
}
fn stage(destination: &Path, operation: impl FnOnce(&Path) -> Result<()>) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .context("published file has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create published file directory {}",
            parent.display()
        )
    })?;
    let staged = temporary_path(destination)?;
    if let Err(error) = operation(&staged) {
        return Err(cleanup_error(&staged, error));
    }
    Ok(staged)
}
fn publish_once(staged: &Path, destination: &Path) -> Result<()> {
    match fs::hard_link(staged, destination) {
        Ok(()) => remove_staged(staged),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists || destination.exists() => {
            remove_staged(staged)
        }
        Err(error) => Err(cleanup_error(
            staged,
            anyhow!(error).context(format!("failed to publish file {}", destination.display())),
        )),
    }
}
fn temporary_path(destination: &Path) -> Result<PathBuf> {
    let file_name = destination
        .file_name()
        .context("published file path has no file name")?;
    let mut temporary_name = OsString::from(file_name);
    temporary_name.push(format!(".{}.{}.tmp", std::process::id(), nanoid::nanoid!()));
    Ok(destination.with_file_name(temporary_name))
}
fn remove_staged(path: &Path) -> Result<()> {
    fs::remove_file(path)
        .with_context(|| format!("failed to remove staged file {}", path.display()))
}
fn cleanup_error(path: &Path, operation_error: anyhow::Error) -> anyhow::Error {
    match fs::remove_file(path) {
        Ok(()) => operation_error,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => operation_error,
        Err(error) => anyhow!(
            "{operation_error:#}; additionally failed to remove staged file {}: {error}",
            path.display()
        ),
    }
}
#[cfg(unix)]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).map_err(Into::into)
}
#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source_wide = wide_path(source);
    let destination_wide = wide_path(destination);
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    let moved = unsafe { MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), flags) };
    if moved == 0_i32 {
        return Err(std::io::Error::last_os_error()).context("MoveFileExW failed");
    }
    Ok(())
}
#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str().encode_wide().chain([0]).collect()
}
#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    #[test]
    fn concurrent_write_once_publishes_one_complete_value() {
        let directory = crate::test_fs::temp_dir("file-publish-once");
        let destination = Arc::new(directory.join("result.txt"));
        let workers = (0_u8..8)
            .map(|value| {
                let path = Arc::clone(&destination);
                std::thread::spawn(move || {
                    let contents = vec![value; 4096];
                    super::write_once(&path, contents).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let contents = std::fs::read(destination.as_path()).unwrap();
        let first = contents.first().copied().unwrap();
        assert_eq!(contents.len(), 4096);
        assert!(contents.iter().all(|byte| *byte == first));
    }
    #[test]
    fn write_replace_overwrites_complete_value() {
        let directory = crate::test_fs::temp_dir("file-publish-replace");
        let destination = directory.join("state.txt");
        super::write_replace(&destination, "first").unwrap();
        super::write_replace(&destination, "second").unwrap();
        assert_eq!(std::fs::read_to_string(destination).unwrap(), "second");
    }
}
