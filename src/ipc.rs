use crate::shell::ShellChoice;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Request {
    Ping,
    NewShell {
        cwd: PathBuf,
        shell: ShellChoice,
    },
    WriteKeyboard {
        shell_id: Uuid,
        bytes_base64: String,
    },
    SendCommand {
        shell_id: Uuid,
        command: String,
        wait_ms: u64,
    },
    Query {
        id: Uuid,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Response {
    Ok { payload: Payload },
    Err { message: String },
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Payload {
    Pong,
    ShellCreated {
        shell_id: Uuid,
    },
    KeyboardWritten,
    CommandAccepted {
        command_id: Uuid,
        end_reason: EndReason,
    },
    Query(QueryResult),
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EndReason {
    CommandEnded,
    WaitTimeout,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "recognized_as", rename_all = "snake_case")]
pub(crate) enum QueryResult {
    Shell {
        screen: String,
    },
    Command {
        finished: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    },
}
