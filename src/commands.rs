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
    let shell_choice = ShellChoice::parse(starting_shell)?;
    let resolved_directory = working_dir::resolve(starting_directory)?;
    let payload = call(&Request::NewTab {
        starting_directory: resolved_directory,
        starting_shell: shell_choice,
    })?;
    text_from_payload(payload, ExpectedPayload::TabCreated)
}
pub(crate) fn manual_write(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    bytes: Vec<u8>,
) -> Result<String> {
    let payload = call(&Request::ManualWrite { tab_id, bytes })?;
    text_from_payload(payload, ExpectedPayload::KeyboardWritten)
}
pub(crate) fn send_command(
    call: impl Fn(&Request) -> Result<Payload>,
    tab_id: String,
    command: String,
    waiting_seconds: f64,
) -> Result<String> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let payload = call(&Request::SendCommand {
        tab_id,
        command,
        waiting,
    })?;
    text_from_payload(payload, ExpectedPayload::CommandAccepted)
}
pub(crate) fn view(
    call: impl Fn(&Request) -> Result<Payload>,
    id: String,
    waiting_seconds: f64,
) -> Result<String> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let payload = call(&Request::View { id, waiting })?;
    text_from_payload(payload, ExpectedPayload::View)
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
fn text_from_payload(payload: Payload, expected: ExpectedPayload) -> Result<String> {
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
        return Ok(payload.into_plain_text());
    }
    bail!("daemon returned an unexpected response")
}
