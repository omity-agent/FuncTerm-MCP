use crate::contract::{DONE_FILE, RELEASED_FILE, STDERR_FILE, STDOUT_FILE};
use crate::runtime::protocol::{CommandOutput, CommandSnapshot, CommandView, OutputTokenizer};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, bail};
use bytestring::ByteString;
use core::time::Duration;
use fs_err as fs;
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub(super) struct CommandRecord {
    pub(super) directory: PathBuf,
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
    pub(super) source: PathBuf,
    pub(super) done: PathBuf,
    pub(super) released: PathBuf,
    pub(super) session_exit: PathBuf,
}
pub(super) type DoneFile = crate::contract::CommandCompletion<String>;
pub(super) fn lifecycle_event(event: &notify::Event) -> bool {
    event
        .paths
        .iter()
        .any(|path| path.file_name() == Some(std::ffi::OsStr::new(RELEASED_FILE)))
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
        done: command_dir.join(DONE_FILE),
        released: command_dir.join(RELEASED_FILE),
        session_exit: command_dir.join(crate::contract::SESSION_EXIT_FILE),
        directory: command_dir,
    })
}
pub(super) fn read_command_result(
    record: &CommandRecord,
    observed_time_consumption: Duration,
    title: String,
    terminal_output: Option<ByteString>,
    tokenizer: &OutputTokenizer,
    done: Option<&DoneFile>,
) -> Result<CommandSnapshot> {
    let output = match terminal_output {
        Some(output) => CommandOutput::Terminal { output },
        None => CommandOutput::Separated {
            stdout: read_plain_output(&record.stdout)?,
            stderr: read_plain_output(&record.stderr)?,
        },
    };
    let exit_code = done.map(|file| file.exit_code);
    let finished = done.is_some();
    let measured_time_consumption = done.map_or(Ok(observed_time_consumption), |file| {
        humantime::parse_duration(&file.time_consumption)
            .context("done file contains an invalid time consumption")
    })?;
    let note = command_note(&output, "");
    Ok(CommandSnapshot {
        title,
        command: CommandView {
            total_output_tokens: tokenizer.count(&output)?,
            output,
            exit_code,
            time_consumption: measured_time_consumption,
            finished,
        },
        note,
    })
}
pub(super) fn remove_record_directory(record: &CommandRecord) -> Result<()> {
    match fs::remove_dir_all(&record.directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
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
fn read_plain_output(path: &Path) -> Result<ByteString> {
    let Some(bytes) = read_if_present(path, "file")? else {
        return Ok(ByteString::new());
    };
    let text =
        decode_text(&bytes).with_context(|| format!("failed to decode {}", path.display()))?;
    Ok(fast_strip_ansi::strip_ansi_string(&text)
        .into_owned()
        .into())
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
