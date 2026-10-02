use itertools::Itertools as _;
use std::path::Path;
use std::path::PathBuf;
#[cfg(windows)]
const TEMP_VARIABLES: &[&str] = &["TMP", "TEMP"];
#[cfg(not(windows))]
const TEMP_VARIABLES: &[&str] = &["TMPDIR"];
pub(super) struct TestRuntime {
    directory: Option<tempfile::TempDir>,
}
impl TestRuntime {
    pub(super) fn new() -> Self {
        Self {
            directory: Some(
                tempfile::Builder::new()
                    .prefix("functerm-tests-")
                    .tempdir()
                    .unwrap(),
            ),
        }
    }
    pub(super) fn environment(&self) -> Vec<(String, String)> {
        let path = self.directory.as_ref().unwrap().path().to_str().unwrap();
        TEMP_VARIABLES
            .iter()
            .map(|name| ((*name).to_owned(), path.to_owned()))
            .collect()
    }
}
impl Drop for TestRuntime {
    fn drop(&mut self) {
        if let Some(directory) = self.directory.take()
            && let Err(error) = directory.close()
        {
            eprintln!("failed to clear isolated test runtime: {error}");
        }
    }
}
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
    let root = temp_root().join("functerm");
    #[cfg(unix)]
    let service_parent = root.join(format!("functerm-{}", rustix::process::geteuid().as_raw()));
    #[cfg(windows)]
    let service_parent = root;
    let service = sole_directory(&service_parent.join("services"));
    let generation = sole_directory(&service.join("generations"));
    generation
        .join("tabs")
        .join(tab_id)
        .join("commands")
        .join(command_id)
}
fn sole_directory(parent: &Path) -> PathBuf {
    std::fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .exactly_one()
        .unwrap()
}
