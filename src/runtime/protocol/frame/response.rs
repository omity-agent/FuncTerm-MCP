use super::codec::{Cursor, append_text, decode_flag};
use super::{
    END_COMMAND_ENDED, END_WAIT_TIMEOUT, PAYLOAD_COMMAND_ACCEPTED, PAYLOAD_KEYBOARD_WRITTEN,
    PAYLOAD_PONG, PAYLOAD_QUERY, PAYLOAD_SHELL_CREATED, QUERY_COMMAND, QUERY_SHELL, RESPONSE_ERR,
    RESPONSE_OK,
};
use crate::runtime::protocol::wire::ResponseHeader;
use crate::runtime::protocol::{EndReason, Payload, QueryResult, Response};
use anyhow::{Result, bail};
pub(crate) struct ResponseFrame {
    pub(crate) header: ResponseHeader,
    pub(crate) payload: Vec<u8>,
}
impl ResponseFrame {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching borrowed protocol variants avoids cloning payload fields"
    )]
    pub(crate) fn from_response(response: &Response) -> Result<Self> {
        let mut header = ResponseHeader::default();
        let mut payload = Vec::new();
        match response {
            Response::Ok { payload: body } => {
                header.status = RESPONSE_OK;
                encode_payload(&mut header, &mut payload, body)?;
            }
            Response::Err { message } => {
                header.status = RESPONSE_ERR;
                header.message_len = append_text(&mut payload, message)?;
            }
        }
        Ok(Self { header, payload })
    }
    pub(crate) fn into_response(self) -> Result<Response> {
        let mut cursor = Cursor::new(&self.payload);
        let response = match self.header.status {
            RESPONSE_OK => Response::Ok {
                payload: decode_payload(&self.header, &mut cursor)?,
            },
            RESPONSE_ERR => Response::Err {
                message: cursor.take_text(self.header.message_len)?,
            },
            other => bail!("unknown response status {other}"),
        };
        cursor.finish()?;
        Ok(response)
    }
}
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching borrowed protocol variants avoids cloning payload fields"
)]
fn encode_payload(
    header: &mut ResponseHeader,
    payload: &mut Vec<u8>,
    body: &Payload,
) -> Result<()> {
    match body {
        Payload::Pong => header.payload_kind = PAYLOAD_PONG,
        Payload::ShellCreated { shell_id } => {
            header.payload_kind = PAYLOAD_SHELL_CREATED;
            header.shell_id_len = append_text(payload, shell_id)?;
        }
        Payload::KeyboardWritten => header.payload_kind = PAYLOAD_KEYBOARD_WRITTEN,
        Payload::CommandAccepted {
            command_id,
            end_reason,
            query,
        } => {
            header.payload_kind = PAYLOAD_COMMAND_ACCEPTED;
            header.end_reason = encode_end_reason(*end_reason);
            header.command_id_len = append_text(payload, command_id)?;
            encode_query(header, payload, query)?;
        }
        Payload::Query(query) => {
            header.payload_kind = PAYLOAD_QUERY;
            encode_query(header, payload, query)?;
        }
    }
    Ok(())
}
fn decode_payload(header: &ResponseHeader, cursor: &mut Cursor<'_>) -> Result<Payload> {
    match header.payload_kind {
        PAYLOAD_PONG => Ok(Payload::Pong),
        PAYLOAD_SHELL_CREATED => Ok(Payload::ShellCreated {
            shell_id: cursor.take_text(header.shell_id_len)?,
        }),
        PAYLOAD_KEYBOARD_WRITTEN => Ok(Payload::KeyboardWritten),
        PAYLOAD_COMMAND_ACCEPTED => Ok(Payload::CommandAccepted {
            command_id: cursor.take_text(header.command_id_len)?,
            end_reason: decode_end_reason(header.end_reason)?,
            query: decode_query(header, cursor)?,
        }),
        PAYLOAD_QUERY => Ok(Payload::Query(decode_query(header, cursor)?)),
        other => bail!("unknown payload kind {other}"),
    }
}
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching borrowed protocol variants avoids cloning payload fields"
)]
fn encode_query(
    header: &mut ResponseHeader,
    payload: &mut Vec<u8>,
    query: &QueryResult,
) -> Result<()> {
    match query {
        QueryResult::Shell { alive, cwd, screen } => {
            header.query_kind = QUERY_SHELL;
            header.alive = u8::from(*alive);
            header.cwd_len = append_text(payload, cwd)?;
            header.screen_len = append_text(payload, screen)?;
        }
        QueryResult::Command {
            cwd,
            finished,
            stdout,
            stderr,
            exit_code,
        } => {
            header.query_kind = QUERY_COMMAND;
            header.finished = u8::from(*finished);
            if let Some(code) = *exit_code {
                header.has_exit_code = 1;
                header.exit_code = code;
            }
            header.cwd_len = append_text(payload, cwd)?;
            header.stdout_len = append_text(payload, stdout)?;
            header.stderr_len = append_text(payload, stderr)?;
        }
    }
    Ok(())
}
fn decode_query(header: &ResponseHeader, cursor: &mut Cursor<'_>) -> Result<QueryResult> {
    match header.query_kind {
        QUERY_SHELL => Ok(QueryResult::Shell {
            alive: decode_flag(header.alive, "alive")?,
            cwd: cursor.take_text(header.cwd_len)?,
            screen: cursor.take_text(header.screen_len)?,
        }),
        QUERY_COMMAND => Ok(QueryResult::Command {
            cwd: cursor.take_text(header.cwd_len)?,
            finished: decode_flag(header.finished, "finished")?,
            stdout: cursor.take_text(header.stdout_len)?,
            stderr: cursor.take_text(header.stderr_len)?,
            exit_code: (header.has_exit_code != 0).then_some(header.exit_code),
        }),
        other => bail!("unknown query kind {other}"),
    }
}
const fn encode_end_reason(end_reason: EndReason) -> u8 {
    match end_reason {
        EndReason::CommandEnded => END_COMMAND_ENDED,
        EndReason::WaitTimeout => END_WAIT_TIMEOUT,
    }
}
fn decode_end_reason(value: u8) -> Result<EndReason> {
    match value {
        END_COMMAND_ENDED => Ok(EndReason::CommandEnded),
        END_WAIT_TIMEOUT => Ok(EndReason::WaitTimeout),
        other => bail!("unknown end reason {other}"),
    }
}
