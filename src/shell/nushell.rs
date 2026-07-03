use super::quote;
use super::shims::CURRENT_SHELL_ENV;
use super::wrappers::nushell_wrapper;
use crate::contract::POSIX_COMMAND_FUNCTION;
use anyhow::Result;
use std::path::Path;
pub(super) fn startup_args(cwd: &Path, ready_file: &Path) -> Result<Vec<String>> {
    Ok(vec![
        "--no-config-file".to_owned(),
        "--no-history".to_owned(),
        "--execute".to_owned(),
        initialization_script(cwd, ready_file)?,
    ])
}
pub(super) fn invocation(command_id: &str, directory: &Path, cwd: &Path) -> Result<String> {
    let line_ending = invocation_line_ending();
    Ok(format!(
        "{POSIX_COMMAND_FUNCTION} {} {} {}{}",
        quote::nushell_string(command_id),
        quote::nushell_path(directory)?,
        quote::nushell_path(cwd)?,
        line_ending
    ))
}
#[cfg(windows)]
const fn invocation_line_ending() -> &'static str {
    "\r\n"
}
#[cfg(not(windows))]
const fn invocation_line_ending() -> &'static str {
    "\n"
}
fn initialization_script(cwd: &Path, ready_file: &Path) -> Result<String> {
    Ok(format!(
        "$env.{CURRENT_SHELL_ENV} = 'nu'\n{}\ncd {}\n'' | save --force --raw {}\n",
        nushell_wrapper(),
        quote::nushell_path(cwd)?,
        quote::nushell_path(ready_file)?
    ))
}
#[cfg(test)]
mod tests {
    #[test]
    fn quotes_single_quotes_for_nushell() {
        let quoted = super::quote::nushell_string("a'b");
        assert_eq!(quoted, "('YSdi' | decode base64 | decode)");
    }
    #[test]
    fn initialization_defines_function_and_cwd() {
        let script = super::initialization_script(
            std::path::Path::new("F:\\dir with ' quote"),
            std::path::Path::new("F:\\ready'file"),
        )
        .unwrap();
        assert!(script.contains("def functerm_run_command"));
        assert!(script.contains("cd ('RjpcZGlyIHdpdGggJyBxdW90ZQ==' | decode base64 | decode)"));
        assert!(
            script.contains("save --force --raw ('RjpccmVhZHknZmlsZQ==' | decode base64 | decode)")
        );
    }
    #[test]
    fn invocation_references_payload_file_by_directory() {
        let line = super::invocation(
            "command",
            std::path::Path::new("F:\\dir with ' quote"),
            std::path::Path::new("F:\\cwd"),
        )
        .unwrap();
        assert_eq!(line.matches("Y29tbWFuZA==").count(), 1);
        assert!(line.contains("RjpcZGlyIHdpdGggJyBxdW90ZQ=="));
    }
}
