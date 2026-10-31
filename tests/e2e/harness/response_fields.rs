use std::process::Output;
pub(crate) struct TabCreated {
    pub(crate) tab_id: String,
}
#[derive(Debug)]
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
        stdout: tagged_content(&text, "STDOUT")
            .or_else(|| tagged_content(&text, "OUTPUT"))
            .unwrap_or_default()
            .to_owned(),
        stderr: tagged_content(&text, "STDERR")
            .unwrap_or_default()
            .trim_end()
            .to_owned(),
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
#[cfg(windows)]
pub(crate) fn assert_powershell_primary_prompt(view: &TabView) {
    let final_line = view.screen.lines().next_back().unwrap_or_default();
    assert!(
        final_line == "PS> " || (final_line.starts_with("PS ") && final_line.ends_with("> ")),
        "PowerShell should finish at its primary prompt:\n{}",
        view.screen
    );
}
pub(crate) fn parse_tab_created(output: &Output) -> TabCreated {
    let text = checked_stdout(output);
    TabCreated {
        tab_id: element(&text, "TAB_ID"),
    }
}
fn checked_stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}
fn element(text: &str, name: &str) -> String {
    let content = tagged_content(text, name)
        .unwrap_or_else(|| panic!("response is missing <{name}>: {text}"));
    if matches!(name, "STDOUT" | "STDERR") {
        return content.to_owned();
    }
    content
        .strip_prefix('\n')
        .unwrap()
        .trim_end_matches('\n')
        .to_owned()
}
pub(crate) fn tagged_content<'text>(text: &'text str, name: &str) -> Option<&'text str> {
    let tag = text
        .lines()
        .filter_map(|line| line.strip_prefix('<')?.strip_suffix('>'))
        .find(|tag| tag.eq_ignore_ascii_case(name))?;
    let (_, after_open) = text.split_once(&format!("<{tag}>"))?;
    let (content, _) = after_open.rsplit_once(&format!("</{tag}>"))?;
    Some(content)
}
fn parse_exit_code(value: &str) -> Option<i32> {
    match value {
        "" => None,
        code => Some(code.parse().unwrap()),
    }
}
