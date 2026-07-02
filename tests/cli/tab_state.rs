#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, manual_write, parse_command_query, parse_tab_query, run_cli, send_command,
    };
    use core::time::Duration;
    use std::thread;
    #[test]
    fn cli_tab_view_reports_last_command() {
        let _guard = crate::support::locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let command = "Write-Output 'MCP_PTY_LAST_COMMAND'";
        let accepted_output = send_command(&created.tab_id, command, 5.0);
        let command_query = parse_command_query(&accepted_output);
        assert!(command_query.finished);
        let tab_query = parse_tab_query(&run_cli(&["view", &created.tab_id]));
        assert_eq!(tab_query.last_command, command);
    }
    #[test]
    fn cli_view_keeps_closed_tab_snapshot_after_operation_detects_exit() {
        let _guard = crate::support::locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let command = "Write-Output 'MCP_PTY_BEFORE_CLOSE'";
        let close_output = send_command(&created.tab_id, command, 5.0);
        let close_query = parse_command_query(&close_output);
        assert!(close_query.finished);
        let written = manual_write(&created.tab_id, b"exit\n");
        assert!(written.status.success());
        wait_for_shell_exit(&created.tab_id);
        let failed = send_command(&created.tab_id, "Write-Output 'MCP_PTY_AFTER_CLOSE'", 1.0);
        assert!(!failed.status.success());
        assert!(
            String::from_utf8_lossy(&failed.stderr)
                .contains("was generated, but its shell is gone")
        );
        let tab_query = parse_tab_query(&run_cli(&["view", &created.tab_id]));
        assert!(!tab_query.alive);
        assert_eq!(tab_query.last_command, command);
        assert!(
            tab_query.screen.contains("MCP_PTY_BEFORE_CLOSE"),
            "closed tab should retain last screen: {}",
            tab_query.screen
        );
    }
    fn wait_for_shell_exit(tab_id: &str) {
        for _attempt in 0_usize..20 {
            let query = parse_tab_query(&run_cli(&["view", tab_id]));
            if !query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("shell should exit before probing closed-tab error");
    }
}
