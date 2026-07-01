#[path = "support/command.rs"]
mod command;
#[path = "support/daemon.rs"]
mod daemon;
#[path = "support/parse.rs"]
mod parse;
#[path = "support/process.rs"]
mod process;
pub(crate) use command::{create_shell, run_cli, send_command, write_keyboard};
#[cfg(windows)]
pub(crate) use command::{run_cli_with_pipes, send_test_command};
#[cfg(windows)]
pub(crate) use daemon::locked;
pub(crate) use daemon::locked_with_env;
#[cfg(unix)]
pub(crate) use parse::CommandQuery;
pub(crate) use parse::{ShellQuery, parse_command_query, parse_shell_query};
