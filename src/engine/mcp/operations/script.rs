use crate::mcp::arguments::RunInput;
use anyhow::{Context as _, Result, ensure};
pub(super) struct ScriptBatch {
    pub(super) commands: Vec<RunInput>,
    pub(super) timeouts: Vec<f64>,
}
struct PendingCommand {
    header_line: usize,
    input: RunInput,
}
pub(super) fn parse(script: &str, separate_output: Option<bool>) -> Result<ScriptBatch> {
    let mut commands = Vec::new();
    let mut timeouts = Vec::new();
    let mut pending = None;
    for (index, line) in script.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        if let Some(fields) = metadata(line) {
            finish(pending.take(), &mut commands)?;
            let (tab_id, timeout) = parse_metadata(fields)
                .with_context(|| format!("invalid run metadata on line {line_number}"))?;
            timeouts.extend(timeout);
            pending = Some(PendingCommand {
                header_line: line_number,
                input: RunInput {
                    tab_id: tab_id.to_owned(),
                    command: String::new(),
                    separate_output,
                },
            });
        } else if let Some(block) = pending.as_mut() {
            block.input.command.push_str(line);
        } else {
            ensure!(
                line.trim().is_empty(),
                "expected a `*** tab_id: <id>;` metadata header on line {line_number}"
            );
        }
    }
    finish(pending, &mut commands)?;
    ensure!(
        !commands.is_empty(),
        "script must contain at least one `*** tab_id: <id>;` metadata header"
    );
    Ok(ScriptBatch { commands, timeouts })
}
fn finish(pending: Option<PendingCommand>, commands: &mut Vec<RunInput>) -> Result<()> {
    if let Some(block) = pending {
        ensure!(
            !block.input.command.trim().is_empty(),
            "expected a command after the metadata on line {}",
            block.header_line
        );
        commands.push(block.input);
    }
    Ok(())
}
fn metadata(line: &str) -> Option<&str> {
    metadata_value(line.trim_start().strip_prefix("***")?, "tab_id")
}
fn metadata_value<'field>(field: &'field str, key: &str) -> Option<&'field str> {
    let (name, value) = field.split_once(':')?;
    (name.trim() == key).then(|| value.trim())
}
fn parse_metadata(fields: &str) -> Result<(&str, Option<f64>)> {
    let mut values = fields.split_terminator(';').map(str::trim);
    let tab_id = values.next().context("expected a tab_id value")?;
    ensure!(
        !tab_id.is_empty()
            && !tab_id.contains(|character: char| character.is_whitespace() || character == ':'),
        "expected a tab_id value"
    );
    let timeout = values
        .next()
        .map(|field| {
            metadata_value(field, "wait_timeout")
                .context("expected wait_timeout: <seconds>")?
                .parse::<f64>()
                .context("wait_timeout must be a number in seconds")
        })
        .transpose()?;
    ensure!(
        values.next().is_none(),
        "unexpected field after wait_timeout"
    );
    Ok((tab_id, timeout))
}
