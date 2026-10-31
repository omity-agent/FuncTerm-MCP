use crate::engine::privileges::PrivilegeMode;
use crate::runtime::{
    client,
    config::Settings,
    protocol::{OutputMode, Request},
};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, ensure};
use clap::{Parser, Subcommand};
use core::num::NonZeroUsize;
use std::path::PathBuf;
#[derive(Parser)]
#[command(version, about)]
pub(super) struct Args {
    #[command(subcommand)]
    pub(super) command: Option<CliCommand>,
}
#[derive(Subcommand)]
pub(super) enum CliCommand {
    Mcp {
        #[arg(
            long,
            help = "Disable Shell Profile loading for all Tabs created by this MCP server"
        )]
        no_profile: bool,
        #[arg(
            long,
            value_enum,
            help = "Fix New Tab permissions and hide its elevated input"
        )]
        privilege_mode: Option<PrivilegeMode>,
        #[arg(
            long,
            value_enum,
            help = "Fix command output handling and hide its separate_output input"
        )]
        output_mode: Option<OutputMode>,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    NewTab {
        #[arg(long)]
        starting_directory: Option<PathBuf>,
        # [arg (long , default_value = "powershell" , value_parser = ShellChoice :: from_canonical_name)]
        starting_shell: ShellChoice,
        #[arg(long, help = "Disable Shell Profile loading")]
        no_profile: bool,
        #[arg(
            long,
            help = "Use Root/administrator permissions already held by the Daemon"
        )]
        elevated: bool,
    },
    Close {
        #[arg(long, required_unless_present = "current", conflicts_with = "current")]
        tab_id: Option<String>,
        #[arg(long)]
        current: bool,
    },
    ManualWrite {
        tab_id: String,
        #[arg(long, required_unless_present = "base64", conflicts_with = "base64")]
        text: Option<String>,
        #[arg(long, required_unless_present = "text", conflicts_with = "text")]
        base64: Option<String>,
        #[arg(long, default_value_t = 0.0)]
        wait_timeout: f64,
    },
    SendCommand {
        tab_id: String,
        #[arg(long)]
        command: String,
        #[arg(long, action = clap::ArgAction::Set)]
        separate_output: Option<bool>,
        #[arg(long, default_value_t = 0.0)]
        wait_timeout: f64,
    },
    View {
        id: String,
        #[arg(long, default_value_t = 0.0)]
        wait_timeout: f64,
        #[arg(long, default_value_t = NonZeroUsize::MIN)]
        start_token: NonZeroUsize,
    },
    #[command(hide = true)]
    InternalLaunchDaemon,
    #[command(hide = true)]
    InternalRunDaemon,
    #[cfg(unix)]
    #[command(hide = true)]
    InternalRunShell {
        #[arg(long)]
        uid: u32,
        #[arg(last = true, required = true)]
        command: Vec<std::ffi::OsString>,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    InternalPtyHost {
        #[arg(long)]
        rows: u16,
        #[arg(long)]
        cols: u16,
        #[arg(long)]
        starting_directory: PathBuf,
        #[arg(last = true, required = true)]
        command: Vec<std::ffi::OsString>,
    },
    #[command(hide = true)]
    InternalTimeMillis,
    #[cfg(windows)]
    #[command(hide = true)]
    InternalCmdPath {
        #[arg(long)]
        startup_directory: PathBuf,
    },
    #[command(hide = true)]
    InternalClaimCommand {
        #[arg(long)]
        session_root: PathBuf,
        # [arg (long , value_parser = ShellChoice :: from_canonical_name)]
        shell: ShellChoice,
    },
    #[command(hide = true)]
    InternalWriteDone {
        #[arg(long)]
        command_id: String,
        #[arg(long, allow_negative_numbers = true)]
        exit_code: i32,
        #[arg(long)]
        time_consumption: String,
        #[arg(long)]
        cwd: String,
        #[arg(long)]
        directory: PathBuf,
    },
    #[command(hide = true)]
    InternalWriteStart {
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        directory: PathBuf,
    },
    #[command(hide = true)]
    InternalEnsureShims {
        #[arg(long)]
        directory: PathBuf,
    },
}
#[derive(Subcommand)]
pub(super) enum DaemonCommand {
    /// Start the daemon in the background.
    Start,
    /// Stop the daemon and terminate tabs and running commands.
    Stop,
    /// Stop the daemon, then start a fresh instance.
    Restart,
}
pub(crate) fn close_target(tab_id: Option<String>) -> Result<String> {
    if let Some(id) = tab_id {
        return Ok(id);
    }
    let current = std::env::var(crate::shell::shims::TAB_ID_ENV)
        .context("cannot identify current Tab; run --current inside a FuncTerm Tab")?;
    ensure!(
        !current.is_empty(),
        "cannot identify current Tab; run --current inside a FuncTerm Tab"
    );
    Ok(current)
}
pub(crate) async fn execute(settings: &Settings, request: Request) -> Result<String> {
    client::ensure_daemon(settings).await?;
    let mut connection = client::DaemonClient::connect(settings).await?;
    Ok(connection.call(request).await?.into_plain_text())
}
