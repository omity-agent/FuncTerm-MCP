use std::process::Output;
pub(crate) struct ShellCreated {
    pub(crate) shell_id: String,
}
pub(crate) struct CommandQuery {
    pub(crate) recognized_as: String,
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
}
pub(crate) struct ShellQuery {
    pub(crate) recognized_as: String,
    pub(crate) cwd: String,
    pub(crate) screen: String,
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
pub(crate) fn parse_shell_query(output: &Output) -> ShellQuery {
    let text = checked_stdout(output);
    let (_, screen) = text.split_once("screen:\n").unwrap();
    ShellQuery {
        recognized_as: field(&text, "recognized_as"),
        cwd: field(&text, "cwd"),
        screen: screen.to_owned(),
    }
}
pub(super) fn parse_shell_created(output: &Output) -> ShellCreated {
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
