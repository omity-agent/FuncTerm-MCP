use super::ShellSession;
use crate::runtime::session::manager::process;
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
        let child = self.child.get_mut();
        process::cleanup(child.as_mut(), "shell child during cleanup");
        let slave = self.slave.get_mut();
        drop(slave.take());
        if let Some(reader) = self.reader.take() {
            process::join_reader(reader, "pty reader thread");
        }
    }
}
