use crate::runtime::protocol::{Payload, Request, waiting_from_seconds};
use crate::runtime::working_dir;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use std::path::Path;
type DaemonCall<'callback> = dyn Fn(&Request) -> Result<Payload> + 'callback;
pub(crate) fn new_tab(
    call: impl Fn(&Request) -> Result<Payload>,
    starting_directory: Option<&Path>,
    starting_shell: &str,
) -> Result<String> {
    Ok(new_tab_payload(call, starting_directory, starting_shell)?.into_plain_text())
}
pub(crate) fn new_tab_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    starting_directory: Option<&Path>,
    starting_shell: &str,
) -> Result<Payload> {
    let shell_choice = ShellChoice::parse(starting_shell)?;
    let resolved_directory = working_dir::resolve(starting_directory)?;
    let payload = call(&Request::NewTab {
        starting_directory: resolved_directory,
        starting_shell: shell_choice,
    })?;
    expected_payload(payload, ExpectedPayload::TabCreated)
}
pub(crate) fn manual_write(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    bytes: Vec<u8>,
) -> Result<String> {
    Ok(manual_write_payload(call, tab_id, bytes)?.into_plain_text())
}
pub(crate) fn manual_write_payload(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    bytes: Vec<u8>,
) -> Result<Payload> {
    let payload = call(&Request::ManualWrite { tab_id, bytes })?;
    expected_payload(payload, ExpectedPayload::KeyboardWritten)
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
    let payload = call(&Request::SendCommand {
        tab_id,
        command,
        waiting,
    })?;
    expected_payload(payload, ExpectedPayload::CommandAccepted)
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
    let payload = call(&Request::View { id, waiting })?;
    expected_payload(payload, ExpectedPayload::View)
}
pub(crate) fn with_daemon(
    daemon_service_name: &str,
    operation: impl FnOnce(&DaemonCall<'_>) -> Result<String>,
) -> Result<String> {
    crate::runtime::client::ensure_daemon(daemon_service_name)?;
    operation(&|request| crate::runtime::client::call(daemon_service_name, request))
}
#[derive(Clone, Copy)]
enum ExpectedPayload {
    TabCreated,
    KeyboardWritten,
    CommandAccepted,
    View,
}
fn expected_payload(payload: Payload, expected: ExpectedPayload) -> Result<Payload> {
    let matches_expected = matches!(
        (&payload, expected),
        (Payload::TabCreated { .. }, ExpectedPayload::TabCreated)
            | (Payload::KeyboardWritten, ExpectedPayload::KeyboardWritten)
            | (
                Payload::CommandAccepted { .. },
                ExpectedPayload::CommandAccepted
            )
            | (Payload::View(_), ExpectedPayload::View)
    );
    if matches_expected {
        return Ok(payload);
    }
    bail!("daemon returned an unexpected response")
}
