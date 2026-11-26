use crate::contract::{CommandReport, RELEASED_FILE, STDERR_FILE, STDOUT_FILE};
use crate::runtime::protocol::{CommandOutput, CommandSnapshot, CommandView};
use crate::shell::ShellChoice;
use anyhow::Result;
use core::time::Duration;
use fs_err as fs;
use std::path::{Path, PathBuf};
pub(super) struct CommandRecord {
    pub(super) directory: PathBuf,
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
    pub(super) source: PathBuf,
    pub(super) released: PathBuf,
}
pub(super) fn create_record(
    command_root: &Path,
    command_id: &str,
    choice: ShellChoice,
) -> Result<CommandRecord> {
    let command_dir = command_root.join(command_id);
    fs::create_dir(&command_dir)?;
    Ok(CommandRecord {
        stdout: command_dir.join(STDOUT_FILE),
        stderr: command_dir.join(STDERR_FILE),
        source: command_dir.join(choice.source_file()),
        released: command_dir.join(RELEASED_FILE),
        directory: command_dir,
    })
}
pub(super) fn command_snapshot(
    observed_time_consumption: Duration,
    title: String,
    output: CommandOutput,
    total_output_tokens: usize,
    report: Option<&CommandReport<String>>,
) -> CommandSnapshot {
    let exit_code = report.map(|result| result.outcome.exit_code());
    let time_consumption = report.map_or(observed_time_consumption, |result| {
        result.elapsed.unwrap_or(observed_time_consumption)
    });
    let note = command_note(&output, "");
    CommandSnapshot {
        title,
        command: CommandView {
            total_output_tokens,
            output,
            exit_code,
            time_consumption,
            finished: report.is_some(),
        },
        note,
    }
}
pub(super) fn remove_record_directory(record: &CommandRecord) -> Result<()> {
    match fs::remove_dir_all(&record.directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn discard_record(record: &CommandRecord) {
    if let Err(error) = remove_record_directory(record) {
        tracing::error!("failed to remove abandoned command record");
        tracing::debug!(error = %error, "abandoned command record cleanup failed");
    }
}
pub(super) fn command_note(output: &CommandOutput, extra: &str) -> String {
    let mut lines = Vec::new();
    if !extra.is_empty() {
        lines.push(extra.to_owned());
    }
    if output.is_empty() {
        lines.push(
            match *output {
                CommandOutput::Separated {
                    stdout: _,
                    stderr: _,
                } => "No stdout or stderr content was captured.",
                CommandOutput::Terminal { output: _ } => "No terminal output was captured.",
            }
            .to_owned(),
        );
    }
    lines.join("\n")
}
