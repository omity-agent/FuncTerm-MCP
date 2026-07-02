use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Request {
    Ping,
    NewShell {
        cwd: PathBuf,
        shell: ShellChoice,
    },
    WriteKeyboard {
        shell_id: String,
        bytes: Vec<u8>,
    },
    SendCommand {
        shell_id: String,
        command: String,
        waiting: Duration,
    },
    Query {
        id: String,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Response {
    Ok { payload: Payload },
    Err { message: String },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum EndReason {
    CommandEnded,
    WaitTimeout,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum QueryResult {
    Shell {
        alive: bool,
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
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Pong => "pong".to_owned(),
            Self::ShellCreated { shell_id } => format!("shell_id: {shell_id}"),
            Self::KeyboardWritten => "ok".to_owned(),
            Self::CommandAccepted {
                command_id, query, ..
            } => {
                let mut text = format!("command_id: {command_id}\n");
                text.push_str(&query.into_plain_text());
                text
            }
            Self::Query(query) => query.into_plain_text(),
        }
    }
}
impl QueryResult {
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Shell { alive, cwd, screen } => {
                format!("recognized_as: shell\nalive: {alive}\ncwd: {cwd}\nscreen:\n{screen}")
            }
            Self::Command {
                cwd,
                finished,
                stdout,
                stderr,
                exit_code,
            } => {
                let exit_code_text =
                    exit_code.map_or_else(|| "pending".to_owned(), |code| code.to_string());
                format!(
                    "recognized_as: command\ncwd: {cwd}\nfinished: {finished}\nexit_code: {exit_code_text}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                )
            }
        }
    }
}
pub(crate) fn waiting_from_seconds(seconds: f64) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds)
        .context("waiting must be a finite non-negative number of seconds")
}
