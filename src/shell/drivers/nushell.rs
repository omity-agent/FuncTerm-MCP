use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{nushell_wrapper, startup_environment};
use anyhow::Result;
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
    Ok(format!(
        "{}\n$env.{CURRENT_SHELL_ENV} = 'nu'\n{}\n{}\ncd {}\nensure_nushell_shims\nsave_nushell_state {} {} {} {}\n'' | save --force --raw {}\n",
        startup_environment(crate::shell::ShellChoice::NuShell, context.environment)?,
        nushell_wrapper(),
        style,
        quote::nushell_path(context.cwd)?,
        quote::nushell_path(&context.startup_directory.join("initial-directory.txt"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-env.nuon"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-config.nuon"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-declarations.nu"))?,
        quote::nushell_path(context.ready_file)?
    ))
}
