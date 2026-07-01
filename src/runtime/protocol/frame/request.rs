use super::codec::{Cursor, append_bytes, append_path, append_text};
use super::{
    REQUEST_NEW_SHELL, REQUEST_PING, REQUEST_QUERY, REQUEST_SEND_COMMAND, REQUEST_WRITE_KEYBOARD,
    SHELL_BASH, SHELL_NUSHELL, SHELL_POWERSHELL,
};
use crate::runtime::protocol::Request;
use crate::runtime::protocol::wire::RequestHeader;
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
pub(crate) struct RequestFrame {
    pub(crate) header: RequestHeader,
    pub(crate) payload: Vec<u8>,
}
impl RequestFrame {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching borrowed protocol variants avoids cloning payload fields"
    )]
    pub(crate) fn from_request(request: &Request) -> Result<Self> {
        let mut header = RequestHeader::default();
        let mut payload = Vec::new();
        match request {
            Request::Ping => header.kind = REQUEST_PING,
            Request::NewShell { cwd, shell } => {
                header.kind = REQUEST_NEW_SHELL;
                header.shell = encode_shell(*shell);
                header.cwd_len = append_path(&mut payload, cwd)?;
            }
            Request::WriteKeyboard { shell_id, bytes } => {
                header.kind = REQUEST_WRITE_KEYBOARD;
                header.shell_id_len = append_text(&mut payload, shell_id)?;
                header.keyboard_len = append_bytes(&mut payload, bytes)?;
            }
            Request::SendCommand {
                shell_id,
                command,
                waiting,
            } => {
                header.kind = REQUEST_SEND_COMMAND;
                header.waiting_ns = encode_waiting(*waiting)?;
                header.shell_id_len = append_text(&mut payload, shell_id)?;
                header.command_len = append_text(&mut payload, command)?;
            }
            Request::Query { id } => {
                header.kind = REQUEST_QUERY;
                header.query_id_len = append_text(&mut payload, id)?;
            }
        }
        Ok(Self { header, payload })
    }
    pub(crate) fn into_request(self) -> Result<Request> {
        let mut cursor = Cursor::new(&self.payload);
        let request = match self.header.kind {
            REQUEST_PING => Request::Ping,
            REQUEST_NEW_SHELL => Request::NewShell {
                cwd: cursor.take_path(self.header.cwd_len)?,
                shell: decode_shell(self.header.shell)?,
            },
            REQUEST_WRITE_KEYBOARD => Request::WriteKeyboard {
                shell_id: cursor.take_text(self.header.shell_id_len)?,
                bytes: cursor.take_bytes(self.header.keyboard_len)?.to_vec(),
            },
            REQUEST_SEND_COMMAND => Request::SendCommand {
                shell_id: cursor.take_text(self.header.shell_id_len)?,
                command: cursor.take_text(self.header.command_len)?,
                waiting: Duration::from_nanos(self.header.waiting_ns),
            },
            REQUEST_QUERY => Request::Query {
                id: cursor.take_text(self.header.query_id_len)?,
            },
            other => bail!("unknown request kind {other}"),
        };
        cursor.finish()?;
        Ok(request)
    }
}
const fn encode_shell(shell: ShellChoice) -> u8 {
    match shell {
        ShellChoice::PowerShell => SHELL_POWERSHELL,
        ShellChoice::Bash => SHELL_BASH,
        ShellChoice::NuShell => SHELL_NUSHELL,
    }
}
fn decode_shell(value: u8) -> Result<ShellChoice> {
    match value {
        SHELL_POWERSHELL => Ok(ShellChoice::PowerShell),
        SHELL_BASH => Ok(ShellChoice::Bash),
        SHELL_NUSHELL => Ok(ShellChoice::NuShell),
        other => bail!("unknown shell kind {other}"),
    }
}
fn encode_waiting(waiting: Duration) -> Result<u64> {
    u64::try_from(waiting.as_nanos()).context("waiting is too large for an IPC request")
}
