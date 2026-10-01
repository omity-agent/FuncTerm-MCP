use anyhow::Result;
use base64_turbo::STANDARD;
use std::path::Path;
#[inline]
pub fn native_path(path: &Path) -> Result<String> {
    crate::text::path_text(path, "shell path")
}
#[must_use]
#[inline]
pub fn cmd_string(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
#[inline]
pub fn powershell_path(path: &Path) -> Result<String> {
    Ok(powershell_string(&native_path(path)?))
}
#[must_use]
#[inline]
pub fn powershell_string(value: &str) -> String {
    format!(
        "([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}')))",
        STANDARD.encode(value.as_bytes())
    )
}
#[inline]
pub fn nushell_path(path: &Path) -> Result<String> {
    Ok(nushell_string(&native_path(path)?))
}
#[must_use]
#[inline]
pub fn nushell_string(value: &str) -> String {
    format!(
        "('{}' | decode base64 | decode)",
        STANDARD.encode(value.as_bytes())
    )
}
#[must_use]
#[inline]
pub fn posix_string(value: &str) -> String {
    shell_words::quote(value).into_owned()
}
