use super::{Payload, Request};
use anyhow::{Result, bail};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadKind {
    Pong,
    TabCreated,
    KeyboardWritten,
    CommandAccepted,
    View,
}
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
    const fn kind(&self) -> PayloadKind {
        match *self {
            Self::Pong => PayloadKind::Pong,
            Self::TabCreated { .. } => PayloadKind::TabCreated,
            Self::KeyboardWritten => PayloadKind::KeyboardWritten,
            Self::CommandAccepted { .. } => PayloadKind::CommandAccepted,
            Self::View(_) => PayloadKind::View,
        }
    }
}
impl Request {
    const fn response_kind(&self) -> PayloadKind {
        match *self {
            Self::Ping => PayloadKind::Pong,
            Self::NewTab { .. } => PayloadKind::TabCreated,
            Self::ManualWrite { .. } => PayloadKind::KeyboardWritten,
            Self::SendCommand { .. } => PayloadKind::CommandAccepted,
            Self::View { .. } => PayloadKind::View,
        }
    }
    const fn name(&self) -> &'static str {
        match *self {
            Self::Ping => "ping",
            Self::NewTab { .. } => "new-tab",
            Self::ManualWrite { .. } => "manual-write",
            Self::SendCommand { .. } => "send-command",
            Self::View { .. } => "view",
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
