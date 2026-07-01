#[cfg(test)]
use super::codec::PayloadVec;
use super::codec::{Cursor, PayloadSink, PayloadSize, PayloadWriter};
use super::{
    REQUEST_NEW_SHELL, REQUEST_PING, REQUEST_QUERY, REQUEST_SEND_COMMAND, REQUEST_WRITE_KEYBOARD,
    SHELL_BASH, SHELL_NUSHELL, SHELL_POWERSHELL,
};
use crate::runtime::protocol::Request;
use crate::runtime::protocol::wire::RequestHeader;
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, bail};
use core::mem::MaybeUninit;
use core::time::Duration;
use std::path::Path;
#[cfg(test)]
pub(crate) struct RequestFrame {
    pub(crate) header: RequestHeader,
    pub(crate) payload: Vec<u8>,
}
pub(crate) enum BorrowedRequest<'payload> {
    Ping,
    NewShell {
        cwd: &'payload Path,
        shell: ShellChoice,
    },
    WriteKeyboard {
        shell_id: &'payload str,
        bytes: &'payload [u8],
    },
    SendCommand {
        shell_id: &'payload str,
        command: &'payload str,
        waiting: Duration,
    },
    Query {
        id: &'payload str,
    },
}
#[cfg(test)]
impl RequestFrame {
    pub(crate) fn from_request(request: &Request) -> Result<Self> {
        let mut sink = PayloadVec::new();
        let header = encode_request(request, &mut sink)?;
        let payload = sink.into_inner();
        Ok(Self { header, payload })
    }
    pub(crate) fn into_request(self) -> Result<Request> {
        decode_request(self.header, &self.payload).map(BorrowedRequest::into_owned)
    }
}
#[cfg(test)]
impl BorrowedRequest<'_> {
    fn into_owned(self) -> Request {
        match self {
            Self::Ping => Request::Ping,
            Self::NewShell { cwd, shell } => Request::NewShell {
                cwd: cwd.to_path_buf(),
                shell,
            },
            Self::WriteKeyboard { shell_id, bytes } => Request::WriteKeyboard {
                shell_id: shell_id.to_owned(),
                bytes: bytes.to_vec(),
            },
            Self::SendCommand {
                shell_id,
                command,
                waiting,
            } => Request::SendCommand {
                shell_id: shell_id.to_owned(),
                command: command.to_owned(),
                waiting,
            },
            Self::Query { id } => Request::Query { id: id.to_owned() },
        }
    }
}
pub(crate) fn request_header_len(request: &Request) -> Result<(RequestHeader, usize)> {
    let mut sink = PayloadSize::default();
    let header = encode_request(request, &mut sink)?;
    Ok((header, sink.len()))
}
pub(crate) fn write_request_payload(
    request: &Request,
    payload: &mut [MaybeUninit<u8>],
) -> Result<RequestHeader> {
    let mut sink = PayloadWriter::new(payload);
    let header = encode_request(request, &mut sink)?;
    sink.finish()?;
    Ok(header)
}
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching borrowed protocol variants avoids cloning payload fields"
)]
fn encode_request(request: &Request, sink: &mut impl PayloadSink) -> Result<RequestHeader> {
    let mut header = RequestHeader::default();
    match request {
        Request::Ping => header.kind = REQUEST_PING,
        Request::NewShell { cwd, shell } => {
            header.kind = REQUEST_NEW_SHELL;
            header.shell = encode_shell(*shell);
            header.cwd_len = sink.append_path(cwd)?;
        }
        Request::WriteKeyboard { shell_id, bytes } => {
            header.kind = REQUEST_WRITE_KEYBOARD;
            header.shell_id_len = sink.append_text(shell_id)?;
            header.keyboard_len = sink.append_bytes(bytes)?;
        }
        Request::SendCommand {
            shell_id,
            command,
            waiting,
        } => {
            header.kind = REQUEST_SEND_COMMAND;
            header.waiting_ns = encode_waiting(*waiting)?;
            header.shell_id_len = sink.append_text(shell_id)?;
            header.command_len = sink.append_text(command)?;
        }
        Request::Query { id } => {
            header.kind = REQUEST_QUERY;
            header.query_id_len = sink.append_text(id)?;
        }
    }
    Ok(header)
}
pub(crate) fn decode_request(header: RequestHeader, payload: &[u8]) -> Result<BorrowedRequest<'_>> {
    let mut cursor = Cursor::new(payload);
    let request = match header.kind {
        REQUEST_PING => BorrowedRequest::Ping,
        REQUEST_NEW_SHELL => BorrowedRequest::NewShell {
            cwd: cursor.take_path_ref(header.cwd_len)?,
            shell: decode_shell(header.shell)?,
        },
        REQUEST_WRITE_KEYBOARD => BorrowedRequest::WriteKeyboard {
            shell_id: cursor.take_str(header.shell_id_len)?,
            bytes: cursor.take_bytes(header.keyboard_len)?,
        },
        REQUEST_SEND_COMMAND => BorrowedRequest::SendCommand {
            shell_id: cursor.take_str(header.shell_id_len)?,
            command: cursor.take_str(header.command_len)?,
            waiting: Duration::from_nanos(header.waiting_ns),
        },
        REQUEST_QUERY => BorrowedRequest::Query {
            id: cursor.take_str(header.query_id_len)?,
        },
        other => bail!("unknown request kind {other}"),
    };
    cursor.finish()?;
    Ok(request)
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
