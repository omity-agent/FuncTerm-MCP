use super::{
    generated::zsh_wrapper,
    posix::{sh_path, sh_quote},
    shims::CURRENT_SHELL_ENV,
};
use crate::contract::POSIX_COMMAND_FUNCTION;
use anyhow::{Context as _, Result};
use std::fs;
use std::path::Path;
type Startup = (Vec<String>, Vec<(String, String)>);
pub(super) fn startup(cwd: &Path, session_root: &Path, ready_file: &Path) -> Result<Startup> {
    let init_path = session_root.join(".zshrc");
    let script = initialization_script(cwd, ready_file);
    fs::write(&init_path, script).context("failed to write Zsh initialization script")?;
    Ok((
        vec!["-i".to_owned()],
        vec![("ZDOTDIR".to_owned(), sh_path(session_root))],
    ))
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
        "export {CURRENT_SHELL_ENV}=zsh\n{}\ncd {}\n: >| {}\n",
        zsh_wrapper(),
        sh_quote(&sh_path(cwd)),
        sh_quote(&sh_path(ready_file))
    )
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn startup_uses_zdotdir_to_load_generated_zshrc() {
        let root = std::env::temp_dir()
            .join("functerm-zsh-startup-test")
            .join(std::process::id().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let startup =
            super::startup(Path::new("F:\\cwd"), &root, &root.join("startup.ready")).unwrap();
        assert_eq!(startup.0, ["-i"]);
        assert_eq!(startup.1, [("ZDOTDIR".to_owned(), super::sh_path(&root))]);
    }
    #[test]
    fn initialization_defines_function_and_cwd() {
        let script =
            super::initialization_script(Path::new("F:\\dir with ' quote"), Path::new("F:\\ready"));
        assert!(script.contains("functerm_run_command()"));
        assert!(script.contains("cd 'F:/dir with '\\'' quote'"));
        assert!(script.contains(": >| 'F:/ready'"));
    }
}
