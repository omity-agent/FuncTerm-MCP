use std::path::PathBuf;
const FUNCTERM_DIRECTORY: &str = "functerm";
const TEST_DIRECTORY: &str = "tests";
pub(crate) fn temp_root() -> PathBuf {
    let root = std::env::temp_dir()
        .join(FUNCTERM_DIRECTORY)
        .join(TEST_DIRECTORY);
    std::fs::create_dir_all(&root).unwrap();
    root
}
pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let path = temp_root()
        .join(name)
        .join(format!("{}-{}", std::process::id(), nanoid::nanoid!()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
pub(crate) fn temp_env() -> Vec<(String, String)> {
    let root = temp_root();
    let text = root.to_string_lossy().into_owned();
    platform_temp_env(text)
}
#[cfg(windows)]
fn platform_temp_env(text: String) -> Vec<(String, String)> {
    vec![("TMP".to_owned(), text.clone()), ("TEMP".to_owned(), text)]
}
#[cfg(not(windows))]
fn platform_temp_env(text: String) -> Vec<(String, String)> {
    vec![("TMPDIR".to_owned(), text)]
}
