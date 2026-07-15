use std::path::PathBuf;
pub(crate) const FUNCTERM_DIRECTORY: &str = "functerm";
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
