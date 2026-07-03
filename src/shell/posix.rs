use anyhow::Result;
use std::path::Path;
pub(super) fn sh_path(path: &Path) -> Result<String> {
    crate::shell::quote::native_path(path)
}
#[must_use]
pub(super) fn sh_quote(value: &str) -> String {
    crate::shell::quote::posix_string(value)
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn preserves_native_path_text_for_posix_shells() {
        let converted = super::sh_path(Path::new("F:\\dir\\child")).unwrap();
        assert_eq!(converted, "F:\\dir\\child");
    }
    #[test]
    fn quotes_single_quotes_for_posix_shells() {
        let quoted = super::sh_quote("a'b");
        assert_eq!(quoted, "'a'\\''b'");
    }
}
