use crate::ipc::{EndReason, QueryResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewShellRequest {
    pub(super) cwd: String,
    pub(super) shell: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewShellResponse {
    shell_id: String,
}
impl NewShellResponse {
    pub(super) fn new(shell_id: Uuid) -> Self {
        Self {
            shell_id: shell_id.to_string(),
        }
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct WriteKeyboardRequest {
    pub(super) shell_id: String,
    pub(super) bytes: Vec<u8>,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct WriteKeyboardResponse {
    ok: bool,
}
impl WriteKeyboardResponse {
    pub(super) const fn ok() -> Self {
        Self { ok: true }
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandRequest {
    pub(super) shell_id: String,
    pub(super) command: String,
    pub(super) wait_ms: u64,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandResponse {
    command_id: String,
    end_reason: CommandEndReason,
}
impl SendCommandResponse {
    pub(super) fn new(command_id: Uuid, end_reason: EndReason) -> Self {
        Self {
            command_id: command_id.to_string(),
            end_reason: end_reason.into(),
        }
    }
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CommandEndReason {
    CommandEnded,
    WaitTimeout,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct QueryRequest {
    pub(super) id: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct QueryResponse {
    recognized_as: QueryResponseKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    screen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
#[serde(rename_all = "snake_case")]
enum QueryResponseKind {
    Shell,
    Command,
}
impl From<EndReason> for CommandEndReason {
    fn from(value: EndReason) -> Self {
        match value {
            EndReason::CommandEnded => Self::CommandEnded,
            EndReason::WaitTimeout => Self::WaitTimeout,
        }
    }
}
impl From<QueryResult> for QueryResponse {
    fn from(value: QueryResult) -> Self {
        match value {
            QueryResult::Shell { screen } => Self {
                recognized_as: QueryResponseKind::Shell,
                screen: Some(screen),
                finished: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            },
            QueryResult::Command {
                finished,
                stdout,
                stderr,
                exit_code,
            } => Self {
                recognized_as: QueryResponseKind::Command,
                screen: None,
                finished: Some(finished),
                stdout: Some(stdout),
                stderr: Some(stderr),
                exit_code,
            },
        }
    }
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    #[test]
    fn query_response_schema_has_object_root() {
        let schema = rmcp::schemars::schema_for!(super::QueryResponse);
        let text = sonic_rs::to_string(&schema).unwrap();
        assert!(text.contains(r#""type":"object""#));
    }
}
