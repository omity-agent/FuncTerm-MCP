use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Request {
    Ping,
    NewTab {
        starting_directory: PathBuf,
        starting_shell: ShellChoice,
    },
    ManualWrite {
        tab_id: String,
        bytes: Vec<u8>,
    },
    SendCommand {
        tab_id: String,
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
    TabCreated {
        tab_id: String,
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
    Tab {
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
            Self::Pong => element("PONG", ""),
            Self::TabCreated { tab_id } => element("TAB_ID", &tab_id),
            Self::KeyboardWritten => element("OK", ""),
            Self::CommandAccepted {
                command_id, query, ..
            } => {
                let mut text = element("COMMAND_ID", &command_id);
                text.push('\n');
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
            Self::Tab { alive, cwd, screen } => elements([
                ("ALIVE", alive.to_string()),
                ("CWD", cwd),
                ("SCREEN", screen),
            ]),
            Self::Command {
                cwd,
                finished,
                stdout,
                stderr,
                exit_code,
            } => {
                let exit_code_text =
                    exit_code.map_or_else(|| "pending".to_owned(), |code| code.to_string());
                elements([
                    ("CWD", cwd),
                    ("FINISHED", finished.to_string()),
                    ("EXIT_CODE", exit_code_text),
                    ("STDOUT", stdout),
                    ("STDERR", stderr),
                ])
            }
        }
    }
}
fn elements<const COUNT: usize>(items: [(&str, String); COUNT]) -> String {
    let mut text = String::new();
    for (index, (tag, content)) in items.into_iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&element(tag, &content));
    }
    text
}
fn element(tag: &str, content: &str) -> String {
    format!("<{tag}>\n{content}\n</{tag}>")
}
pub(crate) fn waiting_from_seconds(seconds: f64) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds)
        .context("waiting must be a finite non-negative number of seconds")
}
#[cfg(test)]
mod tests {
    use super::QueryResult;
    #[test]
    fn command_output_uses_uppercase_tags_without_escaping_content() {
        let text = QueryResult::Command {
            cwd: "F:\\workspace\\A&B".to_owned(),
            finished: true,
            stdout: "left < right".to_owned(),
            stderr: "raw </STDERR> allowed".to_owned(),
            exit_code: Some(0_i32),
        }
        .into_plain_text();
        assert!(text.contains("<CWD>\nF:\\workspace\\A&B\n</CWD>"));
        assert!(text.contains("<STDOUT>\nleft < right\n</STDOUT>"));
        assert!(text.contains("<STDERR>\nraw </STDERR> allowed\n</STDERR>"));
    }
}
