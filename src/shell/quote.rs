use anyhow::Result;
use std::path::Path;
#[inline]
pub fn native_path(path: &Path) -> Result<String> {
    crate::text::path_text(path, "shell path")
}
#[inline]
pub fn powershell_path(path: &Path) -> Result<String> {
    Ok(powershell_string(&native_path(path)?))
}
#[must_use]
#[inline]
pub fn powershell_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
#[inline]
pub fn nushell_path(path: &Path) -> Result<String> {
    Ok(nushell_string(&native_path(path)?))
}
#[must_use]
#[inline]
pub fn nushell_string(value: &str) -> String {
    let text = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{text}\"")
}
#[must_use]
#[inline]
pub fn posix_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn quotes_literal_paths_for_powershell() {
        let quoted = super::powershell_path(Path::new("F:\\dir with ' quote")).unwrap();
        assert_eq!(quoted, "'F:\\dir with '' quote'");
    }
    #[test]
    fn quotes_single_quotes_for_posix_shells() {
        let quoted = super::posix_string("a'b");
        assert_eq!(quoted, "'a'\\''b'");
    }
    #[test]
    fn quotes_single_quotes_for_nushell() {
        let quoted = super::nushell_string("a'b");
        assert_eq!(quoted, "\"a'b\"");
    }
}
