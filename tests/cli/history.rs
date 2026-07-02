#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, locked_with_env, manual_write, parse_command_query, parse_tab_query, run_cli,
        send_command,
    };
    use core::sync::atomic::{AtomicU64, Ordering};
    use core::time::Duration;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    static HISTORY_COUNTER: AtomicU64 = AtomicU64::new(0);
    #[test]
    fn bash_history_does_not_record_internal_invocations() {
        let Ok(bash) = which::which("bash") else {
            return;
        };
        let history_file = history_root("bash").join("bash_history.txt");
        let bash_text = bash.to_string_lossy();
        let _guard = locked_with_env(&[
            ("HISTFILE", history_file.to_str().unwrap()),
            ("SHELL_MCP_PTY_BASH", &bash_text),
        ]);
        let shell = create_tab(&std::env::temp_dir(), "bash");
        let options = parse_command_query(&send_command(
            &shell.tab_id,
            "set -o | grep '^history'; printf 'HISTFILE=%s\\n' \"${HISTFILE-unset}\"",
            5.0,
        ));
        assert!(options.stdout.contains("history"));
        assert!(options.stdout.contains("off"));
        assert!(options.stdout.contains("HISTFILE=unset"));
        let command = parse_command_query(&send_command(
            &shell.tab_id,
            "printf 'MCP_PTY_HISTORY_TEST\\n'",
            5.0,
        ));
        assert!(command.stdout.contains("MCP_PTY_HISTORY_TEST"));
        exit_shell(&shell.tab_id);
        assert_file_does_not_contain(&history_file, "functerm_run_command");
    }
    #[cfg(unix)]
    #[test]
    fn zsh_history_does_not_record_internal_invocations() {
        let Ok(zsh) = which::which("zsh") else { return };
        let history_file = history_root("zsh").join("zsh_history.txt");
        let zsh_text = zsh.to_string_lossy();
        let _guard = locked_with_env(&[
            ("HISTFILE", history_file.to_str().unwrap()),
            ("SHELL_MCP_PTY_ZSH", &zsh_text),
        ]);
        let shell = create_tab(&std::env::temp_dir(), "zsh");
        let options = parse_command_query(&send_command(
            &shell.tab_id,
            "print -r -- \"HISTFILE=${HISTFILE-unset}\"; print -r -- \"HISTSIZE=$HISTSIZE\"; print -r -- \"SAVEHIST=$SAVEHIST\"",
            5.0,
        ));
        assert!(options.stdout.contains("HISTFILE=unset"));
        assert!(options.stdout.contains("HISTSIZE=0"));
        assert!(options.stdout.contains("SAVEHIST=0"));
        let command = parse_command_query(&send_command(
            &shell.tab_id,
            "print -r -- 'MCP_PTY_HISTORY_TEST'",
            5.0,
        ));
        assert!(command.stdout.contains("MCP_PTY_HISTORY_TEST"));
        exit_shell(&shell.tab_id);
        assert_file_does_not_contain(&history_file, "functerm_run_command");
    }
    #[cfg(windows)]
    #[test]
    fn powershell_history_does_not_record_internal_invocations() {
        let app_data = history_root("powershell").join("appdata");
        fs::create_dir_all(&app_data).unwrap();
        let _guard = locked_with_env(&[("APPDATA", app_data.to_str().unwrap())]);
        let shell = create_tab(&std::env::temp_dir(), "powershell");
        let options = parse_command_query(&send_command(
            &shell.tab_id,
            "if (Get-Command Get-PSReadLineOption -ErrorAction SilentlyContinue) { (Get-PSReadLineOption).HistorySaveStyle } else { 'Unavailable' }",
            5.0,
        ));
        assert!(
            options.stdout.contains("SaveNothing") || options.stdout.contains("Unavailable"),
            "unexpected PSReadLine history setting: {}",
            options.stdout
        );
        let command = parse_command_query(&send_command(
            &shell.tab_id,
            "Write-Output 'MCP_PTY_HISTORY_TEST'",
            5.0,
        ));
        assert!(command.stdout.contains("MCP_PTY_HISTORY_TEST"));
        exit_shell(&shell.tab_id);
        for history_text in text_files_under(&app_data) {
            assert!(
                !history_text.contains("Invoke-FuncTermCommand"),
                "PowerShell history should not contain internal invocation: {history_text}"
            );
        }
    }
    fn history_root(shell: &str) -> PathBuf {
        let unique = HISTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join("shell-mcp-history-tests")
            .join(format!("{shell}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn exit_shell(tab_id: &str) {
        let written = manual_write(tab_id, b"exit\n");
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        for _attempt in 0_usize..30 {
            let query = parse_tab_query(&run_cli(&["view", tab_id]));
            if !query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("shell should exit after keyboard exit command");
    }
    fn assert_file_does_not_contain(path: &Path, needle: &str) {
        if !path.exists() {
            return;
        }
        let text = fs::read_to_string(path).unwrap();
        assert!(
            !text.contains(needle),
            "{} should not contain {needle:?}: {text}",
            path.display()
        );
    }
    #[cfg(windows)]
    fn text_files_under(root: &Path) -> Vec<String> {
        if !root.exists() {
            return Vec::new();
        }
        fs::read_dir(root)
            .unwrap()
            .map(Result::unwrap)
            .flat_map(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    text_files_under(&path)
                } else {
                    vec![fs::read_to_string(path).unwrap()]
                }
            })
            .collect()
    }
}
