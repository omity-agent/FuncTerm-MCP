use crate::engine::privileges::tree_guard;
use anyhow::{Context as _, Result};
use portable_pty::{Child, ChildKiller};
use windows_spawn::Job;
pub(super) enum ProcessGuard {
    Hosted(Box<dyn ChildKiller + Send + Sync>),
    Assigned(Job),
}
impl ProcessGuard {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow owning Job handles or killers in place without moving them"
    )]
    pub(super) fn terminate(&mut self) -> Result<()> {
        match self {
            Self::Hosted(killer) => killer
                .kill()
                .context("failed to terminate hosted shell Job"),
            Self::Assigned(job) => job
                .terminate(1)
                .context("failed to terminate assigned shell Job"),
        }
    }
}
pub(super) fn assign(
    child: &dyn Child,
    existing: Option<Box<dyn ChildKiller + Send + Sync>>,
) -> Result<ProcessGuard> {
    if let Some(killer) = existing {
        return Ok(ProcessGuard::Hosted(killer));
    }
    let job = tree_guard::create_job()?;
    tree_guard::attach(&job, child)?;
    Ok(ProcessGuard::Assigned(job))
}
