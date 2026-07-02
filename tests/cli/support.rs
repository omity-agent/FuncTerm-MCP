#[path = "support/command.rs"]
mod command;
#[path = "support/daemon.rs"]
mod daemon;
#[path = "support/parse.rs"]
mod parse;
#[path = "support/process.rs"]
mod process;
pub(crate) use command::{
    create_tab, create_tab_from_directory_argument, manual_write, run_cli, send_command,
};
#[cfg(windows)]
pub(crate) use command::{run_cli_with_pipes, send_test_command};
#[cfg(windows)]
pub(crate) use daemon::locked;
pub(crate) use daemon::locked_with_env;
#[cfg(unix)]
pub(crate) use parse::CommandResult;
pub(crate) use parse::{TabView, parse_command_id, parse_command_result, parse_tab_view};
