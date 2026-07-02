use std::process::Output;
pub(crate) struct TabCreated {
    pub(crate) tab_id: String,
}
pub(crate) struct CommandQuery {
    pub(crate) recognized_as: String,
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
}
pub(crate) struct TabQuery {
    pub(crate) recognized_as: String,
    pub(crate) alive: bool,
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
pub(crate) fn parse_tab_query(output: &Output) -> TabQuery {
    let text = checked_stdout(output);
    let (_, screen) = text.split_once("screen:\n").unwrap();
    TabQuery {
        recognized_as: field(&text, "recognized_as"),
        alive: field(&text, "alive").parse().unwrap(),
        cwd: field(&text, "cwd"),
        screen: screen.to_owned(),
    }
}
pub(super) fn parse_tab_created(output: &Output) -> TabCreated {
    let text = checked_stdout(output);
    TabCreated {
        tab_id: field(&text, "tab_id"),
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
