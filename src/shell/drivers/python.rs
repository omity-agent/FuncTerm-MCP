mod bootstrap;
use super::{DriverStartup, InvocationTerminator, ShellDriver, ShellInvocation, StartupContext};
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result};
pub(crate) struct PythonDriver;
impl ShellDriver for PythonDriver {
    fn display_name(&self) -> &'static str {
        "Python"
    }
    fn shim_executable_names(&self) -> &'static [&'static str] {
        &[
            "python",
            "python.exe",
            "python3",
            "python3.exe",
            "pypy3",
            "pypy3.exe",
        ]
    }
    fn shim_env_name(&self) -> &'static str {
        "FUNCTERM_REAL_PYTHON"
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(settings.python.clone())
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
        let script = context.startup_directory.join("python_repl.py");
        std::fs::write(&script, bootstrap::script(context)?)
            .context("failed to write Python REPL bootstrap")?;
        Ok(DriverStartup {
            args: vec![
                "-i".to_owned(),
                "-u".to_owned(),
                crate::text::path_text(&script, "Python REPL bootstrap path")?,
            ],
            env: Vec::new(),
        })
    }
    fn invocation_terminator(&self) -> InvocationTerminator {
        platform_terminator()
    }
    fn invocation(&self) -> Result<Option<ShellInvocation>> {
        ShellInvocation::new(
            "_functerm_dispatch()".to_owned(),
            self.invocation_terminator(),
        )
        .map(Some)
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        arguments.iter().all(|argument| {
            argument
                .to_str()
                .is_some_and(|value| matches!(value, "-i" | "-u" | "-q"))
        })
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
