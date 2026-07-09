use std::process::Output;
pub(crate) struct TabCreated {
    pub(crate) tab_id: String,
}
pub(crate) struct CommandResult {
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) time_consumption: String,
}
pub(crate) struct TabView {
    pub(crate) alive: bool,
    pub(crate) cwd: String,
    pub(crate) screen: String,
}
pub(crate) fn parse_command_result(output: &Output) -> CommandResult {
    let text = checked_stdout(output);
    CommandResult {
        cwd: element(&text, "CWD"),
        finished: element(&text, "FINISHED").parse().unwrap(),
        exit_code: parse_exit_code(&element(&text, "EXIT_CODE")),
        time_consumption: element(&text, "TIME_CONSUMPTION"),
        stdout: element(&text, "STDOUT"),
        stderr: element(&text, "STDERR").trim_end().to_owned(),
    }
}
pub(crate) fn parse_command_id(output: &Output) -> String {
    element(&checked_stdout(output), "COMMAND_ID")
}
pub(crate) fn parse_tab_view(output: &Output) -> TabView {
    let text = checked_stdout(output);
    TabView {
        alive: element(&text, "ALIVE").parse().unwrap(),
        cwd: element(&text, "CWD"),
        screen: element(&text, "SCREEN"),
    }
}
pub(super) fn parse_tab_created(output: &Output) -> TabCreated {
    let text = checked_stdout(output);
    TabCreated {
        tab_id: element(&text, "TAB_ID"),
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
fn element(text: &str, name: &str) -> String {
    if matches!(name, "STDOUT" | "STDERR") {
        return inline_element(text, name);
    }
    let open = format!("<{name}>\n");
    let close = format!("\n</{name}>");
    let (_, after_open) = text.split_once(&open).unwrap();
    let (content, _) = after_open.rsplit_once(&close).unwrap();
    content.to_owned()
}
fn inline_element(text: &str, name: &str) -> String {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let (_, after_open) = text.split_once(&open).unwrap();
    let (content, _) = after_open.rsplit_once(&close).unwrap();
    content.to_owned()
}
fn parse_exit_code(value: &str) -> Option<i32> {
    match value {
        "pending" => None,
        code => Some(code.parse().unwrap()),
    }
}
