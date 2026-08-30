use super::{DriverStartup, InvocationTerminator, ShellDriver, StartupContext, os_strings_lower};
use crate::runtime::config::Settings;
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{bash_wrapper, zsh_wrapper};
use anyhow::{Context as _, Result};
#[derive(Clone, Copy)]
enum PosixKind {
    Bash,
    Zsh,
}
pub(crate) struct PosixDriver {
    kind: PosixKind,
}
impl PosixDriver {
    pub(crate) const fn bash() -> Self {
        Self {
            kind: PosixKind::Bash,
        }
    }
    pub(crate) const fn zsh() -> Self {
        Self {
            kind: PosixKind::Zsh,
        }
    }
}
impl ShellDriver for PosixDriver {
    fn display_name(&self) -> &'static str {
        match self.kind {
            PosixKind::Bash => "Bash",
            PosixKind::Zsh => "Zsh",
        }
    }
    fn shim_executable_names(&self) -> &'static [&'static str] {
        match self.kind {
            PosixKind::Bash => &["bash", "bash.exe"],
            PosixKind::Zsh => &["zsh"],
        }
    }
    fn shim_env_name(&self) -> &'static str {
        match self.kind {
            PosixKind::Bash => "FUNCTERM_REAL_BASH",
            PosixKind::Zsh => "FUNCTERM_REAL_ZSH",
        }
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(match self.kind {
            PosixKind::Bash => vec![settings.bash.clone()],
            PosixKind::Zsh => vec![settings.zsh.clone()],
        })
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
        match self.kind {
            PosixKind::Bash => bash_startup(context),
            PosixKind::Zsh => zsh_startup(context),
        }
    }
    fn invocation_terminator(&self) -> InvocationTerminator {
        InvocationTerminator::LineFeed
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        let Some(values) = os_strings_lower(arguments) else {
            return false;
        };
        values
            .iter()
            .all(|value| matches!(value.as_str(), "-i" | "-l" | "--login"))
    }
}
fn bash_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join("bash_init.sh");
    let script = initialization_script(context, "bash", &bash_wrapper(), ">")?;
    std::fs::write(&init_path, script).context("failed to write Bash initialization script")?;
    Ok(DriverStartup {
        args: vec![
            "--noprofile".to_owned(),
            "--rcfile".to_owned(),
            quote::native_path(&init_path)?,
            "-i".to_owned(),
        ],
        env: Vec::new(),
    })
}
fn zsh_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join(".zshrc");
    let script = initialization_script(context, "zsh", &zsh_wrapper(), ">|")?;
    std::fs::write(&init_path, script).context("failed to write Zsh initialization script")?;
    Ok(DriverStartup {
        args: vec!["-i".to_owned()],
        env: vec![(
            "ZDOTDIR".to_owned(),
            quote::native_path(context.startup_directory)?,
        )],
    })
}
fn initialization_script(
    context: StartupContext<'_>,
    shell: &str,
    wrapper: &str,
    overwrite: &str,
) -> Result<String> {
    Ok(format!(
        "export {CURRENT_SHELL_ENV}={shell}\n{wrapper}\nfuncterm_cwd=$(functerm_posix_path {}) || exit 1\nfuncterm_ready_file=$(functerm_posix_path {}) || exit 1\ncd \"$functerm_cwd\"\n: {overwrite} \"$functerm_ready_file\"\n",
        quote::posix_string(&quote::native_path(context.cwd)?),
        quote::posix_string(&quote::native_path(context.ready_file)?)
    ))
}
#[cfg(test)]
mod tests {
    use super::PosixDriver;
    use crate::shell::drivers::ShellDriver as _;
    #[test]
    fn invocation_uses_line_feed() {
        for driver in [PosixDriver::bash(), PosixDriver::zsh()] {
            let bytes = driver.invocation().unwrap().unwrap().into_bytes();
            assert_eq!(bytes, b"f\n");
        }
    }
}
