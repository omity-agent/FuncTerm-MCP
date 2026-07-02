use std::path::Path;
pub(super) fn sh_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
pub(super) fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn converts_windows_paths_for_posix_shells() {
        let converted = super::sh_path(Path::new("F:\\dir\\child"));
        assert_eq!(converted, "F:/dir/child");
    }
    #[test]
    fn quotes_single_quotes_for_posix_shells() {
        let quoted = super::sh_quote("a'b");
        assert_eq!(quoted, "'a'\\''b'");
    }
}
