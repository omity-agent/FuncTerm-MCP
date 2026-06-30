use serde::Deserialize;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewShellRequest {
    pub(super) cwd: Option<String>,
    pub(super) shell: String,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct WriteKeyboardRequest {
    pub(super) shell_id: String,
    pub(super) bytes: Vec<u8>,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandRequest {
    pub(super) shell_id: String,
    pub(super) command: String,
    pub(super) wait_ms: u64,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct QueryRequest {
    pub(super) id: String,
}
