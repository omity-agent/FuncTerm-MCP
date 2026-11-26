use crate::contract::{CommandOutcome, CommandReport};
use anyhow::{Context as _, Result, bail, ensure};
use std::io::Write;
use std::path::Path;
mod claim;
mod receipt;
#[cfg(windows)]
mod search_path;
mod title;
pub(crate) use claim::claim_command;
#[cfg(windows)]
pub(crate) use search_path::cmd_search_path;
#[derive(Clone, Copy, clap::Args, Default)]
pub(crate) struct CompletionOptions {
    #[arg(long)]
    replay_output: bool,
    #[arg(long)]
    stdio: bool,
    #[arg(long, action = clap::ArgAction::Set, default_value_t = false)]
    pub(super) session_exit: bool,
}
pub(crate) fn write_start(command_id: &str, directory: &Path, stdio: bool) -> Result<()> {
    let mut terminal = marker_output(stdio)?;
    title::restore(&mut terminal, directory, stdio)?;
    write_start_to(command_id, directory, &mut terminal)
}
fn write_start_to(command_id: &str, directory: &Path, output: &mut impl Write) -> Result<()> {
    anyhow::ensure!(directory.is_dir(), "command directory is missing");
    write_marker(output, crate::contract::TERMINAL_MARKER_START, command_id)
}
pub(crate) fn complete(
    report: &CommandReport<&str>,
    directory: &Path,
    options: CompletionOptions,
) -> Result<()> {
    let mut terminal = marker_output(options.stdio)?;
    if options.replay_output {
        for name in [crate::contract::STDOUT_FILE, crate::contract::STDERR_FILE] {
            let mut stream = std::fs::File::open(directory.join(name))
                .with_context(|| format!("failed to open captured {name}"))?;
            std::io::copy(&mut stream, &mut terminal)
                .with_context(|| format!("failed to replay captured {name}"))?;
        }
    }
    if let Some(previous) = CommandReport::read(directory)? {
        ensure!(
            previous.command_id == report.command_id
                && matches!(previous.outcome, CommandOutcome::HandedOff),
            "cannot replace a published command result"
        );
        return release_handoff(
            directory,
            report.command_id,
            report.cwd,
            !report.outcome.exits_shell(),
        );
    }
    report_boundary(report, directory, &mut terminal)
}
pub(crate) fn release_handoff(
    directory: &Path,
    command_id: &str,
    cwd: &str,
    resume: bool,
) -> Result<()> {
    if resume {
        let command_root = directory
            .parent()
            .context("missing command root for suspended command")?;
        let tab_root = command_root
            .parent()
            .context("missing Tab workspace for suspended command")?;
        crate::shell::frame::ShellRoute::resume(tab_root, command_id, cwd)?;
    }
    crate::publication::write_once(&directory.join(crate::contract::RELEASED_FILE), b"")
        .context("failed to release suspended command record")
}
fn marker_output(stdio: bool) -> Result<Box<dyn Write>> {
    if stdio {
        Ok(Box::new(std::io::stdout()))
    } else {
        Ok(Box::new(crate::shim::terminal_output()?))
    }
}
pub(crate) fn report_boundary(
    report: &CommandReport<&str>,
    directory: &Path,
    output: &mut impl Write,
) -> Result<()> {
    report.publish(directory)?;
    write_marker(
        output,
        crate::contract::TERMINAL_MARKER_END,
        report.command_id,
    )
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
