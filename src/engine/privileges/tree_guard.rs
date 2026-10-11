use alloc::sync::Arc;
use portable_pty::{Child, ChildKiller, ExitStatus};
use std::io;
use std::os::windows::io::{AsHandle as _, AsRawHandle as _, RawHandle};
use windows_spawn::Job;
#[derive(Debug)]
pub(super) struct HostedChild {
    child: windows_spawn::Child,
    job: Arc<Job>,
}
impl HostedChild {
    pub(super) const fn new(child: windows_spawn::Child, job: Arc<Job>) -> Self {
        Self { child, job }
    }
}
impl Child for HostedChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.child.try_wait()?.map(ExitStatus::from))
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        Ok(self.child.wait()?.into())
    }
    fn process_id(&self) -> Option<u32> {
        Some(self.child.id())
    }
    fn as_raw_handle(&self) -> Option<RawHandle> {
        Some(self.child.as_handle().as_raw_handle())
    }
}
impl ChildKiller for HostedChild {
    fn kill(&mut self) -> io::Result<()> {
        self.job.terminate(1)
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(TreeKiller(Arc::clone(&self.job)))
    }
}
#[derive(Debug)]
struct TreeKiller(Arc<Job>);
impl ChildKiller for TreeKiller {
    fn kill(&mut self) -> io::Result<()> {
        self.0.terminate(1)
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self(Arc::clone(&self.0)))
    }
}
