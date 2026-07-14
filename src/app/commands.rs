use crate::runtime::protocol::{
    EnvironmentSnapshot, KeyboardInput, Payload, Request, waiting_from_seconds,
};
use crate::runtime::working_dir;
use crate::shell::ShellChoice;
use anyhow::Result;
use std::path::Path;
type DaemonCall<'callback> = dyn Fn(&Request) -> Result<Payload> + 'callback;
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
    waiting_seconds: f64,
) -> Result<String> {
    Ok(manual_write_payload(call, tab_id, input, waiting_seconds)?.into_plain_text())
}
pub(crate) fn manual_write_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    input: KeyboardInput,
    waiting_seconds: f64,
) -> Result<Payload> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let request = Request::ManualWrite {
        tab_id,
        input,
        waiting,
    };
    call_payload(call, &request)
}
pub(crate) fn send_command(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    command: String,
    waiting_seconds: f64,
) -> Result<String> {
    Ok(send_command_payload(call, tab_id, command, waiting_seconds)?.into_plain_text())
}
pub(crate) fn send_command_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    command: String,
    waiting_seconds: f64,
) -> Result<Payload> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let request = Request::SendCommand {
        tab_id,
        command,
        waiting,
    };
    call_payload(call, &request)
}
pub(crate) fn view(
    call: impl Fn(&Request) -> Result<Payload>,
    id: String,
    waiting_seconds: f64,
) -> Result<String> {
    Ok(view_payload(call, id, waiting_seconds)?.into_plain_text())
}
pub(crate) fn view_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    id: String,
    waiting_seconds: f64,
) -> Result<Payload> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let request = Request::View { id, waiting };
    call_payload(call, &request)
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
