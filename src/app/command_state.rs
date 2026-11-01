use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
mod claim;
mod receipt;
#[cfg(windows)]
mod search_path;
mod title;
pub(crate) use claim::claim_command;
#[cfg(windows)]
pub(crate) use search_path::cmd_search_path;
pub(crate) fn time_millis() -> Result<String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?;
    let rounded = elapsed
        .checked_add(Duration::from_micros(500))
        .context("system clock timestamp overflowed")?
        .as_millis();
    Ok(rounded.to_string())
}
pub(crate) type DoneOutput<'value> = crate::contract::CommandCompletion<&'value str>;
pub(crate) fn write_start(command_id: &str, directory: &Path, stdio: bool) -> Result<()> {
    let mut terminal = marker_output(stdio)?;
    title::restore(&mut terminal, directory, stdio)?;
    write_start_to(command_id, directory, &mut terminal)
}
fn write_start_to(command_id: &str, directory: &Path, output: &mut impl Write) -> Result<()> {
    anyhow::ensure!(directory.is_dir(), "command directory is missing");
    write_marker(output, crate::contract::TERMINAL_MARKER_START, command_id)
}
pub(crate) fn write_done(done: &DoneOutput<'_>, directory: &Path) -> Result<()> {
    complete(done, directory, false, false)
}
pub(crate) fn complete(
    done: &DoneOutput<'_>,
    directory: &Path,
    replay_output: bool,
    stdio: bool,
) -> Result<()> {
    let mut terminal = marker_output(stdio)?;
    if replay_output {
        for name in [crate::contract::STDOUT_FILE, crate::contract::STDERR_FILE] {
            let mut stream = std::fs::File::open(directory.join(name))
                .with_context(|| format!("failed to open captured {name}"))?;
            std::io::copy(&mut stream, &mut terminal)
                .with_context(|| format!("failed to replay captured {name}"))?;
        }
    }
    let guard = early_done_guard(done.command_id, directory)?;
    match fs_err::remove_file(guard) {
        Ok(()) => return write_released(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("failed to consume early completion guard"),
    }
    write_done_to(done, directory, &mut terminal)?;
    write_released(directory)
}
fn marker_output(stdio: bool) -> Result<Box<dyn Write>> {
    if stdio {
        Ok(Box::new(std::io::stdout()))
    } else {
        Ok(Box::new(crate::shim::terminal_output()?))
    }
}
fn write_released(directory: &Path) -> Result<()> {
    let path = directory.join(crate::contract::RELEASED_FILE);
    crate::publication::write_once(&path, b"").context("failed to publish command release file")
}
pub(crate) fn write_done_to(
    done: &DoneOutput<'_>,
    directory: &Path,
    output: &mut impl Write,
) -> Result<()> {
    let done_path = directory.join(crate::contract::DONE_FILE);
    let text = sonic_rs::to_string(done).context("failed to serialize done file")?;
    crate::publication::write_once(&done_path, text).context("failed to publish done file")?;
    write_marker(
        output,
        crate::contract::TERMINAL_MARKER_END,
        done.command_id,
    )
}
pub(crate) fn write_early_done_guard(command_id: &str, directory: &Path) -> Result<()> {
    let guard = early_done_guard(command_id, directory)?;
    crate::publication::write_once(&guard, b"")
        .context("failed to publish early command completion guard")
}
fn early_done_guard(command_id: &str, directory: &Path) -> Result<std::path::PathBuf> {
    let commands_directory = directory
        .parent()
        .context("command directory has no commands parent")?;
    Ok(commands_directory
        .join(crate::contract::EARLY_DONE_DIRECTORY)
        .join(command_id))
}
fn write_marker(output: &mut impl Write, phase: &[u8], command_id: &str) -> Result<()> {
    let command_id_bytes = command_id.as_bytes();
    for (name, field) in [
        ("code", crate::contract::TERMINAL_MARKER_CODE),
        ("name", crate::contract::TERMINAL_MARKER_NAME),
        ("phase", phase),
        ("command id", command_id_bytes),
    ] {
        validate_marker_field(name, field)?;
    }
    let marker = [
        b"\x1b]".as_slice(),
        crate::contract::TERMINAL_MARKER_CODE,
        b";".as_slice(),
        crate::contract::TERMINAL_MARKER_NAME,
        b";".as_slice(),
        phase,
        b";".as_slice(),
        command_id_bytes,
        b"\x1b\\".as_slice(),
    ]
    .concat();
    output
        .write_all(&marker)
        .context("failed to write terminal command marker")?;
    output
        .flush()
        .context("failed to flush terminal command marker")
}
fn validate_marker_field(name: &str, field: &[u8]) -> Result<()> {
    if field.contains(&b';') || field.iter().any(u8::is_ascii_control) {
        bail!("terminal marker {name} contains a control character or semicolon");
    }
    Ok(())
}
