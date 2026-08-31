#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, locked, parse_command_result, parse_tab_view, run_cli, run_cli_with_pipes,
        send_command, temp_root,
    };
    #[test]
    fn cli_pipe_capture_returns_without_hanging() {
        let _guard = locked();
        let cwd = temp_root();
        let output = run_cli_with_pipes(&[
            "new-tab",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            "powershell",
        ]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("<TAB_ID>"));
    }
    #[test]
    fn cli_streams_large_command_output_without_closing_the_tab() {
        const OUTPUT_SIZE: usize = 2 * 1024 * 1024;
        let _guard = locked();
        let created = create_tab(&temp_root(), "powershell");
        let command = parse_command_result(&send_command(
            &created.tab_id,
            &format!("Write-Output ('x' * {OUTPUT_SIZE})"),
            10.0,
        ));
        let output_bytes = command.stdout.bytes().filter(|byte| *byte == b'x').count();
        assert_eq!(output_bytes, OUTPUT_SIZE, "large stdout was truncated");
        let view = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert!(
            view.alive,
            "large output should not close the PowerShell tab"
        );
    }
}
