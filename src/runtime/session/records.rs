use crate::contract::{COMMAND_PAYLOAD_FILE, DONE_FILE, STDERR_FILE, STDOUT_FILE};
use crate::runtime::protocol::ViewResult;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;
#[derive(Clone)]
pub(super) struct CommandRecord {
    pub(super) initial_cwd: PathBuf,
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
    pub(super) payload: PathBuf,
    pub(super) done: PathBuf,
}
#[derive(Deserialize)]
pub(super) struct DoneFile {
    pub(super) exit_code: i32,
    pub(super) cwd: String,
}
#[derive(Serialize)]
struct FailedDoneFile<'value> {
    command_id: &'value str,
    exit_code: i32,
    cwd: String,
}
pub(super) fn create_record(
    command_root: &Path,
    command_id: &str,
    initial_cwd: &Path,
) -> Result<CommandRecord> {
    let command_dir = command_root.join(command_id);
    fs::create_dir_all(&command_dir).context("failed to create command directory")?;
    Ok(CommandRecord {
        initial_cwd: initial_cwd.to_path_buf(),
        stdout: command_dir.join(STDOUT_FILE),
        stderr: command_dir.join(STDERR_FILE),
        payload: command_dir.join(COMMAND_PAYLOAD_FILE),
        done: command_dir.join(DONE_FILE),
    })
}
pub(super) fn wait_for_done(done: &Path, limit: Duration) -> Result<bool> {
    wait_for_path(done, limit)
}
pub(super) fn wait_for_path(path: &Path, limit: Duration) -> Result<bool> {
    if path.exists() {
        return Ok(true);
    }
    if limit.is_zero() {
        return Ok(false);
    }
    let parent = path.parent().context("watched path has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create watched parent directory {}",
            parent.display()
        )
    })?;
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .context("failed to create filesystem watcher")?;
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch directory {}", parent.display()))?;
    if path.exists() {
        return Ok(true);
    }
    let start = Instant::now();
    loop {
        let Some(remaining) = limit.checked_sub(start.elapsed()) else {
            return Ok(false);
        };
        match rx.recv_timeout(remaining) {
            Ok(Ok(_event)) => {
                if path.exists() {
                    return Ok(true);
                }
            }
            Ok(Err(error)) => return Err(error).context("filesystem watcher failed"),
            Err(mpsc::RecvTimeoutError::Timeout) => return Ok(path.exists()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("filesystem watcher disconnected")
            }
        }
    }
}
pub(super) fn read_command_result(
    record: &CommandRecord,
    fallback_cwd: &Path,
) -> Result<ViewResult> {
    let stdout = read_optional(&record.stdout)?;
    let stderr = read_optional(&record.stderr)?;
    let done = read_done(&record.done)?;
    let exit_code = done.as_ref().map(|file| file.exit_code);
    let cwd = done.as_ref().map_or_else(
        || crate::text::path_text(fallback_cwd, "cwd"),
        |file| Ok(file.cwd.clone()),
    )?;
    Ok(ViewResult::Command {
        cwd,
        finished: done.is_some(),
        stdout,
        stderr,
        exit_code,
    })
}
pub(super) fn write_failed_result(
    command_id: &str,
    record: &CommandRecord,
    message: &str,
) -> Result<()> {
    if record.done.exists() {
        return Ok(());
    }
    if let Some(parent) = record.stderr.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create command directory {}", parent.display()))?;
    }
    let mut stderr = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&record.stderr)
        .with_context(|| format!("failed to open {}", record.stderr.display()))?;
    writeln!(stderr, "{message}")
        .with_context(|| format!("failed to write {}", record.stderr.display()))?;
    let done = FailedDoneFile {
        command_id,
        exit_code: 1_i32,
        cwd: crate::text::path_text(&record.initial_cwd, "cwd")?,
    };
    let text = sonic_rs::to_string(&done).context("failed to serialize failed done file")?;
    let temp_path = record.done.with_extension("json.tmp");
    fs::write(&temp_path, text)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    match fs::rename(&temp_path, &record.done) {
        Ok(()) => Ok(()),
        Err(_error) if record.done.exists() => {
            fs::remove_file(&temp_path)
                .with_context(|| format!("failed to remove {}", temp_path.display()))?;
            Ok(())
        }
        Err(error) => Err(error).context("failed to publish failed done file"),
    }
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
mod tests;
