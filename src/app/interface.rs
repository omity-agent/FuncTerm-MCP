use crate::runtime::{client, config::Settings, protocol::Request};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, ensure};
use clap::{Parser, Subcommand};
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
    },
    Daemon,
    NewTab {
        #[arg(long)]
        starting_directory: Option<PathBuf>,
        # [arg (long , default_value = "powershell" , value_parser = ShellChoice :: from_canonical_name)]
        starting_shell: ShellChoice,
        #[arg(long, help = "Disable Shell Profile loading")]
        no_profile: bool,
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
        #[arg(long, default_value_t = 0.0)]
        wait_timeout: f64,
    },
    View {
        id: String,
        #[arg(long, default_value_t = 0.0)]
        wait_timeout: f64,
    },
    #[command(hide = true)]
    InternalLaunchDaemon,
    #[command(hide = true)]
    InternalTimeMillis,
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
