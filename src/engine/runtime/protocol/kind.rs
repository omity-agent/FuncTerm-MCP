use super::{Payload, PayloadKind, RequestKind};
use anyhow::{Result, bail};
impl Payload {
    pub(crate) fn ensure_matches(self, request: RequestKind) -> Result<Self> {
        let expected = request.response_kind();
        let actual = PayloadKind::from(&self);
        if actual == expected {
            return Ok(self);
        }
        bail!("daemon returned {actual}, but {request} expects {expected}")
    }
}
impl RequestKind {
    const fn response_kind(self) -> PayloadKind {
        match self {
            Self::Ping => PayloadKind::Pong,
            Self::NewTab => PayloadKind::TabCreated,
            Self::Close => PayloadKind::TabClosed,
            Self::ManualWrite => PayloadKind::KeyboardWritten,
            Self::SendCommand => PayloadKind::CommandAccepted,
            Self::View => PayloadKind::View,
        }
    }
}
