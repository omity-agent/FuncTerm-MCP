use super::rendering::CommandScreen;
use alloc::sync::Arc;
use anyhow::Result;
use bytestring::ByteString;
use parking_lot::Mutex;
pub(super) enum RenderedOutput {
    Pending(Arc<Mutex<CommandScreen>>),
    Active(Arc<Mutex<CommandScreen>>),
    Frozen(ByteString),
}
impl RenderedOutput {
    pub(super) const fn new(screen: Arc<Mutex<CommandScreen>>) -> Self {
        Self::Pending(screen)
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow the pending screen while advancing the capture state"
    )]
    pub(super) fn start(&mut self) -> Result<()> {
        let Self::Pending(screen) = self else {
            anyhow::bail!("terminal output capture is not pending");
        };
        screen.lock().begin()?;
        *self = Self::Active(Arc::clone(screen));
        Ok(())
    }
    pub(super) fn contents(&self) -> Result<ByteString> {
        #[expect(
            clippy::pattern_type_mismatch,
            reason = "borrow the live screen or frozen output without moving it"
        )]
        match self {
            Self::Pending(_) => Ok(ByteString::new()),
            Self::Active(screen) => Ok(screen.lock().contents()?.into()),
            Self::Frozen(output) => Ok(output.clone()),
        }
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow the active screen before replacing it with its frozen output"
    )]
    pub(super) fn finish(&mut self) -> Result<()> {
        let output = match self {
            Self::Pending(_) => String::new(),
            Self::Active(screen) => screen.lock().finish()?,
            Self::Frozen(_) => return Ok(()),
        };
        *self = Self::Frozen(output.into());
        Ok(())
    }
}
