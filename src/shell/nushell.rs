use std::path::Path;
pub(super) fn startup_args(cwd: &Path, ready_file: &Path) -> Vec<String> {
    vec![
        "--no-config-file".to_owned(),
        "--no-history".to_owned(),
        "--execute".to_owned(),
        initialization_script(cwd, ready_file),
    ]
}
pub(super) fn invocation(command_id: &str, directory: &Path, cwd: &Path) -> String {
    let line_ending = invocation_line_ending();
    format!(
        "functerm_run_command {} {} {}{}",
        nu_quote(command_id),
        nu_quote(&directory.to_string_lossy()),
        nu_quote(&cwd.to_string_lossy()),
        line_ending
    )
}
#[cfg(windows)]
const fn invocation_line_ending() -> &'static str {
    "\r\n"
}
#[cfg(not(windows))]
const fn invocation_line_ending() -> &'static str {
    "\n"
}
fn initialization_script(cwd: &Path, ready_file: &Path) -> String {
    format!(
        "$env.FUNCTERM_CURRENT_SHELL = 'nu'\n{}\ncd {}\n'' | save --force --raw {}\n",
        include_str!("nushell_init.nu"),
        nu_quote(&cwd.to_string_lossy()),
        nu_quote(&ready_file.to_string_lossy())
    )
}
fn nu_quote(value: &str) -> String {
    let text = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{text}\"")
}
#[cfg(test)]
mod tests {
    #[test]
    fn quotes_single_quotes_for_nushell() {
        let quoted = super::nu_quote("a'b");
        assert_eq!(quoted, "\"a'b\"");
    }
    #[test]
    fn initialization_defines_function_and_cwd() {
        let script = super::initialization_script(
            std::path::Path::new("F:\\dir with ' quote"),
            std::path::Path::new("F:\\ready'file"),
        );
        assert!(script.contains("def functerm_run_command"));
        assert!(script.contains("cd \"F:\\\\dir with ' quote\""));
        assert!(script.contains("save --force --raw \"F:\\\\ready'file\""));
    }
    #[test]
    fn invocation_references_payload_file_by_directory() {
        let line = super::invocation(
            "command",
            std::path::Path::new("F:\\dir with ' quote"),
            std::path::Path::new("F:\\cwd"),
        );
        assert_eq!(line.matches("\"command\"").count(), 1);
        assert!(line.contains("\"F:\\\\dir with ' quote\""));
    }
}
