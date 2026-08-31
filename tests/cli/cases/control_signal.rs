#[cfg(test)]
mod tests {
    use crate::support::{
        assert_powershell_primary_prompt, create_tab, locked, manual_write, parse_command_id,
        parse_command_result, parse_tab_view, run_cli, send_command, temp_root,
    };
    use core::time::Duration;
    use functerm::shell::quote;
    use std::io::Write as _;
    use std::thread;
    use std::time::Instant;
    const PROBE_ENV: &str = "FUNCTERM_CTRL_C_PROBE";
    const PROBE_READY: &str = "FUNCTERM_CTRL_C_READY";
    #[test]
    fn ctrl_c_stops_native_process_and_returns_to_usable_shell() {
        let _guard = locked();
        let tab = create_tab(&temp_root(), "powershell");
        let accepted = send_command(&tab.tab_id, &probe_command(), 0.0);
        let command_id = parse_command_id(&accepted);
        let pending = parse_command_result(&accepted);
        assert!(!pending.finished, "native probe should still be running");
        wait_for_stdout(&command_id, PROBE_READY);
        let written = manual_write(&tab.tab_id, &[3], 0.0);
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        let interrupted = parse_command_result(&run_cli(&["view", &command_id, "--waiting", "5"]));
        assert!(
            interrupted.finished,
            "native process did not stop after Ctrl+C; stdout: {}; stderr: {}",
            interrupted.stdout, interrupted.stderr
        );
        assert_eq!(
            interrupted.exit_code,
            Some(130_i32),
            "Ctrl+C should publish the conventional interrupted exit code; stdout: {}; stderr: {}",
            interrupted.stdout,
            interrupted.stderr
        );
        let follow_up = parse_command_result(&send_command(
            &tab.tab_id,
            "Write-Output 'FUNCTERM_CTRL_C_RECOVERED'",
            5.0,
        ));
        assert!(follow_up.finished, "follow-up command should finish");
        assert_eq!(follow_up.exit_code, Some(0_i32));
        assert!(follow_up.stdout.contains("FUNCTERM_CTRL_C_RECOVERED"));
        let query = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
        assert_powershell_primary_prompt(&query);
    }
    #[test]
    fn native_ctrl_c_probe() {
        if std::env::var_os(PROBE_ENV).is_none() {
            return;
        }
        println!("{PROBE_READY}");
        std::io::stdout().flush().unwrap();
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        receiver.recv().unwrap();
    }
    fn probe_command() -> String {
        let executable = std::env::current_exe().unwrap();
        format!(
            "$env:{PROBE_ENV} = '1'; & {} 'control_signal::tests::native_ctrl_c_probe' '--exact' '--nocapture'",
            quote::powershell_path(&executable).unwrap()
        )
    }
    fn wait_for_stdout(command_id: &str, expected: &str) {
        let timeout = Duration::from_secs(5);
        let started = Instant::now();
        while started.elapsed() < timeout {
            let result = parse_command_result(&run_cli(&["view", command_id]));
            assert!(
                !result.finished,
                "command finished before producing {expected}: stdout: {}; stderr: {}",
                result.stdout, result.stderr
            );
            if result.stdout.contains(expected) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("command did not produce {expected} within {timeout:?}");
    }
}
