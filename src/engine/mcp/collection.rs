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
                append_content(&mut content, result.content)?;
                results.push(EntryResult::Success { result: value });
            }
            Err(error) => {
                failed = true;
                append_content(&mut content, vec![ContentBlock::text(error.clone())])?;
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
fn append_content(
    content: &mut Vec<ContentBlock>,
    mut entry: Vec<ContentBlock>,
) -> Result<(), String> {
    if !content.is_empty() {
        let previous = content
            .pop()
            .ok_or_else(|| "MCP output content unexpectedly became empty".to_owned())?;
        let separated = match previous {
            ContentBlock::Text(mut text) => {
                if !text.text.ends_with('\n') {
                    text.text.push('\n');
                }
                ContentBlock::Text(text)
            }
            ContentBlock::Image(_)
            | ContentBlock::Audio(_)
            | ContentBlock::Resource(_)
            | ContentBlock::ResourceLink(_)
            | _ => return Err("MCP output contains a non-text content block".to_owned()),
        };
        content.push(separated);
    }
    content.append(&mut entry);
    Ok(())
}
