use super::{CommandView, Payload, ShellView, ViewResult};
impl Payload {
    pub(crate) fn into_plain_text(self) -> String {
        match self {
            Self::Pong => element("PONG", ""),
            Self::TabCreated { tab_id } => element("TAB_ID", &tab_id),
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
            Self::Tab { .. } => self.tab_plain_text(true),
            Self::Command { .. } => self.command_plain_text(true, None),
        }
    }
    fn tab_plain_text(self, include_alive: bool) -> String {
        match self {
            Self::Tab {
                shell,
                screen,
                note,
            } => tab_text(&shell, screen, note, include_alive),
            Self::Command {
                shell,
                command,
                note,
            } => command_section_text(&shell, &command, note, include_alive, None),
        }
    }
    fn command_plain_text(self, include_alive: bool, command_id: Option<&str>) -> String {
        match self {
            Self::Tab {
                shell,
                screen,
                note,
            } => tab_text(&shell, screen, note, include_alive),
            Self::Command {
                shell,
                command,
                note,
            } => command_section_text(&shell, &command, note, include_alive, command_id),
        }
    }
}
fn tab_text(shell: &ShellView, screen: String, note: String, include_alive: bool) -> String {
    elements([
        ("SHELL", shell_text(shell, include_alive)),
        ("SCREEN", screen),
        ("NOTE", note),
    ])
}
fn command_section_text(
    shell: &ShellView,
    command: &CommandView,
    note: String,
    include_alive: bool,
    command_id: Option<&str>,
) -> String {
    elements([
        ("SHELL", shell_text(shell, include_alive)),
        ("COMMAND", command_text(command, command_id)),
        ("NOTE", note),
    ])
}
fn shell_text(shell: &ShellView, include_alive: bool) -> String {
    let mut items = Vec::new();
    if include_alive {
        items.push(("ALIVE", shell.alive.to_string()));
    }
    items.extend([
        ("TITLE", shell.title.clone()),
        ("TYPE", shell.shell_type.display_name().to_owned()),
        ("CWD", shell.cwd.clone()),
        ("IDLE", shell.idle.to_string()),
    ]);
    elements_vec(items)
}
fn command_text(command: &CommandView, command_id: Option<&str>) -> String {
    let exit_code = command
        .exit_code
        .map_or_else(|| "pending".to_owned(), |code| code.to_string());
    let mut items = Vec::new();
    if let Some(id) = command_id {
        items.push(("COMMAND_ID", id.to_owned()));
    }
    items.extend([
        ("STDOUT", command.stdout.clone()),
        ("STDERR", command.stderr.clone()),
        ("EXIT_CODE", exit_code),
        ("TIME_CONSUMPTION", command.time_consumption.clone()),
        ("FINISHED", command.finished.to_string()),
    ]);
    elements_vec(items)
}
fn elements<const COUNT: usize>(items: [(&str, String); COUNT]) -> String {
    elements_vec(Vec::from(items))
}
fn elements_vec(items: Vec<(&str, String)>) -> String {
    let mut text = String::new();
    for (index, (tag, content)) in items.into_iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&element(tag, &content));
    }
    text
}
fn element(tag: &str, content: &str) -> String {
    format!("<{tag}>\n{content}\n</{tag}>")
}
