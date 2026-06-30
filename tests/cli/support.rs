use core::time::Duration;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
mod daemon;
#[path = "powershell_version.rs"]
mod powershell_version;
pub(crate) use daemon::{locked, locked_with_env};
const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) struct ChildGuard {
    child: Child,
}
pub(crate) struct ShellCreated {
    pub(crate) shell_id: String,
}
pub(crate) struct CommandAccepted {
    pub(crate) command_id: String,
}
pub(crate) struct CommandQuery {
    pub(crate) recognized_as: String,
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
}
impl ChildGuard {
    pub(crate) const fn new(child: Child) -> Self {
        Self { child }
    }
    pub(crate) fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this test guard"
)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}
pub(crate) fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_shell-mcp-pty"))
}
pub(crate) fn run_cli(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command.args(arguments).stdin(Stdio::null());
    daemon::apply_active_env(&mut command);
    output_to_files(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn run_cli_with_pipes(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    daemon::apply_active_env(&mut command);
    output_from_pipes(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn create_powershell_shell(cwd: &Path) -> ShellCreated {
    parse_shell_created(&run_cli(&[
        "new-shell",
        "--cwd",
        cwd.to_str().unwrap(),
        "--shell",
        "pwsh",
    ]))
}
pub(crate) fn send_test_command(shell_id: &str) -> Output {
    run_cli(&[
        "send-command",
        shell_id,
        "--command",
        "Write-Output 'MCP_PTY_TEST'",
        "--wait-ms",
        "5000",
    ])
}
pub(crate) fn parse_command_accepted(output: &Output) -> CommandAccepted {
    let text = checked_stdout(output);
    CommandAccepted {
        command_id: field(&text, "command_id"),
    }
}
pub(crate) fn parse_command_query(output: &Output) -> CommandQuery {
    let text = checked_stdout(output);
    let (_, after_stdout_marker) = text.split_once("stdout:\n").unwrap();
    let (stdout, stderr) = after_stdout_marker.split_once("\nstderr:\n").unwrap();
    CommandQuery {
        recognized_as: field(&text, "recognized_as"),
        cwd: field(&text, "cwd"),
        finished: field(&text, "finished").parse().unwrap(),
        exit_code: parse_exit_code(&field(&text, "exit_code")),
        stdout: stdout.to_owned(),
        stderr: stderr.trim_end().to_owned(),
    }
}
fn output_to_files(mut command: Command, timeout: Duration) -> Output {
    let (stdout_path, stderr_path) = output_paths();
    command
        .stdout(Stdio::from(fs::File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr_path).unwrap()));
    let status = wait_for_status(&mut command.spawn().unwrap(), timeout);
    let output = Output {
        status,
        stdout: fs::read(&stdout_path).unwrap(),
        stderr: fs::read(&stderr_path).unwrap(),
    };
    fs::remove_file(&stdout_path).unwrap();
    fs::remove_file(&stderr_path).unwrap();
    output
}
fn output_from_pipes(mut command: Command, timeout: Duration) -> Output {
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = read_pipe(stdout);
    let stderr_rx = read_pipe(stderr);
    let status = wait_for_status(&mut child, timeout);
    Output {
        status,
        stdout: stdout_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
        stderr: stderr_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
    }
}
fn wait_for_status(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() >= timeout {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("CLI command timed out after {timeout:?} with status {status}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
fn read_pipe(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).unwrap();
        sender.send(bytes).unwrap();
    });
    receiver
}
fn output_paths() -> (PathBuf, PathBuf) {
    let output_dir = std::env::temp_dir()
        .join("agent")
        .join("shell-mcp-test-output");
    fs::create_dir_all(&output_dir).unwrap();
    let prefix = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4().simple());
    (
        output_dir.join(format!("{prefix}.stdout.txt")),
        output_dir.join(format!("{prefix}.stderr.txt")),
    )
}
fn parse_shell_created(output: &Output) -> ShellCreated {
    let text = checked_stdout(output);
    ShellCreated {
        shell_id: field(&text, "shell_id"),
    }
}
fn checked_stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}
fn field(text: &str, name: &str) -> String {
    let prefix = format!("{name}: ");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .unwrap()
}
fn parse_exit_code(value: &str) -> Option<i32> {
    match value {
        "pending" => None,
        code => Some(code.parse().unwrap()),
    }
}
