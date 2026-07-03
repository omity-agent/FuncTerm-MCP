use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;
pub(crate) fn path_text(path: &Path, role: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{role} is not valid UTF-8: {}", path.display()))
}
pub(crate) fn os_text(value: OsString, role: &str) -> Result<String> {
    value
        .into_string()
        .map_err(|text| anyhow::anyhow!("{role} is not valid UTF-8: {}", text.to_string_lossy()))
}
