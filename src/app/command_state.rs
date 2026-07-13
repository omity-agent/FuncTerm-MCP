use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use std::io::Write;
use std::path::Path;
#[derive(Serialize)]
pub(crate) struct DoneOutput<'value> {
    pub(crate) command_id: &'value str,
    pub(crate) exit_code: i32,
    pub(crate) time_consumption: &'value str,
    pub(crate) cwd: &'value str,
}
pub(crate) fn write_start(command_id: &str, directory: &Path, model_title: &str) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    write_start_to(command_id, directory, model_title, &mut stdout)
}
fn write_start_to(
    command_id: &str,
    directory: &Path,
    model_title: &str,
    output: &mut impl Write,
) -> Result<()> {
    output
        .write_all(crate::contract::window_title_sequence(model_title)?.as_bytes())
        .context("failed to restore terminal model title")?;
    write_marker(output, crate::contract::TERMINAL_MARKER_START, command_id)?;
    let started_path = directory
        .join(crate::contract::COMMAND_STATE_DIRECTORY)
        .join(crate::contract::STARTED_FILE);
    crate::file_publish::write_once(&started_path, b"")
        .context("failed to publish command started file")
}
pub(crate) fn write_done(done: &DoneOutput<'_>, directory: &Path) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    write_done_to(done, directory, &mut stdout)
}
pub(crate) fn write_done_to(
    done: &DoneOutput<'_>,
    directory: &Path,
    output: &mut impl Write,
) -> Result<()> {
    write_marker(
        output,
        crate::contract::TERMINAL_MARKER_END,
        done.command_id,
    )?;
    let state_dir = directory.join(crate::contract::COMMAND_STATE_DIRECTORY);
    let done_path = state_dir.join(crate::contract::DONE_FILE);
    let text = sonic_rs::to_string(done).context("failed to serialize done file")?;
    crate::file_publish::write_once(&done_path, text).context("failed to publish done file")
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
#[cfg(test)]
mod tests {
    #[test]
    fn command_start_restores_model_title_before_capture_marker() {
        let directory = crate::test_fs::temp_dir("command-start-title");
        let mut output = Vec::new();
        super::write_start_to("command-a", &directory, "Model", &mut output).unwrap();
        assert_eq!(
            output,
            b"\x1b]2;Model\x1b\\\x1b]9999;FuncTerm;start;command-a\x1b\\"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
