use crate::runtime::ipc::QueryResult;
use anyhow::{Context as _, Result};
use core::time::Duration;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;
#[derive(Clone)]
pub(super) struct CommandRecord {
    pub(super) shell_id: String,
    pub(super) initial_cwd: PathBuf,
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
    pub(super) done: PathBuf,
}
#[derive(Deserialize)]
pub(super) struct DoneFile {
    pub(super) exit_code: i32,
    pub(super) cwd: String,
}
pub(super) fn create_record(
    command_root: &Path,
    command_id: &str,
    shell_id: &str,
    initial_cwd: &Path,
) -> Result<CommandRecord> {
    let command_dir = command_root.join(command_id);
    fs::create_dir_all(&command_dir).context("failed to create command directory")?;
    Ok(CommandRecord {
        shell_id: shell_id.to_owned(),
        initial_cwd: initial_cwd.to_path_buf(),
        stdout: command_dir.join("stdout.txt"),
        stderr: command_dir.join("stderr.txt"),
        done: command_dir.join("done.json"),
    })
}
pub(super) fn wait_for_done(done: &Path, limit: Duration) -> bool {
    let start = Instant::now();
    loop {
        if done.exists() {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn command_query(record: &CommandRecord, fallback_cwd: &Path) -> Result<QueryResult> {
    let stdout = read_optional(&record.stdout)?;
    let stderr = read_optional(&record.stderr)?;
    let done = read_done(&record.done)?;
    let exit_code = done.as_ref().map(|file| file.exit_code);
    let cwd = done
        .as_ref()
        .map_or_else(|| path_text(fallback_cwd), |file| Ok(file.cwd.clone()))?;
    Ok(QueryResult::Command {
        cwd,
        finished: record.done.exists(),
        stdout,
        stderr,
        exit_code,
    })
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
    if let Some(body) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16_chunks(body, u16::from_le_bytes);
    }
    if let Some(body) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16_chunks(body, u16::from_be_bytes);
    }
    if let Some(body) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(body.to_vec()).context("text is not valid UTF-8");
    }
    String::from_utf8(bytes.to_vec()).context("text is not valid UTF-8")
}
fn decode_utf16_chunks(bytes: &[u8], convert: fn([u8; 2]) -> u16) -> Result<String> {
    let chunks = bytes.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        anyhow::bail!("UTF-16 text has an odd byte length");
    }
    let words = chunks
        .map(|chunk| {
            let pair = <[u8; 2]>::try_from(chunk).unwrap();
            convert(pair)
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words).context("text is not valid UTF-16")
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
fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("cwd is not valid UTF-8: {}", path.display()))
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use super::wait_for_done;
    use core::time::Duration;
    use std::path::Path;
    #[test]
    fn zero_wait_does_not_block_for_missing_done_file() {
        let missing_path = Path::new("Z:\\definitely-missing-command.done");
        assert!(!wait_for_done(missing_path, Duration::from_millis(0)));
    }
    #[test]
    fn reads_utf16_little_endian_output() {
        let bytes = [
            0xFF, 0xFE, b'H', 0x00, b'E', 0x00, b'L', 0x00, b'L', 0x00, b'O', 0x00,
        ];
        let text = super::decode_text(&bytes).unwrap();
        assert_eq!(text, "HELLO");
    }
    #[test]
    fn reads_utf8_with_bom_output() {
        let text = super::decode_text(&[0xEF, 0xBB, 0xBF, b'{', b'}']).unwrap();
        assert_eq!(text, "{}");
    }
}
