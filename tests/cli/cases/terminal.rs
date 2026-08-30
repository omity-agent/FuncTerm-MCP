#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use crate::support::locked;
    #[cfg(unix)]
    use crate::support::locked_with_env;
    use crate::support::{create_tab, parse_command_result, send_command, temp_root};
    use functerm::shell::quote;
    use std::io::{IsTerminal as _, stderr, stdin, stdout};
    use std::path::Path;
    const PROBE_ENV: &str = "FUNCTERM_IS_TERMINAL_PROBE";
    const PROBE_MARKER: &str = "FUNCTERM_IS_TERMINAL";
    #[test]
    fn cli_tab_exposes_terminal_stdin_to_child_programs() {
        #[cfg(windows)]
        let _guard = locked();
        #[cfg(windows)]
        let shell = "powershell";
        #[cfg(unix)]
        let bash = required_executable("bash");
        #[cfg(unix)]
        let _guard = locked_with_env(&[("FUNCTERM_BASH", &bash)]);
        #[cfg(unix)]
        let shell = "bash";
        let created = create_tab(&temp_root(), shell);
        let executable = std::env::current_exe().unwrap();
        let command = probe_command(&executable);
        let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(
            result.finished,
            "terminal probe should finish: stdout: {}\nstderr: {}",
            result.stdout, result.stderr
        );
        assert_eq!(
            result.exit_code,
            Some(0_i32),
            "stdout: {}\nstderr: {}",
            result.stdout,
            result.stderr
        );
        assert!(
            result
                .stdout
                .contains("FUNCTERM_IS_TERMINAL stdin=true stdout=false stderr=false"),
            "unexpected terminal probe output: {}",
            result.stdout
        );
    }
    #[test]
    fn is_terminal_probe() {
        if std::env::var_os(PROBE_ENV).is_none() {
            return;
        }
        println!(
            "{PROBE_MARKER} stdin={} stdout={} stderr={}",
            stdin().is_terminal(),
            stdout().is_terminal(),
            stderr().is_terminal()
        );
    }
    #[cfg(windows)]
    fn probe_command(executable: &Path) -> String {
        format!(
            "$env:{PROBE_ENV} = '1'; & {} is_terminal_probe --nocapture",
            quote::powershell_path(executable).unwrap()
        )
    }
    #[cfg(unix)]
    fn probe_command(executable: &Path) -> String {
        let path = quote::native_path(executable).unwrap();
        format!(
            "{PROBE_ENV}=1 {} is_terminal_probe --nocapture",
            quote::posix_string(&path)
        )
    }
    #[cfg(unix)]
    fn required_executable(name: &str) -> String {
        crate::support::required_executable(&[name])
            .to_string_lossy()
            .into_owned()
    }
}
