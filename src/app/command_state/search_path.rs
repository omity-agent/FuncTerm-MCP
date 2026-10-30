use crate::shell::shims::{PATH_ENV, SHIM_DIR_ENV};
use anyhow::{Context as _, Result, ensure};
use std::path::{Path, PathBuf};
use sugar_path::SugarPath as _;
pub(crate) fn cmd_search_path(startup_directory: &Path) -> Result<String> {
    let shim_directory =
        PathBuf::from(std::env::var_os(SHIM_DIR_ENV).context("missing managed shim directory")?);
    let prefixes = [shim_directory.normalize(), startup_directory.normalize()];
    let mut entries = prefixes
        .iter()
        .map(|prefix| prefix.to_path_buf())
        .collect::<Vec<_>>();
    if let Some(inherited) = std::env::var_os(PATH_ENV) {
        entries.extend(std::env::split_paths(&inherited).filter(|entry| {
            let normalized = entry.normalize();
            !prefixes.iter().any(|prefix| {
                normalized
                    .as_os_str()
                    .eq_ignore_ascii_case(prefix.as_os_str())
            })
        }));
    }
    let joined = std::env::join_paths(entries).context("failed to join CMD search path")?;
    let text = joined
        .to_str()
        .context("CMD search path is not valid Unicode")?;
    ensure!(
        !text.contains(['\r', '\n']),
        "CMD search path contains a line break"
    );
    Ok(text.to_owned())
}
