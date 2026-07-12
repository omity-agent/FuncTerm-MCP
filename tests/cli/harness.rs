#[path = "support/command_runner.rs"]
mod command;
#[path = "support/daemon_process.rs"]
mod daemon;
#[path = "support/parse.rs"]
mod parse;
#[path = "support/process.rs"]
mod process;
#[path = "../support/temp.rs"]
mod temp;
pub(crate) use command::run_cli_with_env;
#[cfg(windows)]
pub(crate) use command::run_cli_with_pipes;
pub(crate) use command::{
    create_tab, create_tab_with_env, manual_write, run_cli, send_command, send_command_with_env,
};
#[cfg(windows)]
pub(crate) use daemon::locked;
pub(crate) use daemon::{TestGuard, locked_with_env};
pub(crate) use parse::CommandResult;
pub(crate) use parse::{TabView, parse_command_id, parse_command_result, parse_tab_view};
pub(crate) use temp::{temp_dir, temp_root};
