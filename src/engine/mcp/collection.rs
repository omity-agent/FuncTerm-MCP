use rmcp::{
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
    entries: Vec<Result<CallToolResult, String>>,
) -> Result<CallToolResult, String> {
    let mut results = Vec::with_capacity(entries.len());
    let mut content = Vec::new();
    let mut failed = false;
    for entry in entries {
        match entry {
            Ok(result) => {
                let value = result
                    .structured_content
                    .ok_or_else(|| "tool result has no structured content".to_owned())?;
                content.extend(result.content);
                results.push(EntryResult::Success { result: value });
            }
            Err(error) => {
                failed = true;
                content.push(ContentBlock::text(error.clone()));
                results.push(EntryResult::Failure { error });
            }
        }
    }
    let structured =
        rmcp::serde_json::to_value(BatchOutput::<Value> { results }).map_err(super::error_text)?;
    let mut combined = CallToolResult::success(content);
    combined.structured_content = Some(structured);
    combined.is_error = Some(failed);
    Ok(combined)
}
