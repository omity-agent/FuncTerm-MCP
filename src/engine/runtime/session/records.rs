use crate::contract::{
    COMMAND_FILE, COMMAND_INPUT_DIRECTORY, COMMAND_OUTPUT_DIRECTORY,
    COMMAND_POWERSHELL_SCRIPT_FILE, COMMAND_SCRIPT_FILE, COMMAND_STATE_DIRECTORY,
    COMMAND_WORKING_DIRECTORY_FILE, DONE_FILE, STARTED_FILE, STDERR_FILE, STDOUT_FILE,
};
use crate::runtime::protocol::{CommandSnapshot, CommandView};
use crate::shell::ShellChoice;
mod wait;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
pub(crate) use wait::wait_for_path;
pub(super) use wait::{wait_for_done, wait_for_start_or_done};
#[derive(Clone)]
pub(super) struct CommandRecord {
    pub(super) directory: PathBuf,
    pub(super) initial_cwd: PathBuf,
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
#[derive(Serialize)]
struct FailedDoneFile<'value> {
    command_id: &'value str,
    exit_code: i32,
    time_consumption: &'value str,
    cwd: String,
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
    fs::create_dir_all(&input_dir).context("failed to create command input directory")?;
    fs::create_dir_all(&output_dir).context("failed to create command output directory")?;
    fs::create_dir_all(&state_dir).context("failed to create command state directory")?;
    let working_directory = input_dir.join(COMMAND_WORKING_DIRECTORY_FILE);
    fs::write(
        &working_directory,
        crate::text::path_text(initial_cwd, "command working directory")?,
    )
    .context("failed to write command working directory")?;
    Ok(CommandRecord {
        directory: command_dir,
        initial_cwd: initial_cwd.to_path_buf(),
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
            ShellChoice::Bash | ShellChoice::NuShell | ShellChoice::Zsh | ShellChoice::Cmd => {
                &self.script
            }
        }
    }
}
pub(super) fn read_command_result(
    record: &CommandRecord,
    observed_time_consumption: Duration,
    title: String,
) -> Result<CommandSnapshot> {
    let stdout = read_optional(&record.stdout)?;
    let stderr = read_optional(&record.stderr)?;
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
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to remove command directory {}",
                record.directory.display()
            )
        }),
    }
}
pub(super) fn write_failed_result(
    command_id: &str,
    record: &CommandRecord,
    _message: &str,
) -> Result<()> {
    if let Some(parent) = record.stderr.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create command directory {}", parent.display()))?;
    }
    let done = FailedDoneFile {
        command_id,
        exit_code: 1_i32,
        time_consumption: "0ns",
        cwd: crate::text::path_text(&record.initial_cwd, "cwd")?,
    };
    let text = sonic_rs::to_string(&done).context("failed to serialize failed done file")?;
    crate::file_publish::write_once(&record.done, text)
        .context("failed to publish failed done file")
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
fn read_optional(path: &Path) -> Result<String> {
    if path.exists() {
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        decode_text(&bytes).with_context(|| format!("failed to decode {}", path.display()))
    } else {
        Ok(String::new())
    }
}
fn decode_text(bytes: &[u8]) -> Result<String> {
    let (encoding, body) =
        if let Some((detected_encoding, bom_length)) = encoding_rs::Encoding::for_bom(bytes) {
            let body = bytes
                .get(bom_length..)
                .context("detected BOM length exceeds text length")?;
            (detected_encoding, body)
        } else {
            (encoding_rs::UTF_8, bytes)
        };
    let (text, had_errors) = encoding.decode_without_bom_handling(body);
    if had_errors {
        bail!("text is not valid {}", encoding.name());
    }
    Ok(text.into_owned())
}
pub(super) fn read_done(path: &Path) -> Result<Option<DoneFile>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).context("failed to read done file")?;
    let text = decode_text(&bytes).context("failed to decode done file")?;
    let done = sonic_rs::from_str::<DoneFile>(&text).context("failed to parse done file")?;
    Ok(Some(done))
}
#[cfg(test)]
#[path = "records/record_tests.rs"]
mod tests;
