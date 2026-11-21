use crate::engine::privileges::{ShellIdentity, query_process};
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result, ensure};
use itertools::Itertools as _;
use semver::Version;
use std::path::{Path, PathBuf};
pub(super) fn select(
    candidates: impl Iterator<Item = PathBuf>,
    cwd: &Path,
    environment: &EnvironmentSnapshot,
    identity: &ShellIdentity,
) -> Result<PathBuf> {
    let (version, executable) = candidates
        .unique()
        .filter_map(|path| {
            let version = probe(&path, cwd, environment, identity)
                .inspect_err(|error| {
                    tracing::debug!(?path, error = %format_args!("{error:#}"), "PowerShell candidate is unusable");
                })
                .ok()?;
            Some((version, path))
        })
        .reduce(|best, candidate| {
            if candidate.0.cmp_precedence(&best.0).is_gt() {
                candidate
            } else {
                best
            }
        })
        .context("no PowerShell 7 or newer executable was found")?;
    tracing::debug!(?executable, %version, "selected PowerShell executable");
    Ok(executable)
}
fn probe(
    executable: &Path,
    cwd: &Path,
    environment: &EnvironmentSnapshot,
    identity: &ShellIdentity,
) -> Result<Version> {
    let output = query_process::query(executable, &["-Version"], cwd, environment, identity)?;
    let text = core::str::from_utf8(&output)?;
    let value = text
        .trim()
        .strip_prefix("PowerShell ")
        .context("unexpected PowerShell version response")?;
    let version = Version::parse(value)?;
    ensure!(
        version.major >= 7,
        "unsupported PowerShell version {version}"
    );
    Ok(version)
}
