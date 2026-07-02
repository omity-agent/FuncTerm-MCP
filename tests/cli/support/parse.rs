use std::process::Output;
pub(crate) struct TabCreated {
    pub(crate) tab_id: String,
}
pub(crate) struct CommandQuery {
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
}
pub(crate) struct TabQuery {
    pub(crate) alive: bool,
    pub(crate) cwd: String,
    pub(crate) screen: String,
}
pub(crate) fn parse_command_query(output: &Output) -> CommandQuery {
    let text = checked_stdout(output);
    CommandQuery {
        cwd: element(&text, "CWD"),
        finished: element(&text, "FINISHED").parse().unwrap(),
        exit_code: parse_exit_code(&element(&text, "EXIT_CODE")),
        stdout: element(&text, "STDOUT"),
        stderr: element(&text, "STDERR").trim_end().to_owned(),
    }
}
pub(crate) fn parse_tab_query(output: &Output) -> TabQuery {
    let text = checked_stdout(output);
    TabQuery {
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
    let open = format!("<{name}>\n");
    let close = format!("\n</{name}>");
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
