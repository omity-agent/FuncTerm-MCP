use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::nushell_wrapper;
use anyhow::Result;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    Ok(DriverStartup {
        args: vec![
            "--no-config-file".to_owned(),
            "--no-history".to_owned(),
            "--execute".to_owned(),
            initialization_script(context)?,
        ],
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
        "$env.{CURRENT_SHELL_ENV} = 'nu'\n{}\ncd {}\n'' | save --force --raw {}\n",
        nushell_wrapper(),
        quote::nushell_path(context.cwd)?,
        quote::nushell_path(context.ready_file)?
    ))
}
