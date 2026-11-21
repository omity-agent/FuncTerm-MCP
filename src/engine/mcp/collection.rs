use super::{internal_error, output::EntryOutput};
use crate::runtime::protocol::RequestRejected;
use rmcp::{
    ErrorData,
    model::{CallToolResult, ContentBlock},
    serde_json::Value,
};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct BatchOutput<T> {
    results: Vec<EntryResult<T>>,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
#[serde(untagged)]
enum EntryResult<T> {
    Success { result: T },
    Failure { error: String },
}
pub(super) fn combine(
    entries: Vec<anyhow::Result<EntryOutput>>,
) -> Result<CallToolResult, ErrorData> {
    let count = entries.len();
    let mut results = Vec::with_capacity(count);
    let mut content = Vec::with_capacity(count);
    let mut failed = false;
    for (index, entry) in entries.into_iter().enumerate() {
        let (mut text, result) = match entry {
            Ok(output) => (
                output.text,
                EntryResult::Success {
                    result: output.value,
                },
            ),
            Err(error) => {
                let RequestRejected(message) = error.downcast().map_err(internal_error)?;
                failed = true;
                (message.clone(), EntryResult::Failure { error: message })
            }
        };
        if index + 1 < count && !text.ends_with('\n') {
            text.push('\n');
        }
        content.push(ContentBlock::text(text));
        results.push(result);
    }
    let structured =
        rmcp::serde_json::to_value(BatchOutput::<Value> { results }).map_err(internal_error)?;
    let mut combined = CallToolResult::success(content);
    combined.structured_content = Some(structured);
    combined.is_error = Some(failed);
    Ok(combined)
}
