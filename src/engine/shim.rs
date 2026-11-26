mod interactive;
mod invocation;
mod process_contract;
mod routing;
mod spawning;
mod stdio;
pub(crate) use self::stdio::terminal_output;
use crate::shell::{ShellChoice, shims};
use anyhow::{Context as _, Result};
pub(crate) use process_contract::process_exit;
use routing::LaunchRoute;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
pub(crate) fn run_if_requested() -> Result<Option<i32>> {
    let Some(choice) = requested_shell() else {
        return Ok(None);
    };
    if !is_shim_invocation()? {
        return Ok(None);
    }
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    match LaunchRoute::detect(choice, &arguments) {
        LaunchRoute::ManagedSession => interactive::run(choice, &arguments).map(Some),
        LaunchRoute::NativeProcess => run_passthrough(choice, &arguments).map(Some),
    }
}
fn requested_shell() -> Option<ShellChoice> {
    let argument = std::env::args_os().next()?;
    let name = Path::new(&argument)
        .file_name()
        .and_then(|name| name.to_str())?;
    ShellChoice::from_shim_name(name)
}
fn is_shim_invocation() -> Result<bool> {
    let Some(shim_dir) = std::env::var_os(shims::SHIM_DIR_ENV) else {
        return Ok(false);
    };
    let invocation = std::env::args_os()
        .next()
        .context("missing executable invocation")?;
    let executable = which::which(invocation).context("failed to resolve shim invocation")?;
    let executable_dir = executable
        .parent()
        .context("shim executable path has no parent")?;
    let directory_matches = executable_dir.canonicalize().with_context(|| {
        format!(
            "failed to resolve shim executable directory {}",
            executable_dir.display()
        )
    })? == PathBuf::from(shim_dir)
        .canonicalize()
        .context("failed to resolve shim directory")?;
    Ok(directory_matches
        && same_file::is_same_file(
            executable,
            std::env::current_exe().context("failed to resolve shim executable")?,
        )?)
}
fn run_passthrough(choice: ShellChoice, arguments: &[std::ffi::OsString]) -> Result<i32> {
    let status = process_contract::command(&real_executable(choice)?, arguments)
        .status()
        .with_context(|| format!("failed to run {}", choice.canonical_name()))?;
    Ok(exit_code(status))
}
fn real_executable(choice: ShellChoice) -> Result<String> {
    std::env::var(choice.shim_env_name()).with_context(|| {
        format!(
            "missing real executable for {} shim",
            choice.canonical_name()
        )
    })
}
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}
