use crate::runtime::protocol::{
    EnvironmentSnapshot, KeyboardInput, Payload, Request, wait_timeout_from_seconds,
};
use crate::runtime::working_dir;
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, ensure};
use core::time::Duration;
use std::path::Path;
type DaemonCall<'callback> = dyn Fn(&Request) -> Result<Payload> + 'callback;
pub(crate) fn close_target(tab_id: Option<String>) -> Result<String> {
    if let Some(id) = tab_id {
        return Ok(id);
    }
    let current = std::env::var(crate::shell::shims::TAB_ID_ENV)
        .context("cannot identify current Tab; run --current inside a FuncTerm Tab")?;
    ensure!(
        !current.is_empty(),
        "cannot identify current Tab; run --current inside a FuncTerm Tab"
    );
    Ok(current)
}
pub(crate) fn close(call: impl Fn(&Request) -> Result<Payload>, tab_id: String) -> Result<String> {
    Ok(call_payload(call, &Request::Close { tab_id })?.into_plain_text())
}
pub(crate) fn new_tab(
    call: impl Fn(&Request) -> Result<Payload>,
    starting_directory: Option<&Path>,
    starting_shell: ShellChoice,
) -> Result<String> {
    Ok(new_tab_payload(call, starting_directory, starting_shell)?.into_plain_text())
}
pub(crate) fn new_tab_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    starting_directory: Option<&Path>,
    starting_shell: ShellChoice,
) -> Result<Payload> {
    let resolved_directory = working_dir::resolve(starting_directory)?;
    let request = Request::NewTab {
        starting_directory: resolved_directory,
        starting_shell,
        environment: EnvironmentSnapshot::for_new_tab_request(),
    };
    call_payload(call, &request)
}
pub(crate) fn manual_write(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    input: KeyboardInput,
    wait_timeout_seconds: f64,
) -> Result<String> {
    Ok(manual_write_payload(call, tab_id, input, wait_timeout_seconds)?.into_plain_text())
}
pub(crate) fn manual_write_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    input: KeyboardInput,
    wait_timeout_seconds: f64,
) -> Result<Payload> {
    call_with_wait_timeout(call, wait_timeout_seconds, |wait_timeout| {
        Request::ManualWrite {
            tab_id,
            input,
            wait_timeout,
        }
    })
}
pub(crate) fn send_command(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    command: String,
    wait_timeout_seconds: f64,
) -> Result<String> {
    Ok(send_command_payload(call, tab_id, command, wait_timeout_seconds)?.into_plain_text())
}
pub(crate) fn send_command_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    command: String,
    wait_timeout_seconds: f64,
) -> Result<Payload> {
    call_with_wait_timeout(call, wait_timeout_seconds, |wait_timeout| {
        Request::SendCommand {
            tab_id,
            command,
            wait_timeout,
        }
    })
}
pub(crate) fn view(
    call: impl Fn(&Request) -> Result<Payload>,
    id: String,
    wait_timeout_seconds: f64,
) -> Result<String> {
    Ok(view_payload(call, id, wait_timeout_seconds)?.into_plain_text())
}
pub(crate) fn view_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    id: String,
    wait_timeout_seconds: f64,
) -> Result<Payload> {
    call_with_wait_timeout(call, wait_timeout_seconds, |wait_timeout| Request::View {
        id,
        wait_timeout,
    })
}
pub(crate) fn with_daemon(
    daemon_service_name: &str,
    operation: impl FnOnce(&DaemonCall<'_>) -> Result<String>,
) -> Result<String> {
    crate::runtime::client::ensure_daemon(daemon_service_name)?;
    operation(&|request| crate::runtime::client::call(daemon_service_name, request))
}
fn call_payload(call: impl Fn(&Request) -> Result<Payload>, request: &Request) -> Result<Payload> {
    call(request)?.ensure_matches(request)
}
fn call_with_wait_timeout(
    call: impl Fn(&Request) -> Result<Payload>,
    wait_timeout_seconds: f64,
    request: impl FnOnce(Duration) -> Request,
) -> Result<Payload> {
    let wait_timeout = wait_timeout_from_seconds(wait_timeout_seconds)?;
    call_payload(call, &request(wait_timeout))
}
