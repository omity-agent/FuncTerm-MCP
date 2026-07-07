#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, parse_command_result, parse_tab_view, run_cli, send_command, temp_root,
    };
    use core::time::Duration;
    use std::thread;
    #[test]
    fn cli_view_keeps_closed_tab_snapshot_after_operation_detects_exit() {
        let _guard = crate::support::locked();
        let cwd = temp_root();
        let created = create_tab(&cwd, "powershell");
        let command = "Write-Output 'MCP_PTY_BEFORE_CLOSE'";
        let close_output = send_command(&created.tab_id, command, 5.0);
        let close_query = parse_command_result(&close_output);
        assert!(close_query.finished);
        let _closed = send_command(&created.tab_id, "exit", 0.2);
        wait_for_shell_exit(&created.tab_id);
        let failed = send_command(&created.tab_id, "Write-Output 'MCP_PTY_AFTER_CLOSE'", 1.0);
        assert!(!failed.status.success());
        assert!(
            String::from_utf8_lossy(&failed.stderr)
                .contains("was generated, but its shell is gone")
        );
        let tab_view = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert!(!tab_view.alive);
        assert!(
            tab_view.screen.contains("MCP_PTY_BEFORE_CLOSE"),
            "closed tab should retain last screen: {}",
            tab_view.screen
        );
    }
    fn wait_for_shell_exit(tab_id: &str) {
        for _attempt in 0_usize..20 {
            let query = parse_tab_view(&run_cli(&["view", tab_id]));
            if !query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("shell should exit before probing closed-tab error");
    }
}
