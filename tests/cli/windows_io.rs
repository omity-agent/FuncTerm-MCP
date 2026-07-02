#[cfg(test)]
mod tests {
    use crate::support::{locked, run_cli_with_pipes};
    #[test]
    fn cli_pipe_capture_returns_without_hanging() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
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
}
