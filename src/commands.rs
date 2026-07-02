use crate::runtime::protocol::{Payload, Request, waiting_from_seconds};
use crate::runtime::working_dir;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use std::path::Path;
type DaemonCall<'callback> = dyn Fn(&Request) -> Result<Payload> + 'callback;
pub(crate) fn new_shell(
    call: impl Fn(&Request) -> Result<Payload>,
    cwd: Option<&Path>,
    shell: &str,
) -> Result<String> {
    let shell_choice = ShellChoice::parse(shell)?;
    let resolved_cwd = working_dir::resolve(cwd)?;
    let payload = call(&Request::NewShell {
        cwd: resolved_cwd,
        shell: shell_choice,
    })?;
    text_from_payload(payload, ExpectedPayload::ShellCreated)
}
pub(crate) fn write_keyboard(
    call: impl Fn(&Request) -> Result<Payload>,
    shell_id: String,
    bytes: Vec<u8>,
) -> Result<String> {
    let payload = call(&Request::WriteKeyboard { shell_id, bytes })?;
    text_from_payload(payload, ExpectedPayload::KeyboardWritten)
}
pub(crate) fn send_command(
    call: impl Fn(&Request) -> Result<Payload>,
    shell_id: String,
    command: String,
    waiting_seconds: f64,
) -> Result<String> {
    let waiting = waiting_from_seconds(waiting_seconds)?;
    let payload = call(&Request::SendCommand {
        shell_id,
        command,
        waiting,
    })?;
    text_from_payload(payload, ExpectedPayload::CommandAccepted)
}
pub(crate) fn query(call: impl Fn(&Request) -> Result<Payload>, id: String) -> Result<String> {
    let payload = call(&Request::Query { id })?;
    text_from_payload(payload, ExpectedPayload::Query)
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
    ShellCreated,
    KeyboardWritten,
    CommandAccepted,
    Query,
}
fn text_from_payload(payload: Payload, expected: ExpectedPayload) -> Result<String> {
    let matches_expected = matches!(
        (&payload, expected),
        (Payload::ShellCreated { .. }, ExpectedPayload::ShellCreated)
            | (Payload::KeyboardWritten, ExpectedPayload::KeyboardWritten)
            | (
                Payload::CommandAccepted { .. },
                ExpectedPayload::CommandAccepted
            )
            | (Payload::Query(_), ExpectedPayload::Query)
    );
    if matches_expected {
        return Ok(payload.into_plain_text());
    }
    bail!("daemon returned an unexpected response")
}
