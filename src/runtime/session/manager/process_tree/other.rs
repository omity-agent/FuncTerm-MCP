use anyhow::{Result, bail};
use portable_pty::Child;
#[derive(Default)]
pub(crate) struct ProcessTree;
impl ProcessTree {
    pub(crate) fn new() -> Result<Self> {
        bail!("process tree cleanup is not implemented for this operating system")
    }
    pub(crate) fn attach(&self, _child: &dyn Child) -> Result<()> {
        bail!("process tree cleanup is not implemented for this operating system")
    }
    pub(crate) fn terminate(&self) -> Result<()> {
        bail!("process tree cleanup is not implemented for this operating system")
    }
}
