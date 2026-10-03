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
    Ok(format!(
        "{}\n$env.{CURRENT_SHELL_ENV} = 'nu'\n{}\ncd {}\nensure_nushell_shims\nsave_nushell_state {} {} {} {}\n'' | save --force --raw {}\n",
        startup_environment(crate::shell::ShellChoice::NuShell, context.environment)?,
        nushell_wrapper(),
        quote::nushell_path(context.cwd)?,
        quote::nushell_path(&context.startup_directory.join("initial-directory.txt"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-env.nuon"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-config.nuon"))?,
        quote::nushell_path(&context.ready_file.with_file_name("nushell-declarations.nu"))?,
        quote::nushell_path(context.ready_file)?
    ))
}
