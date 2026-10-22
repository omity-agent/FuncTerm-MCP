use super::{checked_text, run_terminal, terminal_output};
use crate::support::{create_tab, locked_with_env, temp_dir};
#[test]
fn terminal_mode_preserves_external_exit_codes() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-exit-status");
    #[cfg(windows)]
    let shells = ["powershell", "cmd", "bash", "nu"];
    #[cfg(unix)]
    let shells = ["bash", "zsh"];
    for shell in shells {
        let tab = create_tab(&directory, shell);
        let result = run_terminal(
            &tab.tab_id,
            "python -c \"import sys; print('NONZERO_TERMINAL', flush=True); sys.exit(7)\"",
            "10",
        );
        let text = checked_text(&result);
        assert!(
            terminal_output(&text).contains("NONZERO_TERMINAL"),
            "{shell}: {text}"
        );
        assert!(text.contains("<EXIT_CODE>\n7\n"), "{shell}: {text}");
        assert!(text.contains("<FINISHED>\ntrue\n"), "{shell}: {text}");
    }
}
