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
#[path = "shells/interpreter_transitions.rs"]
mod nested_repl;
#[cfg(test)]
#[path = "shells/state.rs"]
mod state;
