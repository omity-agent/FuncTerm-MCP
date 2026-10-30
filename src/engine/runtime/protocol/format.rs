use super::{CommandView, OutputPresentation, Payload, ShellView, ViewResult};
impl Payload {
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Pong => element("PONG", ""),
            Self::ShutdownComplete => element("DAEMON_STOPPED", ""),
            Self::TabCreated { tab_id } => tab_created_plain_text(&tab_id),
            Self::TabClosed { tab_id } => element("TAB_CLOSED", &tab_id),
            Self::KeyboardWritten { view } => view.plain_text(false, None),
            Self::CommandAccepted {
                command_id,
                end_reason: _,
                view,
            } => view.plain_text(false, Some(&command_id)),
            Self::View(view) => view.into_plain_text(),
        }
    }
}
impl ViewResult {
    pub(crate) fn into_plain_text(self) -> String {
        self.plain_text(true, None)
    }
    fn plain_text(self, include_alive: bool, command_id: Option<&str>) -> String {
        match self {
            Self::Tab {
                shell,
                screen,
                note,
            } => tab_plain_text(&shell, &screen, &note, include_alive),
            Self::Command {
                shell,
                command,
                note,
            } => command_plain_text(&shell, &command, &note, include_alive, command_id),
        }
    }
}
#[must_use = "the returned text contains the line terminator"]
pub(crate) fn line_terminated(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}
pub(crate) fn tab_created_plain_text(tab_id: &str) -> String {
    element("TAB_ID", tab_id)
}
pub(crate) fn tab_plain_text(
    shell: &ShellView,
    screen: &str,
    note: &str,
    include_alive: bool,
) -> String {
    let mut text = String::new();
    append_element(&mut text, "SHELL", &shell_text(shell, include_alive));
    append_element(&mut text, "SCREEN", screen);
    append_element(&mut text, "NOTE", note);
    text
}
pub(crate) fn command_plain_text(
    shell: &ShellView,
    command: &CommandView,
    note: &str,
    include_alive: bool,
    command_id: Option<&str>,
) -> String {
    let mut text = String::new();
    append_element(&mut text, "SHELL", &shell_text(shell, include_alive));
    append_element(&mut text, "COMMAND", &command_text(command, command_id));
    append_element(&mut text, "NOTE", note);
    text
}
fn shell_text(shell: &ShellView, include_alive: bool) -> String {
    let presentation = shell.presentation(include_alive);
    let mut text = String::new();
    if let Some(alive) = presentation.alive {
        append_element(&mut text, "ALIVE", &alive.to_string());
    }
    append_element(&mut text, "TITLE", presentation.title);
    append_element(&mut text, "TYPE", presentation.shell_type);
    append_element(&mut text, "CWD", &presentation.cwd);
    append_element(&mut text, "IDLE", &presentation.idle.to_string());
    text
}
fn command_text(command: &CommandView, command_id: Option<&str>) -> String {
    let presentation = command.presentation(command_id);
    let exit_code = presentation
        .exit_code
        .map_or_else(String::new, |code| code.to_string());
    let mut text = String::new();
    if let Some(id) = presentation.command_id {
        append_element(&mut text, "COMMAND_ID", id);
    }
    match presentation.output {
        OutputPresentation::Separated { stdout, stderr } => {
            append_element(&mut text, "STDOUT", stdout);
            append_element(&mut text, "STDERR", stderr);
        }
        OutputPresentation::Terminal { output } => append_element(&mut text, "OUTPUT", output),
    }
    append_element(
        &mut text,
        "TOTAL_OUTPUT_TOKENS",
        &presentation.total_output_tokens.to_string(),
    );
    append_element(&mut text, "EXIT_CODE", &exit_code);
    append_element(
        &mut text,
        "TIME_CONSUMPTION",
        &super::time_consumption::adaptive(command.time_consumption),
    );
    append_element(&mut text, "FINISHED", &presentation.finished.to_string());
    text
}
fn append_element(text: &mut String, tag: &str, content: &str) {
    let name: String = tag
        .chars()
        .zip(nanoid::rngs::thread_local(tag.len()))
        .map(|(letter, random)| {
            if random.is_multiple_of(2) {
                letter.to_ascii_lowercase()
            } else {
                letter.to_ascii_uppercase()
            }
        })
        .collect();
    if !text.is_empty() {
        text.push('\n');
    }
    text.push('<');
    text.push_str(&name);
    text.push_str(">\n");
    if !content.is_empty() {
        text.push_str(content);
        if !content.ends_with('\n') {
            text.push('\n');
        }
    }
    text.push_str("</");
    text.push_str(&name);
    text.push('>');
}
fn element(tag: &str, content: &str) -> String {
    let mut text = String::new();
    append_element(&mut text, tag, content);
    text
}
