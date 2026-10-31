use anyhow::{Context as _, Result};
use core::time::Duration;
#[cfg(not(windows))]
use kill_tree::{Config, blocking::kill_tree_with_config};
use std::io::Read;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use std::process::{Child, Command, ExitStatus};
use std::sync::mpsc;
use std::thread;
use wait_timeout::ChildExt as _;
pub(crate) struct ChildGuard {
    child: Child,
    #[cfg(windows)]
    job: OwnedHandle,
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
        let job = create_test_job()?;
        let handles = [job.as_raw_handle()];
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
        {
            use windows::Win32::TerminateJobObject;
            unsafe { TerminateJobObject(self.job.as_raw_handle(), 1_u32) }
                .ok()
                .context("failed to terminate test job")
                .unwrap();
        }
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
#[cfg(windows)]
fn create_test_job() -> Result<OwnedHandle> {
    use windows::Win32::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows::core::PCWSTR;
    let raw_job = unsafe { CreateJobObjectW(None, PCWSTR::null()) };
    if raw_job.is_null() {
        return Err(std::io::Error::last_os_error()).context("failed to create test job");
    }
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = u32::try_from(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
        .context("test job limit flags do not fit u32")?;
    let size = u32::try_from(core::mem::size_of_val(&limits))
        .context("test job information size does not fit u32")?;
    unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            core::ptr::from_mut(&mut limits).cast(),
            size,
        )
    }
    .ok()
    .context("failed to configure test job")?;
    Ok(job)
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}
pub(crate) fn wait_for_status(
    child: &mut Child,
    timeout: Duration,
    description: &str,
) -> ExitStatus {
    child.wait_timeout(timeout).unwrap().unwrap_or_else(|| {
        child.kill().unwrap();
        let status = child.wait().unwrap();
        panic!("CLI command {description} timed out after {timeout:?} with status {status}");
    })
}
pub(crate) fn read_pipe(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).unwrap();
        sender.send(bytes).unwrap();
    });
    receiver
}
