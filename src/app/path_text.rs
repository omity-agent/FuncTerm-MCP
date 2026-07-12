use anyhow::Result;
use std::path::Path;
pub(crate) fn path_text(path: &Path, role: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{role} is not valid UTF-8: {}", path.display()))
}
