use crate::contract::{COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV};
#[derive(Clone, Copy)]
pub(super) enum PosixDialect {
    Bash,
    Zsh,
}
impl PosixDialect {
    pub(super) const fn emulate(self) -> &'static str {
        match self {
            Self::Bash => "",
            Self::Zsh => "    emulate -L zsh\n",
        }
    }
    pub(super) const fn mkdir(self) -> &'static str {
        match self {
            Self::Bash => "mkdir -p",
            Self::Zsh => "mkdir -p --",
        }
    }
    pub(super) fn previous_flags(self) -> String {
        match self {
            Self::Bash => format!(
                r#"    local had_previous_command_id=0
    local had_previous_command_directory=0
    if [ "${{{COMMAND_ID_ENV}+x}}" ]; then
        had_previous_command_id=1
    fi
    if [ "${{{COMMAND_DIRECTORY_ENV}+x}}" ]; then
        had_previous_command_directory=1
    fi"#
            ),
            Self::Zsh => format!(
                "    local had_previous_command_id=${{+{COMMAND_ID_ENV}}}\n    local had_previous_command_directory=${{+{COMMAND_DIRECTORY_ENV}}}"
            ),
        }
    }
    pub(super) const fn truncate(self) -> &'static str {
        match self {
            Self::Bash => ": >",
            Self::Zsh => ": >|",
        }
    }
    pub(super) const fn write_done_temp(self) -> &'static str {
        match self {
            Self::Bash => ">",
            Self::Zsh => ">|",
        }
    }
    pub(super) const fn move_done(self) -> &'static str {
        match self {
            Self::Bash => "mv \"$done_temp_file\" \"$done_file\"",
            Self::Zsh => "mv -f -- \"$done_temp_file\" \"$done_file\"",
        }
    }
    pub(super) const fn cd(self) -> &'static str {
        match self {
            Self::Bash => "cd",
            Self::Zsh => "builtin cd --",
        }
    }
    pub(super) fn test_arg(self, arg: &str) -> String {
        match self {
            Self::Bash => format!("[ \"${arg}\" = 1 ]"),
            Self::Zsh => format!("[[ \"${arg}\" == 1 ]]"),
        }
    }
}
