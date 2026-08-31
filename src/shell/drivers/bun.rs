mod bootstrap;
use super::{DriverStartup, StartupContext};
use anyhow::{Context as _, Result};
use std::path::Path;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let script = context.startup_directory.join("bun_repl.mjs");
    fs_err::write(
        &script,
        bootstrap::script(&json_path(context.cwd)?, &json_path(context.ready_file)?),
    )?;
    Ok(DriverStartup {
        args: vec![crate::text::path_text(&script, "Bun REPL bootstrap path")?],
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    matches ! (arguments , [argument] if argument == "repl" || argument == "--interactive")
}
fn json_path(path: &Path) -> Result<String> {
    sonic_rs::to_string(&crate::text::path_text(path, "Bun bootstrap path")?)
        .context("failed to encode Bun bootstrap path")
}
