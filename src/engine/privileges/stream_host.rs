use super::access_token::{current_token, is_elevated};
use anyhow::{Context as _, Result, ensure};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::os::windows::io::AsHandle as _;
use std::path::Path;
use std::thread;
pub(crate) fn run(rows: u16, cols: u16, cwd: &Path, arguments: Vec<OsString>) -> Result<i32> {
    ensure!(
        !is_elevated(&current_token()?)?,
        "ordinary PTY host still has administrator permissions"
    );
    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open ordinary PTY")?;
    let mut command = CommandBuilder::from_argv(arguments);
    command.cwd(cwd);
    command.env_clear();
    for (name, value) in std::env::vars_os() {
        command.env(name, value);
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .context("failed to start shell in ordinary PTY")?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .context("failed to read ordinary PTY")?;
    let mut writer = pair
        .master
        .take_writer()
        .context("failed to write ordinary PTY")?;
    let _input_worker = thread::Builder::new()
        .name("pty-input-forwarder".into())
        .spawn(move || {
            let mut input = io::stdin().lock();
            let copied = io::copy(&mut input, &mut writer);
            drop(input);
            if let Err(error) = copied {
                eprintln!("failed to forward ordinary PTY input: {error}");
            }
        })
        .context("failed to start ordinary PTY input forwarding")?;
    let output_worker = thread::Builder::new()
        .name("pty-output-forwarder".into())
        .spawn(move || {
            let output_handle = io::stdout().as_handle().try_clone_to_owned()?;
            let mut output = std::fs::File::from(output_handle);
            io::copy(&mut reader, &mut output)?;
            output.flush()
        })
        .context("failed to start ordinary PTY output forwarding")?;
    let status = child
        .wait()
        .context("failed to wait for ordinary PTY shell")?;
    drop(pair.slave);
    drop(pair.master);
    output_worker.join().map_err(|panic_payload| {
        anyhow::anyhow!("ordinary PTY output forwarding panicked: {panic_payload:?}")
    })??;
    Ok(status.exit_code().cast_signed())
}
