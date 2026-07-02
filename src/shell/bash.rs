use super::{
    generated::bash_wrapper,
    posix::{sh_path, sh_quote},
    shims::CURRENT_SHELL_ENV,
};
use crate::contract::POSIX_COMMAND_FUNCTION;
use anyhow::{Context as _, Result};
use std::fs;
use std::path::Path;
pub(super) fn startup_args(
    cwd: &Path,
    session_root: &Path,
    ready_file: &Path,
) -> Result<Vec<String>> {
    let init_path = session_root.join("bash_init.sh");
    let script = initialization_script(cwd, ready_file);
    fs::write(&init_path, script).context("failed to write Bash initialization script")?;
    Ok(vec![
        "--noprofile".to_owned(),
        "--rcfile".to_owned(),
        sh_path(&init_path),
        "-i".to_owned(),
    ])
}
pub(super) fn invocation(command_id: &str, directory: &Path, cwd: &Path) -> String {
    format!(
        "{POSIX_COMMAND_FUNCTION} {} {} {}\n",
        sh_quote(command_id),
        sh_quote(&sh_path(directory)),
        sh_quote(&sh_path(cwd))
    )
}
fn initialization_script(cwd: &Path, ready_file: &Path) -> String {
    format!(
        "export {CURRENT_SHELL_ENV}=bash\n{}\ncd {}\n: > {}\n",
        bash_wrapper(),
        sh_quote(&sh_path(cwd)),
        sh_quote(&sh_path(ready_file))
    )
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn initialization_converts_windows_paths_for_bash() {
        let script =
            super::initialization_script(Path::new("F:\\dir\\child"), Path::new("F:\\ready"));
        assert!(script.contains("cd 'F:/dir/child'"));
    }
    #[test]
    fn invocation_converts_windows_paths_for_bash() {
        let converted =
            super::invocation("command", Path::new("F:\\dir\\child"), Path::new("F:\\cwd"));
        assert!(!converted.contains("printf ok"));
        assert!(converted.contains("'F:/dir/child'"));
    }
}
