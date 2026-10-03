#[path = "harness/capacity.rs"]
mod capacity;
#[path = "harness/command_runner.rs"]
mod command;
#[path = "harness/daemon_process.rs"]
mod daemon;
#[path = "harness/discovery.rs"]
mod executable;
#[path = "harness/response_fields.rs"]
mod parse;
#[path = "harness/child_guard.rs"]
mod process;
#[path = "harness/scratch.rs"]
mod temp;
#[path = "harness/isolation.rs"]
mod test_environment;
pub(crate) use command::run_cli_with_env;
#[cfg(windows)]
pub(crate) use command::run_cli_with_pipes;
pub(crate) use command::{create_tab, create_tab_with_env, manual_write, run_cli, send_command};
#[cfg(windows)]
pub(crate) use daemon::locked;
pub(crate) use daemon::{TestGuard, locked_with_env};
pub(crate) use executable::required as required_executable;
pub(crate) use parse::CommandResult;
#[cfg(windows)]
pub(crate) use parse::assert_powershell_primary_prompt;
pub(crate) use parse::parse_tab_created;
pub(crate) use parse::{TabView, parse_command_id, parse_command_result, parse_tab_view};
pub(crate) use temp::{command_directory, temp_dir, temp_root};
