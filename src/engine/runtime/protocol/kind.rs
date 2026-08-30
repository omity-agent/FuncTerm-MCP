use super::{Payload, PayloadKind, Request};
use anyhow::{Result, bail};
impl Payload {
    pub(crate) fn ensure_matches(self, request: &Request) -> Result<Self> {
        let expected = request.response_kind();
        if self.kind() == expected {
            return Ok(self);
        }
        bail!(
            "daemon returned {}, but {} expects {}",
            self.kind().name(),
            request.name(),
            expected.name()
        )
    }
    fn kind(&self) -> PayloadKind {
        self.into()
    }
}
impl Request {
    const fn response_kind(&self) -> PayloadKind {
        match *self {
            Self::Ping => PayloadKind::Pong,
            Self::NewTab {
                starting_directory: _,
                starting_shell: _,
                environment: _,
            } => PayloadKind::TabCreated,
            Self::ManualWrite {
                tab_id: _,
                input: _,
                waiting: _,
            } => PayloadKind::KeyboardWritten,
            Self::SendCommand {
                tab_id: _,
                command: _,
                waiting: _,
            } => PayloadKind::CommandAccepted,
            Self::View { id: _, waiting: _ } => PayloadKind::View,
        }
    }
    const fn name(&self) -> &'static str {
        match *self {
            Self::Ping => "ping",
            Self::NewTab {
                starting_directory: _,
                starting_shell: _,
                environment: _,
            } => "new-tab",
            Self::ManualWrite {
                tab_id: _,
                input: _,
                waiting: _,
            } => "manual-write",
            Self::SendCommand {
                tab_id: _,
                command: _,
                waiting: _,
            } => "send-command",
            Self::View { id: _, waiting: _ } => "view",
        }
    }
}
impl PayloadKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Pong => "pong",
            Self::TabCreated => "tab-created",
            Self::KeyboardWritten => "keyboard-written",
            Self::CommandAccepted => "command-accepted",
            Self::View => "view",
        }
    }
}
