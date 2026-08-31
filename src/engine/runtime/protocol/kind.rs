use super::{Payload, PayloadKind, Request, RequestKind};
use anyhow::{Result, bail};
impl Payload {
    pub(crate) fn ensure_matches(self, request: &Request) -> Result<Self> {
        let expected = request.response_kind();
        let actual = PayloadKind::from(&self);
        if actual == expected {
            return Ok(self);
        }
        bail!(
            "daemon returned {}, but {} expects {}",
            actual,
            RequestKind::from(request),
            expected
        )
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
}
