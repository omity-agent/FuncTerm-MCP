use super::ShellSession;
use crate::runtime::session::manager::process;
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
        let child = match self.child.get_mut() {
            Ok(child) => child,
            Err(error) => {
                eprintln!("child mutex poisoned during shell cleanup");
                error.into_inner()
            }
        };
        process::cleanup(child.as_mut(), "shell child during cleanup");
        let slave = match self.slave.get_mut() {
            Ok(slave) => slave,
            Err(error) => {
                eprintln!("slave mutex poisoned during shell cleanup");
                error.into_inner()
            }
        };
        drop(slave.take());
        if let Some(reader) = self.reader.take() {
            process::join_reader(reader, "pty reader thread");
        }
    }
}
