#[cfg(test)]
#[path = "shells/shell_contract.rs"]
mod basic;
#[cfg(test)]
#[path = "shells/environment_reset.rs"]
mod environment;
#[path = "shells/matrix_support.rs"]
mod matrix;
#[cfg(test)]
#[path = "shells/nested.rs"]
mod nested;
#[cfg(test)]
#[path = "shells/interpreters/nested_repl.rs"]
mod nested_repl;
#[cfg(windows)]
#[path = "shells/powershell_scope.rs"]
mod powershell_scope;
#[cfg(test)]
#[path = "shells/interpreters/source_visibility.rs"]
mod source_visibility;
#[cfg(test)]
#[path = "shells/state.rs"]
mod state;
#[cfg(test)]
#[path = "shells/command_titles.rs"]
mod title;
