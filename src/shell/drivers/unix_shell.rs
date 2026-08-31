use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{bash_wrapper, zsh_wrapper};
use anyhow::Result;
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    values
        .iter()
        .all(|value| matches!(value.as_str(), "-i" | "-l" | "--login"))
}
pub(super) fn bash_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join("bash_init.sh");
    let script = initialization_script(context, "bash", &bash_wrapper(), ">")?;
    fs_err::write(&init_path, script)?;
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
pub(super) fn zsh_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join(".zshrc");
    let script = initialization_script(context, "zsh", &zsh_wrapper(), ">|")?;
    fs_err::write(&init_path, script)?;
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
    use crate::shell::ShellChoice;
    #[test]
    fn invocation_uses_line_feed() {
        for choice in [ShellChoice::Bash, ShellChoice::Zsh] {
            let bytes = crate::shell::drivers::invocation(choice)
                .unwrap()
                .unwrap()
                .into_bytes();
            assert_eq!(bytes, b"f\n");
        }
    }
}
