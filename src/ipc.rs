use crate::shell::ShellChoice;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Request {
    Ping,
    NewShell {
        cwd: PathBuf,
        shell: ShellChoice,
    },
    WriteKeyboard {
        shell_id: String,
        bytes_base64: String,
    },
    SendCommand {
        shell_id: String,
        command: String,
        wait_ms: u64,
    },
    Query {
        id: String,
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
        shell_id: String,
    },
    KeyboardWritten,
    CommandAccepted {
        command_id: String,
        end_reason: EndReason,
        query: QueryResult,
    },
    Query(QueryResult),
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EndReason {
    CommandEnded,
    WaitTimeout,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "recognized_as", rename_all = "snake_case")]
pub(crate) enum QueryResult {
    Shell {
        cwd: String,
        screen: String,
    },
    Command {
        cwd: String,
        finished: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    },
}
impl Payload {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching borrowed enum variants keeps the formatter concise"
    )]
    pub(crate) fn to_plain_text(&self) -> String {
        match self {
            Self::Pong => "pong".to_owned(),
            Self::ShellCreated { shell_id } => format!("shell_id: {shell_id}"),
            Self::KeyboardWritten => "ok".to_owned(),
            Self::CommandAccepted {
                command_id, query, ..
            } => {
                let mut text = format!("command_id: {command_id}\n");
                text.push_str(&query.to_plain_text());
                text
            }
            Self::Query(query) => query.to_plain_text(),
        }
    }
}
impl QueryResult {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching borrowed enum variants keeps the formatter concise"
    )]
    pub(crate) fn to_plain_text(&self) -> String {
        match self {
            Self::Shell { cwd, screen } => {
                format!("recognized_as: shell\ncwd: {cwd}\nscreen:\n{screen}")
            }
            Self::Command {
                cwd,
                finished,
                stdout,
                stderr,
                exit_code,
            } => {
                let exit_code_text =
                    (*exit_code).map_or_else(|| "pending".to_owned(), |code| code.to_string());
                format!(
                    "recognized_as: command\ncwd: {cwd}\nfinished: {finished}\nexit_code: {exit_code_text}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                )
            }
        }
    }
}
