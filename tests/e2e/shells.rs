#[cfg(test)]
#[path = "shells/shell_contract.rs"]
mod basic;
#[cfg(test)]
#[path = "shells/interpreters/bun_console.rs"]
mod bun;
#[cfg(test)]
#[path = "shells/environment_reset.rs"]
mod environment;
#[path = "shells/matrix_support.rs"]
mod matrix;
#[cfg(test)]
#[path = "shells/nested.rs"]
mod nested;
#[cfg(test)]
#[path = "shells/interpreters/interactive_python.rs"]
mod python;
#[cfg(test)]
#[path = "shells/interpreters/repl_commands.rs"]
mod repl_commands;
#[cfg(test)]
#[path = "shells/state.rs"]
mod state;
#[cfg(test)]
#[path = "shells/command_titles.rs"]
mod title;
