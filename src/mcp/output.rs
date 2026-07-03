use crate::runtime::protocol::{Payload, ViewResult};
use alloc::sync::Arc;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabOutput {
    pub(super) tab_id: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteOutput {
    pub(super) ok: bool,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandOutput {
    pub(super) command_id: String,
    pub(super) view: ViewData,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ViewOutput {
    pub(super) view: ViewData,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ViewData {
    Tab {
        alive: bool,
        cwd: String,
        screen: String,
        last_command: Option<String>,
    },
    Command {
        cwd: String,
        finished: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    },
}
pub(super) fn schema<T>() -> Arc<JsonObject>
where
    T: rmcp::schemars::JsonSchema + 'static,
{
    rmcp::handler::server::tool::schema_for_output::<T>()
        .unwrap_or_else(|error| panic!("invalid MCP output schema: {error}"))
}
pub(super) fn new_tab(payload: Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    let Payload::TabCreated { tab_id } = payload else {
        return Err(unexpected_response());
    };
    result(text, NewTabOutput { tab_id })
}
pub(super) fn manual_write(payload: &Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    match payload {
        &Payload::KeyboardWritten => result(text, ManualWriteOutput { ok: true }),
        &Payload::Pong
        | &Payload::TabCreated { .. }
        | &Payload::CommandAccepted { .. }
        | &Payload::View(_) => Err(unexpected_response()),
    }
}
pub(super) fn send_command(payload: Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    let Payload::CommandAccepted {
        command_id, view, ..
    } = payload
    else {
        return Err(unexpected_response());
    };
    result(
        text,
        SendCommandOutput {
            command_id,
            view: view.into(),
        },
    )
}
pub(super) fn view(payload: Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    let Payload::View(view) = payload else {
        return Err(unexpected_response());
    };
    result(text, ViewOutput { view: view.into() })
}
fn result<T>(content: String, structured_content: T) -> Result<CallToolResult, String>
where
    T: Serialize,
{
    let value = rmcp::serde_json::to_value(structured_content)
        .map_err(|error| format!("failed to serialize structured content: {error}"))?;
    let mut result = CallToolResult::success(vec![ContentBlock::text(content)]);
    result.structured_content = Some(value);
    Ok(result)
}
fn unexpected_response() -> String {
    "daemon returned an unexpected response".to_owned()
}
impl From<ViewResult> for ViewData {
    fn from(value: ViewResult) -> Self {
        match value {
            ViewResult::Tab {
                alive,
                cwd,
                screen,
                last_command,
            } => Self::Tab {
                alive,
                cwd,
                screen,
                last_command,
            },
            ViewResult::Command {
                cwd,
                finished,
                stdout,
                stderr,
                exit_code,
            } => Self::Command {
                cwd,
                finished,
                stdout,
                stderr,
                exit_code,
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::protocol::EndReason;
    #[test]
    fn command_result_contains_pseudo_xml_content_and_structured_content() {
        let result = match send_command(Payload::CommandAccepted {
            command_id: "cmd".to_owned(),
            end_reason: EndReason::CommandEnded,
            view: ViewResult::Command {
                cwd: "F:\\workspace".to_owned(),
                finished: true,
                stdout: "ok".to_owned(),
                stderr: String::new(),
                exit_code: Some(0_i32),
            },
        }) {
            Ok(result) => result,
            Err(error) => panic!("payload should be converted: {error}"),
        };
        let text = result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .unwrap_or_else(|| panic!("content should contain text"));
        assert!(text.text.contains("<COMMAND_ID>\ncmd\n</COMMAND_ID>"));
        let structured = result
            .structured_content
            .unwrap_or_else(|| panic!("structuredContent should be present"));
        assert_eq!(
            structured.get("command_id"),
            Some(&rmcp::serde_json::json!("cmd"))
        );
        assert_eq!(structured.get("end_reason"), None);
    }
}
