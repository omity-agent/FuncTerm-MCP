mod bootstrap;
use super::{DriverStartup, InvocationTerminator, ShellDriver, ShellInvocation, StartupContext};
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result};
use std::path::Path;
pub(crate) struct BunDriver;
impl ShellDriver for BunDriver {
    fn display_name(&self) -> &'static str {
        "Bun"
    }
    fn shim_executable_names(&self) -> &'static [&'static str] {
        &["bun", "bun.exe"]
    }
    fn shim_env_name(&self) -> &'static str {
        "FUNCTERM_REAL_BUN"
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(vec![settings.bun.clone()])
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
        let script = context.startup_directory.join("bun_repl.mjs");
        std::fs::write(
            &script,
            bootstrap::script(&json_path(context.cwd)?, &json_path(context.ready_file)?),
        )
        .context("failed to write Bun REPL bootstrap")?;
        Ok(DriverStartup {
            args: vec![crate::text::path_text(&script, "Bun REPL bootstrap path")?],
            env: Vec::new(),
        })
    }
    fn invocation_terminator(&self) -> InvocationTerminator {
        platform_terminator()
    }
    fn invocation(&self) -> Result<Option<ShellInvocation>> {
        Ok(None)
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        matches ! (arguments , [argument] if argument == "repl" || argument == "--interactive")
    }
}
#[cfg(windows)]
const fn platform_terminator() -> InvocationTerminator {
    InvocationTerminator::CarriageReturn
}
#[cfg(not(windows))]
const fn platform_terminator() -> InvocationTerminator {
    InvocationTerminator::LineFeed
}
fn json_path(path: &Path) -> Result<String> {
    sonic_rs::to_string(&crate::text::path_text(path, "Bun bootstrap path")?)
        .context("failed to encode Bun bootstrap path")
}
