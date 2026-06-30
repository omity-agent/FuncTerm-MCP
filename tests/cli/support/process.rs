use core::time::Duration;
use std::io::Read;
use std::process::{Child, ExitStatus, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
pub(crate) struct ChildGuard {
    child: Child,
}
impl ChildGuard {
    pub(crate) const fn new(child: Child) -> Self {
        Self { child }
    }
    pub(crate) fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this test guard"
)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}
pub(crate) fn wait_for_status(child: &mut Child, timeout: Duration) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() >= timeout {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("CLI command timed out after {timeout:?} with status {status}");
        }
        thread::sleep(Duration::from_millis(50));
    }
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
pub(crate) const fn output_from_parts(
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
) -> Output {
    Output {
        status,
        stdout,
        stderr,
    }
}
