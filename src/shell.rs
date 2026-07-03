use crate::runtime::config::Settings;
mod choice;
mod drivers;
pub mod quote;
pub(crate) mod shims;
mod wrappers;
use alloc::borrow::Cow;
use anyhow::{Context as _, Result, bail};
pub(crate) use choice::ShellChoice;
use std::path::{Path, PathBuf};
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) ready_file: std::path::PathBuf,
}
impl ShellChoice {
    pub(crate) fn executable(self, settings: &Settings) -> Result<String> {
        crate::text::path_text(&self.executable_path(settings)?, "executable path")
    }
    pub(crate) fn executable_path(self, settings: &Settings) -> Result<PathBuf> {
        select_available_executable(&self.driver().executable_candidates(settings)?)
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let ready_file = session_root.join("startup.ready");
        let startup = self.driver().startup(drivers::StartupContext {
            cwd,
            session_root,
            ready_file: &ready_file,
        })?;
        let args = startup.args;
        let env = startup.env;
        Ok(ShellStartup {
            args,
            env,
            ready_file,
        })
    }
    pub(crate) fn invocation(
        self,
        command_id: &str,
        directory: &Path,
        cwd: &Path,
    ) -> Result<String> {
        self.driver().invocation(drivers::InvocationContext {
            command_id,
            directory,
            cwd,
        })
    }
    pub(crate) fn keyboard_bytes(self, bytes: &[u8]) -> Cow<'_, [u8]> {
        self.driver().keyboard_bytes(bytes)
    }
    pub(crate) fn parse(value: &str) -> Result<Self> {
        drivers::parse(value).with_context(|| {
            format!(
                "unsupported shell `{value}`; supported shells are {}",
                drivers::supported_shells()
            )
        })
    }
    pub(crate) fn canonical_name(self) -> &'static str {
        self.driver().id()
    }
    pub(crate) fn shim_env_name(self) -> &'static str {
        self.driver().shim_env_name()
    }
    pub(crate) fn executable_aliases(self) -> &'static [&'static str] {
        self.driver().aliases()
    }
    pub(crate) const fn all() -> [Self; 5] {
        [
            Self::PowerShell,
            Self::Bash,
            Self::NuShell,
            Self::Zsh,
            Self::Cmd,
        ]
    }
    pub(crate) fn interactive_arguments(self, arguments: &[std::ffi::OsString]) -> bool {
        self.driver().interactive_arguments(arguments)
    }
    fn driver(self) -> &'static dyn drivers::ShellDriver {
        drivers::driver(self)
    }
}
fn select_available_executable(candidates: &[String]) -> Result<PathBuf> {
    for candidate in candidates {
        if let Ok(path) = resolve_executable(candidate) {
            return Ok(path);
        }
    }
    bail!(
        "none of the configured shell executables are available: {}",
        candidates.join(", ")
    )
}
fn resolve_executable(candidate: &str) -> Result<PathBuf> {
    let paths = which::which_all(candidate)?;
    for path in paths {
        if !is_inherited_shim(&path)? {
            return Ok(path);
        }
    }
    bail!("all executable candidates for `{candidate}` point to FuncTerm shims")
}
fn is_inherited_shim(path: &Path) -> Result<bool> {
    if same_file(
        path,
        &std::env::current_exe().context("failed to resolve current executable")?,
    ) {
        return Ok(true);
    }
    let Some(shim_dir) = std::env::var_os(shims::SHIM_DIR_ENV) else {
        return Ok(false);
    };
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    Ok(same_file(parent, &PathBuf::from(shim_dir)))
}
fn same_file(left: &Path, right: &Path) -> bool {
    let Ok(left_path) = left.canonicalize() else {
        return false;
    };
    let Ok(right_path) = right.canonicalize() else {
        return false;
    };
    left_path == right_path
}
#[cfg(test)]
mod tests {
    use super::ShellChoice;
    #[test]
    fn shell_aliases_parse_to_supported_choices() {
        assert_eq!(
            ShellChoice::parse("powershell.exe").unwrap(),
            ShellChoice::PowerShell
        );
        assert_eq!(ShellChoice::parse("pwsh").unwrap(), ShellChoice::PowerShell);
        assert_eq!(ShellChoice::parse("bash.exe").unwrap(), ShellChoice::Bash);
        assert_eq!(ShellChoice::parse("nu").unwrap(), ShellChoice::NuShell);
        assert_eq!(
            ShellChoice::parse("nushell.exe").unwrap(),
            ShellChoice::NuShell
        );
        assert_eq!(ShellChoice::parse("zsh").unwrap(), ShellChoice::Zsh);
        assert_eq!(ShellChoice::parse("cmd.exe").unwrap(), ShellChoice::Cmd);
    }
}
