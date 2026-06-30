use base64_turbo::STANDARD;
use std::path::Path;
use uuid::Uuid;
pub(super) fn startup_args(cwd: &Path) -> Vec<String> {
    vec![
        "--no-config-file".to_owned(),
        "--no-history".to_owned(),
        "--execute".to_owned(),
        initialization_script(cwd),
    ]
}
pub(super) fn invocation(command_id: Uuid, command: &str, directory: &Path) -> String {
    let payload = STANDARD.encode(command.as_bytes());
    format!(
        "mcp_pty_command {} {} {}\n",
        nu_quote(&command_id.to_string()),
        nu_quote(&payload),
        nu_quote(&directory.to_string_lossy())
    )
}
fn initialization_script(cwd: &Path) -> String {
    format!(
        "{}\ncd {}\n",
        include_str!("nushell_init.nu"),
        nu_quote(&cwd.to_string_lossy())
    )
}
fn nu_quote(value: &str) -> String {
    let text = value.replace('\'', "''");
    format!("'{text}'")
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    #[test]
    fn quotes_single_quotes_for_nushell() {
        let quoted = super::nu_quote("a'b");
        assert_eq!(quoted, "'a''b'");
    }
    #[test]
    fn initialization_defines_function_and_cwd() {
        let script = super::initialization_script(std::path::Path::new("F:\\dir with ' quote"));
        assert!(script.contains("def mcp_pty_command"));
        assert!(script.contains("cd 'F:\\dir with '' quote'"));
    }
}
