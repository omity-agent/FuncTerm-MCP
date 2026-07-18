use anyhow::{Context as _, Result};
use atomicwrites::{AllowOverwrite, AtomicFile, DisallowOverwrite};
use std::fs;
use std::io::Write as _;
use std::path::Path;
pub(crate) fn write_once(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if destination.exists() {
        return Ok(());
    }
    publish_once(destination, |file| file.write_all(contents.as_ref()))
}
pub(crate) fn copy_once(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        return Ok(());
    }
    let mut source_file = fs::File::open(source)
        .with_context(|| format!("failed to open copied file {}", source.display()))?;
    let permissions = source_file
        .metadata()
        .with_context(|| format!("failed to read copied file metadata {}", source.display()))?
        .permissions();
    publish_once(destination, |destination_file| {
        std::io::copy(&mut source_file, destination_file)?;
        destination_file.set_permissions(permissions)
    })
}
pub(crate) fn write_replace(destination: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    prepare_parent(destination)?;
    AtomicFile::new(destination, AllowOverwrite)
        .write::<_, std::io::Error, _>(|file| file.write_all(contents.as_ref()))
        .with_context(|| {
            format!(
                "failed to atomically replace file {}",
                destination.display()
            )
        })
}
fn publish_once(
    destination: &Path,
    operation: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
    prepare_parent(destination)?;
    match AtomicFile::new(destination, DisallowOverwrite).write(operation) {
        Ok(()) => Ok(()),
        Err(atomicwrites::Error::Internal(error))
            if error.kind() == std::io::ErrorKind::AlreadyExists =>
        {
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
fn prepare_parent(destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("published file has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create published file directory {}",
            parent.display()
        )
    })
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
