use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use std::fs;
use std::path::Path;
use uuid::Uuid;
pub(super) fn startup_args(cwd: &Path, session_root: &Path) -> Result<Vec<String>> {
    let init_path = session_root.join("bash_init.sh");
    let script = initialization_script(cwd);
    fs::write(&init_path, script).context("failed to write Bash initialization script")?;
    Ok(vec![
        "--noprofile".to_owned(),
        "--rcfile".to_owned(),
        bash_path(&init_path),
        "-i".to_owned(),
    ])
}
pub(super) fn invocation(command_id: Uuid, command: &str, directory: &Path) -> String {
    let payload = STANDARD.encode(command.as_bytes());
    format!(
        "mcp_pty_command {} {} {}\n",
        sh_quote(&command_id.to_string()),
        sh_quote(&payload),
        sh_quote(&bash_path(directory))
    )
}
fn initialization_script(cwd: &Path) -> String {
    format!(
        "{}\ncd {}\n",
        include_str!("bash_init.sh"),
        sh_quote(&bash_path(cwd))
    )
}
fn bash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use std::path::Path;
    #[test]
    fn converts_windows_paths_for_bash() {
        let converted = super::bash_path(Path::new("F:\\dir\\child"));
        assert_eq!(converted, "F:/dir/child");
    }
    #[test]
    fn quotes_single_quotes_for_shell() {
        let quoted = super::sh_quote("a'b");
        assert_eq!(quoted, "'a'\\''b'");
    }
}
