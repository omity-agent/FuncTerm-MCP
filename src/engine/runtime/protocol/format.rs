use super::{CommandView, Payload, ShellView, ViewResult};
impl Payload {
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Pong => element("PONG", ""),
            Self::TabCreated { tab_id } => tab_created_plain_text(&tab_id),
            Self::KeyboardWritten { view } => view.tab_plain_text(false),
            Self::CommandAccepted {
                command_id, view, ..
            } => view.command_plain_text(false, Some(&command_id)),
            Self::View(view) => view.into_plain_text(),
        }
    }
}
impl ViewResult {
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Tab {
                shell,
                screen,
                note,
            } => tab_plain_text(&shell, &screen, &note, true),
            Self::Command {
                shell,
                command,
                note,
            } => command_plain_text(&shell, &command, &note, true, None),
        }
    }
    fn tab_plain_text(self, include_alive: bool) -> String {
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
            } => command_plain_text(&shell, &command, &note, include_alive, None),
        }
    }
    fn command_plain_text(self, include_alive: bool, command_id: Option<&str>) -> String {
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
        .map_or_else(|| "pending".to_owned(), |code| code.to_string());
    let mut items = Vec::new();
    if let Some(id) = presentation.command_id {
        items.push(element("COMMAND_ID", id));
    }
    items.extend([
        element("STDOUT", presentation.stdout),
        element("STDERR", presentation.stderr),
        element("EXIT_CODE", &exit_code),
        element(
            "TIME_CONSUMPTION",
            &super::time_consumption::adaptive(command.time_consumption),
        ),
        element("FINISHED", &presentation.finished.to_string()),
    ]);
    items.join("\n")
}
fn append_element(text: &mut String, tag: &str, content: &str) {
    if !text.is_empty() {
        text.push('\n');
    }
    text.push('<');
    text.push_str(tag);
    text.push_str(">\n");
    if !content.is_empty() {
        text.push_str(content);
        if !content.ends_with('\n') {
            text.push('\n');
        }
    }
    text.push_str("</");
    text.push_str(tag);
    text.push('>');
}
fn element(tag: &str, content: &str) -> String {
    let mut text = String::new();
    append_element(&mut text, tag, content);
    text
}
