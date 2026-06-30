use std::io::Write as _;
use std::process::{Command, Stdio};
use std::sync::Once;
static PRINT: Once = Once::new();
pub(super) fn print_once() {
    PRINT.call_once(|| {
        let output = Command::new("pwsh.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                "$PSVersionTable.PSVersion.ToString()",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "failed to query PowerShell version\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let version = String::from_utf8_lossy(&output.stdout);
        let message = format!("PowerShell test version: {}\n", version.trim());
        std::io::stdout().write_all(message.as_bytes()).unwrap();
        std::io::stdout().flush().unwrap();
    });
}
