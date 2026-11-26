use core::time::Duration;
use itertools::Itertools as _;
use std::path::{Path, PathBuf};
use std::time::Instant;
#[cfg(windows)]
const TEMP_VARIABLES: &[&str] = &["TMP", "TEMP"];
#[cfg(not(windows))]
const TEMP_VARIABLES: &[&str] = &["TMPDIR"];
pub(super) struct TestRuntime(PathBuf);
impl TestRuntime {
    pub(super) fn new() -> Self {
        let root = std::env::temp_dir().join("functerm").join("tests");
        std::fs::create_dir_all(&root).unwrap();
        Self(
            tempfile::Builder::new()
                .prefix("runtime-")
                .tempdir_in(root)
                .unwrap()
                .keep(),
        )
    }
    pub(super) fn environment(&self) -> Vec<(String, String)> {
        let path = self.0.to_str().unwrap();
        TEMP_VARIABLES
            .iter()
            .map(|name| ((*name).to_owned(), path.to_owned()))
            .collect()
    }
}
impl Drop for TestRuntime {
    fn drop(&mut self) {
        if let Err(error) = remove_runtime(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("failed to clear isolated test runtime: {error}");
        }
    }
}
#[cfg(windows)]
fn remove_runtime(path: &Path) -> std::io::Result<()> {
    let started = Instant::now();
    loop {
        match std::fs::remove_dir_all(path) {
            Err(error)
                if error.raw_os_error() == Some(windows::Win32::ERROR_SHARING_VIOLATION)
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(15));
            }
            outcome => return outcome,
        }
    }
}
#[cfg(not(windows))]
use std::fs::remove_dir_all as remove_runtime;
pub(crate) fn temp_root() -> PathBuf {
    let environment = super::daemon::active_env();
    let directory = environment
        .iter()
        .find(|pair| pair.0 == *TEMP_VARIABLES.first().unwrap())
        .unwrap_or_else(|| panic!("temporary test paths require an active daemon guard"));
    PathBuf::from(&directory.1)
}
pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let path = temp_root()
        .join(name)
        .join(format!("{}-{}", std::process::id(), nanoid::nanoid!()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
pub(crate) fn command_directory(tab_id: &str, command_id: &str) -> PathBuf {
    tab_directory(tab_id).join("commands").join(command_id)
}
pub(crate) fn tab_directory(tab_id: &str) -> PathBuf {
    let root = temp_root().join("functerm");
    #[cfg(unix)]
    let service_parent = root.join(format!("functerm-{}", rustix::process::geteuid().as_raw()));
    #[cfg(windows)]
    let service_parent = root;
    let service = sole_directory(&service_parent.join("services"));
    let generation = sole_directory(&service.join("generations"));
    generation.join("tabs").join(tab_id)
}
pub(crate) fn wait_for_removal(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while path.try_exists().unwrap() {
        assert!(
            Instant::now() < deadline,
            "path was not reclaimed: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn sole_directory(parent: &Path) -> PathBuf {
    std::fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .exactly_one()
        .unwrap()
}
