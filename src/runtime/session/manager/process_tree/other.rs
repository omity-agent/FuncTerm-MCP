use anyhow::Result;
use portable_pty::Child;
#[derive(Default)]
pub(crate) struct ProcessTree;
impl ProcessTree {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
    pub(crate) fn attach(&self, _child: &dyn Child) -> Result<()> {
        Ok(())
    }
    pub(crate) fn terminate(&self) -> Result<()> {
        Ok(())
    }
}
