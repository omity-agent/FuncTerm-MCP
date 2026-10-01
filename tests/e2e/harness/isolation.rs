use super::executable::is_functerm_runtime_shim;
use std::ffi::{OsStr, OsString};
use std::process::Command;
pub(crate) fn apply(command: &mut Command, environment: &[(String, String)]) {
    sanitize_inherited(command);
    for pair in environment {
        if pair.0.eq_ignore_ascii_case("PATH") {
            command.env(&pair.0, sanitize_path(pair.1.as_ref()));
        } else {
            command.env(&pair.0, &pair.1);
        }
    }
}
fn sanitize_inherited(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("FUNCTERM_")
        {
            command.env_remove(name);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", sanitize_path(&path));
    }
}
fn sanitize_path(path: &OsStr) -> OsString {
    let entries = std::env::split_paths(path)
        .filter(|entry| !is_functerm_runtime_shim(entry))
        .collect::<Vec<_>>();
    std::env::join_paths(entries).unwrap()
}
