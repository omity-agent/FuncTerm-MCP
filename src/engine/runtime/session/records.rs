use crate::contract::{
    COMMAND_FILE, COMMAND_INPUT_DIRECTORY, COMMAND_OUTPUT_DIRECTORY,
    COMMAND_POWERSHELL_SCRIPT_FILE, COMMAND_SCRIPT_FILE, COMMAND_STATE_DIRECTORY,
    COMMAND_WORKING_DIRECTORY_FILE, DONE_FILE, STARTED_FILE, STDERR_FILE, STDOUT_FILE,
};
use crate::runtime::protocol::{CommandSnapshot, CommandView};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use fs_err as fs;
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub(super) struct CommandRecord {
    pub(super) directory: PathBuf,
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
    pub(super) command: PathBuf,
    pub(super) script: PathBuf,
    pub(super) powershell_script: PathBuf,
    pub(super) started: PathBuf,
    pub(super) done: PathBuf,
}
#[derive(Deserialize)]
pub(super) struct DoneFile {
    pub(super) exit_code: i32,
    pub(super) time_consumption: String,
    pub(super) cwd: String,
}
pub(super) fn create_record(
    command_root: &Path,
    command_id: &str,
    initial_cwd: &Path,
) -> Result<CommandRecord> {
    let command_dir = command_root.join(command_id);
    let input_dir = command_dir.join(COMMAND_INPUT_DIRECTORY);
    let output_dir = command_dir.join(COMMAND_OUTPUT_DIRECTORY);
    let state_dir = command_dir.join(COMMAND_STATE_DIRECTORY);
    fs::create_dir_all(&input_dir)?;
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(&state_dir)?;
    let working_directory = input_dir.join(COMMAND_WORKING_DIRECTORY_FILE);
    fs::write(
        &working_directory,
        crate::text::path_text(initial_cwd, "command working directory")?,
    )?;
    Ok(CommandRecord {
        directory: command_dir,
        stdout: output_dir.join(STDOUT_FILE),
        stderr: output_dir.join(STDERR_FILE),
        command: input_dir.join(COMMAND_FILE),
        script: input_dir.join(COMMAND_SCRIPT_FILE),
        powershell_script: input_dir.join(COMMAND_POWERSHELL_SCRIPT_FILE),
        started: state_dir.join(STARTED_FILE),
        done: state_dir.join(DONE_FILE),
    })
}
impl CommandRecord {
    pub(super) fn script_for(&self, choice: ShellChoice) -> &Path {
        match choice {
            ShellChoice::PowerShell => &self.powershell_script,
            ShellChoice::Bash
            | ShellChoice::NuShell
            | ShellChoice::Zsh
            | ShellChoice::Cmd
            | ShellChoice::Bun
            | ShellChoice::Python => &self.script,
        }
    }
}
pub(super) fn read_command_result(
    record: &CommandRecord,
    observed_time_consumption: Duration,
    title: String,
) -> Result<CommandSnapshot> {
    let stdout = read_plain_output(&record.stdout)?;
    let stderr = read_plain_output(&record.stderr)?;
    let done = read_done(&record.done)?;
    let exit_code = done.as_ref().map(|file| file.exit_code);
    let finished = done.is_some();
    let measured_time_consumption = done.map_or(Ok(observed_time_consumption), |file| {
        humantime::parse_duration(&file.time_consumption)
            .context("done file contains an invalid time consumption")
    })?;
    let note = command_note(&stdout, &stderr, "");
    Ok(CommandSnapshot {
        title,
        command: CommandView {
            stdout,
            stderr,
            exit_code,
            time_consumption: measured_time_consumption,
            finished,
        },
        note,
    })
}
pub(super) fn read_and_clear_command_result(
    record: &CommandRecord,
    time_consumption: Duration,
    title: String,
) -> Result<CommandSnapshot> {
    let result = read_command_result(record, time_consumption, title)?;
    if let Err(error) = remove_record_directory(record) {
        eprintln!("{error:#}");
    }
    Ok(result)
}
pub(super) fn remove_record_directory(record: &CommandRecord) -> Result<()> {
    match fs::remove_dir_all(&record.directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn command_note(stdout: &str, stderr: &str, extra: &str) -> String {
    let mut lines = Vec::new();
    if !extra.is_empty() {
        lines.push(extra.to_owned());
    }
    if stdout.is_empty() && stderr.is_empty() {
        lines.push("No stdout or stderr content was captured.".to_owned());
    }
    lines.join("\n")
}
fn read_plain_output(path: &Path) -> Result<String> {
    let Some(bytes) = read_if_present(path, "file")? else {
        return Ok(String::new());
    };
    let text =
        decode_text(&bytes).with_context(|| format!("failed to decode {}", path.display()))?;
    Ok(fast_strip_ansi::strip_ansi_string(&text).into_owned())
}
fn decode_text(bytes: &[u8]) -> Result<String> {
    let encoding = encoding_rs::Encoding::for_bom(bytes)
        .map_or(encoding_rs::UTF_8, |(detected_encoding, _)| {
            detected_encoding
        });
    let (text, had_errors) = encoding.decode_with_bom_removal(bytes);
    if had_errors {
        bail!("text is not valid {}", encoding.name());
    }
    Ok(text.into_owned())
}
pub(super) fn read_done(path: &Path) -> Result<Option<DoneFile>> {
    let Some(bytes) = read_if_present(path, "done file")? else {
        return Ok(None);
    };
    let text = decode_text(&bytes).context("failed to decode done file")?;
    let done = sonic_rs::from_str::<DoneFile>(&text).context("failed to parse done file")?;
    Ok(Some(done))
}
fn read_if_present(path: &Path, label: &str) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {label}")),
    }
}
#[cfg(test)]
#[path = "../../../../tests/unit/runtime/command_records.rs"]
mod tests;
