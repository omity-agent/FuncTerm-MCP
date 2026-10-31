use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::contract::NUSHELL_SUBMISSION_FILE;
use crate::shell::ShellChoice;
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{nushell_wrapper, startup_environment};
use anyhow::{Result, ensure};
use std::path::Path;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let bootstrap = context.startup_directory.join("nu_bootstrap.nu");
    fs_err::write(&bootstrap, initialization_script(context)?)?;
    let mut args = vec!["--no-history".to_owned()];
    if !context.load_profile {
        args.push("--no-config-file".to_owned());
    }
    args.extend([
        "--execute".to_owned(),
        format!("source {}", quote::nushell_path(&bootstrap)?),
    ]);
    Ok(DriverStartup {
        args,
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    values.iter().all(|value| {
        matches!(
            value.as_str(),
            "--login" | "--no-config-file" | "--no-history"
        )
    })
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    let style = if context.ignore_shell_style {
        "$env.NO_COLOR = '1'
$env.config.use_ansi_coloring = false
$env.PROMPT_COMMAND = ''
$env.PROMPT_COMMAND_RIGHT = ''
$env.PROMPT_INDICATOR = '> '
$env.PROMPT_INDICATOR_VI_INSERT = '> '
$env.PROMPT_INDICATOR_VI_NORMAL = '> '
$env.PROMPT_MULTILINE_INDICATOR = '> '
$env.TRANSIENT_PROMPT_COMMAND = ''
$env.TRANSIENT_PROMPT_COMMAND_RIGHT = ''
$env.TRANSIENT_PROMPT_MULTILINE_INDICATOR = ''
$env.config.render_right_prompt_on_last_line = false
$env.config.shell_integration = { osc2: false, osc7: false, osc8: false, osc9_9: false, osc133: false, osc633: false, reset_application_mode: false }
"
    } else {
        ""
    };
    let initialization = format!(
        "{}\n$env.{CURRENT_SHELL_ENV} = 'nu'\nlet @VAR_active_command@ = {}\nlet @VAR_helper_path@ = $env.FUNCTERM_HELPER_EXECUTABLE\n{}\nlet @VAR_recovery_environment@ = $env\n{}\ncd {}\nensure_nushell_shims\n$env.config.hooks.pre_execution = ($env.config.hooks.pre_execution | append {{ functerm_begin }})\n$env.config.hooks.pre_prompt = ($env.config.hooks.pre_prompt | append {{ functerm_finish }})\n'' | save --force --raw {}\n",
        startup_environment(crate::shell::ShellChoice::NuShell, context.environment)?,
        quote::nushell_path(&context.ready_file.with_file_name("active-command.nuon"))?,
        style,
        nushell_wrapper(),
        quote::nushell_path(context.cwd)?,
        quote::nushell_path(context.ready_file)?
    );
    Ok(crate::shell::wrappers::VariableNamespace::new().render(&initialization))
}
pub(super) fn invocation(
    directory: &Path,
    separate_output: bool,
) -> Result<super::ShellInvocation> {
    ensure!(
        !separate_output,
        "NuShell does not support separate_output=true; use separate_output=false"
    );
    let submission = directory.join(NUSHELL_SUBMISSION_FILE);
    let submission_path = quote::nushell_path(&submission)?;
    // The execution hook fills this file after the line editor validates the invocation.
    fs_err::write(&submission, b"")?;
    let line = format!("source {submission_path}");
    super::ShellInvocation::new(line, super::invocation_terminator(ShellChoice::NuShell))
}
