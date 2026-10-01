use core::time::Duration;
use kill_tree::{Config, blocking::kill_tree_with_config};
#[cfg(windows)]
use std::io::Read;
use std::process::{Child, ExitStatus, Output};
#[cfg(windows)]
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
    pub(crate) fn terminate(&mut self) {
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
pub(crate) fn wait_for_status(
    child: &mut Child,
    timeout: Duration,
    description: &str,
) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() >= timeout {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("CLI command {description} timed out after {timeout:?} with status {status}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
#[cfg(windows)]
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
