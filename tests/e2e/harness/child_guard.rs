use anyhow::{Context as _, Result};
#[cfg(not(windows))]
use kill_tree::{Config, blocking::kill_tree_with_config};
#[cfg(windows)]
use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
use std::process::{Child, Command};
#[cfg(windows)]
use windows_spawn::Job;
pub(crate) struct ChildGuard {
    child: Child,
    #[cfg(windows)]
    job: Job,
}
impl ChildGuard {
    #[cfg(not(windows))]
    pub(crate) fn spawn(command: &mut Command) -> Result<Self> {
        Ok(Self {
            child: command.spawn().context("failed to spawn test process")?,
        })
    }
    #[cfg(windows)]
    pub(crate) fn spawn(command: &mut Command) -> Result<Self> {
        use std::os::windows::process::{CommandExt as _, ProcThreadAttributeList};
        use windows::Win32::PROC_THREAD_ATTRIBUTE_JOB_LIST;
        let job = Job::create()?;
        job.set_kill_on_close(true)?;
        let handles = [job.as_handle().as_raw_handle()];
        let attribute = usize::try_from(PROC_THREAD_ATTRIBUTE_JOB_LIST)
            .context("test job attribute does not fit usize")?;
        let attributes = ProcThreadAttributeList::build()
            .attribute(attribute, &handles)
            .finish()
            .context("failed to build test process job attributes")?;
        let child = command
            .spawn_with_attributes(&attributes)
            .context("failed to spawn isolated test process")?;
        Ok(Self { child, job })
    }
    pub(crate) const fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }
    pub(crate) fn terminate(&mut self) {
        #[cfg(windows)]
        self.job.terminate(1).unwrap();
        #[cfg(not(windows))]
        if self.child.try_wait().unwrap().is_none() {
            let config = Config {
                signal: "SIGKILL".to_owned(),
                ..Default::default()
            };
            kill_tree_with_config(self.child.id(), &config).unwrap();
        }
        self.child.wait().unwrap();
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}
